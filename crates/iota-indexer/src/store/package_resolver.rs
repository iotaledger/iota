// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use iota_package_resolver::{
    Package, PackageStore, Resolver, error::Error as PackageResolverError,
};
use iota_sdk_types::Address;
use iota_types::object::Object;

use crate::{db::ConnectionPool, errors::IndexerError, schema::objects, store::diesel_macro::*};

/// A package store that reads packages a simulation wrote before falling back
/// to `fallback`.
///
/// A simulated transaction can publish a package, and then refer to its types —
/// an event emitted from the new package's `init`, for one. That package was
/// never committed, so it is not in the database and only the simulation's own
/// output holds it. The node's JSON-RPC resolves the same way, over the objects
/// the simulation wrote with its store behind them.
pub struct SimulationPackageStore<F: PackageStore> {
    /// The packages among the objects the simulation wrote, kept as objects and
    /// deserialized on demand: a simulation usually publishes nothing, and when
    /// it does the package is only read if a type actually refers to it.
    published: BTreeMap<Address, Object>,
    fallback: Arc<Resolver<F>>,
}

impl<F: PackageStore> SimulationPackageStore<F> {
    pub fn new(written_objects: &[Object], fallback: Arc<Resolver<F>>) -> Self {
        let published = written_objects
            .iter()
            .filter(|object| object.data.as_opt_package().is_some())
            .map(|object| (object.id().into(), object.clone()))
            .collect();

        Self {
            published,
            fallback,
        }
    }
}

#[async_trait]
impl<F: PackageStore> PackageStore for SimulationPackageStore<F> {
    async fn fetch(&self, id: Address) -> Result<Arc<Package>, PackageResolverError> {
        match self.published.get(&id) {
            Some(object) => Package::read_from_object(object).map(Arc::new),
            None => self.fallback.package_store().fetch(id).await,
        }
    }
}

/// A package resolver that reads packages from the database.
pub struct IndexerStorePackageResolver {
    cp: ConnectionPool,
}

impl Clone for IndexerStorePackageResolver {
    fn clone(&self) -> IndexerStorePackageResolver {
        Self {
            cp: self.cp.clone(),
        }
    }
}

impl IndexerStorePackageResolver {
    pub fn new(cp: ConnectionPool) -> Self {
        Self { cp }
    }
}

#[async_trait]
impl PackageStore for IndexerStorePackageResolver {
    async fn fetch(&self, id: Address) -> Result<Arc<Package>, PackageResolverError> {
        let pkg = self
            .get_package_from_db_in_blocking_task(id)
            .await
            .map_err(|e| PackageResolverError::Store {
                store: "PostgresDB",
                source: Arc::new(e),
            })?;
        Ok(Arc::new(pkg))
    }
}

impl IndexerStorePackageResolver {
    fn get_package_from_db(&self, id: Address) -> Result<Package, IndexerError> {
        let Some(bcs) = read_only_blocking!(&self.cp, |conn| {
            let query = objects::dsl::objects
                .select(objects::dsl::serialized_object)
                .filter(objects::dsl::object_id.eq(id.as_bytes().to_vec()));
            query.get_result::<Vec<u8>>(conn).optional()
        })?
        else {
            return Err(IndexerError::PostgresRead(format!(
                "Package not found in DB: {id}"
            )));
        };
        let object = bcs::from_bytes::<Object>(&bcs)?;
        Package::read_from_object(&object).map_err(|e| {
            IndexerError::PostgresRead(format!("Failed parsing object to package: {e:?}"))
        })
    }

    async fn get_package_from_db_in_blocking_task(
        &self,
        id: Address,
    ) -> Result<Package, IndexerError> {
        let this = self.clone();
        spawn_blocking_task(move || this.get_package_from_db(id)).await?
    }
}
