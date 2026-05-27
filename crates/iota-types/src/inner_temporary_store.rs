// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use iota_sdk_types::{Identifier, ObjectId};
use move_binary_format::{CompiledModule, binary_config::BinaryConfig};
use move_bytecode_utils::module_cache::GetModule;
use move_core_types::language_storage::ModuleId;

use crate::{
    base_types::{SequenceNumber, VersionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    error::IotaResult,
    execution::DynamicallyLoadedObjectMetadata,
    move_package::MovePackageExt,
    object::{Object, Owner},
    storage::{BackingPackageStore, InputKey, PackageObject},
};

pub type WrittenObjects = BTreeMap<ObjectId, Object>;
pub type ObjectMap = BTreeMap<ObjectId, Object>;
pub type TxCoins = (ObjectMap, WrittenObjects);

#[derive(Debug, Clone)]
pub struct InnerTemporaryStore {
    pub input_objects: ObjectMap,
    pub mutable_inputs: BTreeMap<ObjectId, (VersionDigest, Owner)>,
    // All the written objects' sequence number should have been updated to the lamport version.
    pub written: WrittenObjects,
    pub loaded_runtime_objects: BTreeMap<ObjectId, DynamicallyLoadedObjectMetadata>,
    pub events: TransactionEvents,
    pub binary_config: BinaryConfig,
    pub runtime_packages_loaded_from_db: BTreeMap<ObjectId, PackageObject>,
    pub lamport_version: SequenceNumber,
}

impl InnerTemporaryStore {
    pub fn get_output_keys(&self, effects: &TransactionEffects) -> Vec<InputKey> {
        let mut output_keys: Vec<_> = self
            .written
            .iter()
            .map(|(id, obj)| {
                if obj.is_package() {
                    InputKey::Package { id: *id }
                } else {
                    InputKey::VersionedObject {
                        id: *id,
                        version: obj.version(),
                    }
                }
            })
            .collect();

        let deleted: HashMap<_, _> = effects
            .deleted()
            .iter()
            .map(|oref| (oref.object_id, oref.version))
            .collect();

        // add deleted shared objects to the outputkeys that then get sent to
        // notify_commit
        let deleted_output_keys = deleted
            .iter()
            .filter(|(id, _)| {
                self.input_objects
                    .get(id)
                    .is_some_and(|obj| obj.is_shared())
            })
            .map(|(id, seq)| InputKey::VersionedObject {
                id: *id,
                version: *seq,
            });
        output_keys.extend(deleted_output_keys);

        // For any previously deleted shared objects that appeared mutably in the
        // transaction, synthesize a notification for the next version of the
        // object.
        let smeared_version = self.lamport_version;
        let deleted_accessed_objects = effects.deleted_mutably_accessed_shared_objects();
        for object_id in deleted_accessed_objects.into_iter() {
            let key = InputKey::VersionedObject {
                id: object_id,
                version: smeared_version,
            };
            output_keys.push(key);
        }

        output_keys
    }
}

pub struct TemporaryModuleResolver<'a, R> {
    temp_store: &'a InnerTemporaryStore,
    fallback: R,
}

impl<'a, R> TemporaryModuleResolver<'a, R> {
    pub fn new(temp_store: &'a InnerTemporaryStore, fallback: R) -> Self {
        Self {
            temp_store,
            fallback,
        }
    }
}

impl<R> GetModule for TemporaryModuleResolver<'_, R>
where
    R: GetModule<Item = Arc<CompiledModule>, Error = anyhow::Error>,
{
    type Error = anyhow::Error;
    type Item = Arc<CompiledModule>;

    fn get_module_by_id(&self, id: &ModuleId) -> anyhow::Result<Option<Self::Item>, Self::Error> {
        let obj = self
            .temp_store
            .written
            .get(&ObjectId::new(id.address().into_bytes()));
        if let Some(o) = obj {
            if let Some(p) = o.data.as_package_opt() {
                return Ok(Some(Arc::new(p.deserialize_module(
                    &Identifier::new_unchecked(id.name().as_str()),
                    &self.temp_store.binary_config,
                )?)));
            }
        }
        self.fallback.get_module_by_id(id)
    }
}

impl BackingPackageStore for InnerTemporaryStore {
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        Ok(self
            .written
            .get(package_id)
            .cloned()
            .map(PackageObject::new))
    }
}

pub struct PackageStoreWithFallback<P, F> {
    primary: P,
    fallback: F,
}

impl<P, F> PackageStoreWithFallback<P, F> {
    pub fn new(primary: P, fallback: F) -> Self {
        Self { primary, fallback }
    }
}

impl<P, F> BackingPackageStore for PackageStoreWithFallback<P, F>
where
    P: BackingPackageStore,
    F: BackingPackageStore,
{
    fn get_package_object(&self, package_id: &ObjectId) -> IotaResult<Option<PackageObject>> {
        if let Some(package) = self.primary.get_package_object(package_id)? {
            Ok(Some(package))
        } else {
            self.fallback.get_package_object(package_id)
        }
    }
}
