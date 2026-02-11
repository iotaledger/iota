// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use async_trait::async_trait;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl};
use iota_package_resolver::{Package, PackageStore, error::Error as PackageResolverError};
use iota_types::{base_types::IotaAddress, object::Object};

use crate::{db::ConnectionPool, errors::IndexerError, schema::objects, store::diesel_macro::*};

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
    async fn fetch(&self, id: IotaAddress) -> Result<Arc<Package>, PackageResolverError> {
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
    fn get_package_from_db(&self, id: IotaAddress) -> Result<Package, IndexerError> {
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
        id: IotaAddress,
    ) -> Result<Package, IndexerError> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || this.get_package_from_db(id)).await?
    }
}
