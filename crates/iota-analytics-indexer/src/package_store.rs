// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use iota_grpc_client::Client;
use iota_package_resolver::{
    Package, PackageStore, PackageStoreWithLruCache, error::Error as PackageResolverError,
};
use iota_types::{base_types::ObjectID, object::Object};
use move_core_types::account_address::AccountAddress;
use thiserror::Error;
use typed_store::{
    DBMapUtils, Map, TypedStoreError,
    rocks::{DBMap, MetricConf},
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
    pub(crate) fn update(&self, package: &Object) -> iota_package_resolver::Result<()> {
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
    pub fn new(path: &Path, client: Client) -> Self {
        Self {
            package_store_tables: PackageStoreTables::new(path),
            fallback_client: client,
        }
    }

    pub fn update(&self, object: &Object) -> iota_package_resolver::Result<()> {
        let Some(_package) = object.data.try_as_package() else {
            return Ok(());
        };
        self.package_store_tables.update(object)?;
        Ok(())
    }

    pub async fn get(&self, id: AccountAddress) -> iota_package_resolver::Result<Object> {
        let object = if let Some(object) = self
            .package_store_tables
            .packages
            .get(&ObjectID::from(id))
            .map_err(Error::TypedStore)?
        {
            object
        } else {
            let object_id = ObjectID::from(id);
            fn grpc_err(e: impl std::error::Error + Send + Sync + 'static) -> PackageResolverError {
                PackageResolverError::Store {
                    store: "gRPC",
                    source: Arc::new(e),
                }
            }
            let objects = self
                .fallback_client
                .get_objects(&[(object_id.into(), None)], Some("bcs"))
                .await
                .map_err(grpc_err)?
                .into_inner();
            let proto_obj = objects
                .into_iter()
                .next()
                .ok_or(PackageResolverError::PackageNotFound(id))?;
            let sdk_obj = proto_obj.object().map_err(grpc_err)?;
            let object: Object = sdk_obj.try_into().map_err(grpc_err)?;
            self.update(&object)?;
            object
        };
        Ok(object)
    }
}

#[async_trait]
impl PackageStore for LocalDBPackageStore {
    async fn fetch(&self, id: AccountAddress) -> iota_package_resolver::Result<Arc<Package>> {
        let object = self.get(id).await?;
        Ok(Arc::new(Package::read_from_object(&object)?))
    }
}

pub(crate) type PackageCache = PackageStoreWithLruCache<LocalDBPackageStore>;
