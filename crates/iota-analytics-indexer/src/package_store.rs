// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use iota_package_resolver::{
    Package, PackageStore, PackageStoreWithLruCache, Result, error::Error as PackageResolverError,
};
use iota_rest_api::Client;
use iota_types::{base_types::ObjectID, object::Object};
use move_core_types::account_address::AccountAddress;
use thiserror::Error;
use typed_store::{
    DBMapUtils, Map, TypedStoreError,
    rocks::{DBMap, MetricConf},
    traits::{TableSummary, TypedStoreDebug},
};

const STORE: &str = "RocksDB";

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0}")]
    TypedStore(#[from] TypedStoreError),
}

impl From<Error> for PackageResolverError {
    fn from(source: Error) -> Self {
        match source {
            Error::TypedStore(store_error) => Self::Store {
                store: STORE,
                source: Arc::new(store_error),
            },
        }
    }
}

#[derive(DBMapUtils)]
pub struct PackageStoreTables {
    pub(crate) packages: DBMap<ObjectID, Object>,
}

impl PackageStoreTables {
    pub fn new(path: &Path) -> Arc<Self> {
        Arc::new(Self::open_tables_read_write(
            path.to_path_buf(),
            MetricConf::new("package"),
            None,
            None,
        ))
    }
    pub(crate) fn update(&self, package: &Object) -> Result<()> {
        let mut batch = self.packages.batch();
        batch
            .insert_batch(&self.packages, std::iter::once((package.id(), package)))
            .map_err(Error::TypedStore)?;
        batch.write().map_err(Error::TypedStore)?;
        Ok(())
    }
}

/// Store which keeps package objects in a local rocksdb store. It is expected
/// that this store is kept updated with latest version of package objects while
/// iterating over checkpoints. If the local db is missing (or gets deleted),
/// packages are fetched from a full node and local store is updated
#[derive(Clone)]
pub struct LocalDBPackageStore {
    package_store_tables: Arc<PackageStoreTables>,
    fallback_client: Client,
}

impl LocalDBPackageStore {
    pub fn new(path: &Path, rest_url: &str) -> Self {
        Self {
            package_store_tables: PackageStoreTables::new(path),
            fallback_client: Client::new(rest_url),
        }
    }

    pub fn update(&self, object: &Object) -> Result<()> {
        let Some(_package) = object.data.try_as_package() else {
            return Ok(());
        };
        self.package_store_tables.update(object)?;
        Ok(())
    }

    pub async fn get(&self, id: AccountAddress) -> Result<Object> {
        let object = if let Some(object) = self
            .package_store_tables
            .packages
            .get(&ObjectID::from(id))
            .map_err(Error::TypedStore)?
        {
            object
        } else {
            let object = self
                .fallback_client
                .get_object(ObjectID::from(id))
                .await
                .map_err(|_| PackageResolverError::PackageNotFound(id))?;
            self.update(&object)?;
            object
        };
        Ok(object)
    }
}

#[async_trait]
impl PackageStore for LocalDBPackageStore {
    async fn fetch(&self, id: AccountAddress) -> Result<Arc<Package>> {
        let object = self.get(id).await?;
        Ok(Arc::new(Package::read_from_object(&object)?))
    }
}

pub(crate) type PackageCache = PackageStoreWithLruCache<LocalDBPackageStore>;