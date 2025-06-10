// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use iota_package_resolver::{Package, PackageStore, error::Error as PackageResolverError};
use move_core_types::account_address::AccountAddress;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::{config::Config, verifier::get_verified_object};

pub struct RemotePackageStore {
    config: Config,
    cache: Mutex<HashMap<AccountAddress, Arc<Package>>>,
}

impl RemotePackageStore {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            cache: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl PackageStore for RemotePackageStore {
    /// Read package contents. Fails if `id` is not an object, not a package, or
    /// is malformed in some way.
    async fn fetch(&self, id: AccountAddress) -> iota_package_resolver::Result<Arc<Package>> {
        // Check if we have it in the cache
        let res: Result<Arc<Package>> = async move {
            if let Some(package) = self.cache.lock().await.get(&id) {
                info!("Fetch Package: {id} cache hit");
                return Ok(package.clone());
            }

            info!("Fetch Package: {id}");

            let object = get_verified_object(&self.config, id.into()).await?;
            let package = Arc::new(Package::read_from_object(&object)?);

            // Add to the cache
            self.cache.lock().await.insert(id, package.clone());

            Ok(package)
        }
        .await;
        res.map_err(|e| {
            error!("Fetch Package: {id} error: {e:?}");
            PackageResolverError::PackageNotFound(id)
        })
    }
}
