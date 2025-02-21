// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use async_graphql::dataloader::Loader;
use async_trait::async_trait;
use diesel::{ExpressionMethods, QueryDsl};
use iota_indexer::{models::packages::StoredPackage, schema::packages};
use iota_package_resolver::{
    Package, PackageStore, PackageStoreWithLruCache, Resolver, Result,
    error::Error as PackageResolverError,
};
use move_core_types::account_address::AccountAddress;

use crate::data::{DataLoader, Db, DbConnection, QueryExecutor};

const STORE: &str = "PostgresDB";

pub(crate) type PackageCache = PackageStoreWithLruCache<DbPackageStore>;
pub(crate) type PackageResolver = Arc<Resolver<PackageCache>>;

/// Store which fetches package for the given address from the backend db on
/// every call to `fetch`
pub struct DbPackageStore(DataLoader);

/// `DataLoader` key for fetching the latest version of a `Package` by its ID.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
struct PackageKey(AccountAddress);

impl DbPackageStore {
    pub fn new(loader: DataLoader) -> Self {
        Self(loader)
    }
}

#[async_trait]
impl PackageStore for DbPackageStore {
    async fn fetch(&self, id: AccountAddress) -> Result<Arc<Package>> {
        let Self(DataLoader(loader)) = self;
        let Some(package) = loader.load_one(PackageKey(id)).await? else {
            return Err(PackageResolverError::PackageNotFound(id));
        };

        Ok(package)
    }
}

impl Loader<PackageKey> for Db {
    type Value = Arc<Package>;
    type Error = PackageResolverError;

    async fn load(&self, keys: &[PackageKey]) -> Result<HashMap<PackageKey, Arc<Package>>> {
        use packages::dsl;

        let ids: BTreeSet<_> = keys.iter().map(|PackageKey(id)| id.to_vec()).collect();
        let stored_packages: Vec<StoredPackage> = self
            .execute(move |conn| {
                conn.results(move || {
                    dsl::packages.filter(dsl::package_id.eq_any(ids.iter().cloned()))
                })
            })
            .await
            .map_err(|e| PackageResolverError::Store {
                store: STORE,
                source: Arc::new(e),
            })?;

        let mut id_to_package = HashMap::new();
        for stored_package in stored_packages {
            let move_package = bcs::from_bytes(&stored_package.move_package)?;
            let package = Package::read_from_package(&move_package)?;
            id_to_package.insert(PackageKey(*move_package.id()), Arc::new(package));
        }

        Ok(id_to_package)
    }
}
