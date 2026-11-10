// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The object store a single transaction executes against: the objects the checkpoint carries
//! for the transaction, completed on demand from the full node at the versions the transaction
//! read.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use iota_sdk_types::{EpochId, ObjectId, Owner, Version};
use iota_types::{
    error::{IotaError, IotaResult},
    object::Object,
    storage::{
        BackingPackageStore, ChildObjectResolver, ObjectStore, PackageObject,
        error::{Error as StorageError, Result as StorageResult},
    },
};
use tokio::runtime::Handle;

use crate::rpc::Rpc;

/// Move packages shared by all transactions of a run. Packages are immutable, so a package read
/// once is valid for every later transaction. System packages are pinned to the versions of the
/// replayed epoch.
pub struct PackageCache {
    rpc: Arc<Rpc>,
    system_packages: BTreeMap<ObjectId, Object>,
    packages: Mutex<HashMap<ObjectId, Object>>,
}

impl PackageCache {
    pub fn new(rpc: Arc<Rpc>, system_packages: BTreeMap<ObjectId, Object>) -> Self {
        Self {
            rpc,
            system_packages,
            packages: Mutex::new(HashMap::new()),
        }
    }

    /// Reads the package from the cache or the full node. Must be called from a blocking
    /// context, as it waits for the full node.
    fn get_blocking(&self, handle: &Handle, id: &ObjectId) -> anyhow::Result<Option<Object>> {
        if let Some(package) = self.system_packages.get(id) {
            return Ok(Some(package.clone()));
        }
        if let Some(package) = self.packages.lock().expect("package cache lock").get(id) {
            return Ok(Some(package.clone()));
        }
        let Some(object) = handle.block_on(self.rpc.latest_object(*id))? else {
            return Ok(None);
        };
        if object.is_package() {
            self.packages
                .lock()
                .expect("package cache lock")
                .insert(*id, object.clone());
        }
        Ok(Some(object))
    }

    pub fn system_package(&self, id: &ObjectId) -> Option<Object> {
        self.system_packages.get(id).cloned()
    }

    pub async fn get(&self, id: &ObjectId) -> anyhow::Result<Option<Object>> {
        if let Some(package) = self.system_packages.get(id) {
            return Ok(Some(package.clone()));
        }
        if let Some(package) = self.packages.lock().expect("package cache lock").get(id) {
            return Ok(Some(package.clone()));
        }
        let Some(object) = self.rpc.latest_object(*id).await? else {
            return Ok(None);
        };
        if object.is_package() {
            self.packages
                .lock()
                .expect("package cache lock")
                .insert(*id, object.clone());
        }
        Ok(Some(object))
    }
}

/// The objects one transaction executes against. Dropped once the transaction is replayed.
pub struct TransactionStore {
    rpc: Arc<Rpc>,
    packages: Arc<PackageCache>,
    handle: Handle,
    /// Every object read before the transaction ran is at most this version.
    max_input_version: Version,
    /// Objects by ID, at the version the transaction read.
    objects: Mutex<BTreeMap<ObjectId, Object>>,
    /// Objects read from the full node during execution, for the report.
    fetched_during_execution: Mutex<Vec<ObjectId>>,
    /// Errors from the full node during execution, for the report.
    errors: Mutex<Vec<String>>,
}

impl TransactionStore {
    pub fn new(
        rpc: Arc<Rpc>,
        packages: Arc<PackageCache>,
        handle: Handle,
        max_input_version: Version,
        objects: BTreeMap<ObjectId, Object>,
    ) -> Self {
        Self {
            rpc,
            packages,
            handle,
            max_input_version,
            objects: Mutex::new(objects),
            fetched_during_execution: Mutex::new(vec![]),
            errors: Mutex::new(vec![]),
        }
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors.lock().expect("errors lock").clone()
    }

    pub fn fetched_during_execution(&self) -> Vec<ObjectId> {
        self.fetched_during_execution
            .lock()
            .expect("fetch log lock")
            .clone()
    }

    fn record_error(&self, error: String) {
        self.errors.lock().expect("errors lock").push(error);
    }

    fn cached(&self, id: &ObjectId) -> Option<Object> {
        self.objects.lock().expect("objects lock").get(id).cloned()
    }

    /// The latest version of the object that is at most `version_upper_bound`, read from the
    /// transaction's objects or from the full node.
    fn object_at_or_before(
        &self,
        id: &ObjectId,
        version_upper_bound: Version,
    ) -> anyhow::Result<Option<Object>> {
        if let Some(object) = self.cached(id)
            && object.version() <= version_upper_bound
        {
            return Ok(Some(object));
        }
        let bound = version_upper_bound.min(self.max_input_version);
        let fetched = self
            .handle
            .block_on(self.rpc.object_at_or_before_version(*id, bound))?;
        self.fetched_during_execution
            .lock()
            .expect("fetch log lock")
            .push(*id);
        if let Some(object) = &fetched {
            self.objects
                .lock()
                .expect("objects lock")
                .insert(*id, object.clone());
        }
        Ok(fetched)
    }
}

impl BackingPackageStore for TransactionStore {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        match self.packages.get_blocking(&self.handle, package_id) {
            Ok(object) => Ok(object
                .filter(|object| object.is_package())
                .map(PackageObject::new)),
            Err(e) => {
                self.record_error(format!("package {package_id}: {e:#}"));
                Err(IotaError::Storage(e.to_string()))
            }
        }
    }
}

impl ChildObjectResolver for TransactionStore {
    fn read_child_object(
        &self,
        parent: &ObjectId,
        child: &ObjectId,
        child_version_upper_bound: Version,
    ) -> IotaResult<Option<Object>> {
        let object = self
            .object_at_or_before(child, child_version_upper_bound)
            .map_err(|e| {
                self.record_error(format!("child object {child}: {e:#}"));
                IotaError::Storage(e.to_string())
            })?;
        Ok(object.filter(|object| object.owner == Owner::Object(*parent)))
    }

    fn get_object_received_at_version(
        &self,
        owner: &ObjectId,
        receiving_object_id: &ObjectId,
        receive_object_at_version: Version,
        _epoch_id: EpochId,
    ) -> IotaResult<Option<Object>> {
        Ok(self.cached(receiving_object_id).filter(|object| {
            object.version() == receive_object_at_version
                && object.owner == Owner::Address((*owner).into())
        }))
    }
}

impl ObjectStore for TransactionStore {
    fn try_get_object(&self, object_id: &ObjectId) -> StorageResult<Option<Object>> {
        if let Some(object) = self.cached(object_id) {
            return Ok(Some(object));
        }
        // System packages are upgraded in place, so their version says nothing about when they
        // were written: use the versions pinned for the replayed epoch.
        if let Some(package) = self.packages.system_package(object_id) {
            return Ok(Some(package));
        }
        self.object_at_or_before(object_id, self.max_input_version)
            .map_err(|e| {
                self.record_error(format!("object {object_id}: {e:#}"));
                StorageError::custom(e.to_string())
            })
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> StorageResult<Option<Object>> {
        if let Some(object) = self.cached(object_id)
            && object.version() == version
        {
            return Ok(Some(object));
        }
        let fetched = self
            .handle
            .block_on(self.rpc.object_at_version(*object_id, version))
            .map_err(|e| {
                self.record_error(format!("object {object_id} at {version}: {e:#}"));
                StorageError::custom(e.to_string())
            })?;
        Ok(fetched)
    }
}
