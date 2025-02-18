// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tap::Pipe;

use crate::{
    base_types::ObjectRef,
    effects::{
        IDOperation, ObjectIn, ObjectOut, TransactionEffects, TransactionEffectsAPI,
        TransactionEvents,
    },
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents},
    object::Object,
    storage::BackingPackageStore,
    transaction::Transaction,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointData {
    pub checkpoint_summary: CertifiedCheckpointSummary,
    pub checkpoint_contents: CheckpointContents,
    pub transactions: Vec<CheckpointTransaction>,
}

impl CheckpointData {
    // returns the latest versions of the output objects that still exist at the end
    // of the checkpoint
    pub fn latest_live_output_objects(&self) -> Vec<&Object> {
        let mut latest_live_objects = BTreeMap::new();
        for tx in self.transactions.iter() {
            for obj in tx.output_objects.iter() {
                latest_live_objects.insert(obj.id(), obj);
            }
            for obj_ref in tx.removed_object_refs_post_version() {
                latest_live_objects.remove(&(obj_ref.0));
            }
        }
        latest_live_objects.into_values().collect()
    }

    // returns the object refs that are eventually deleted or wrapped in the current
    // checkpoint
    pub fn eventually_removed_object_refs_post_version(&self) -> Vec<ObjectRef> {
        let mut eventually_removed_object_refs = BTreeMap::new();
        for tx in self.transactions.iter() {
            for obj_ref in tx.removed_object_refs_post_version() {
                eventually_removed_object_refs.insert(obj_ref.0, obj_ref);
            }
            for obj in tx.output_objects.iter() {
                eventually_removed_object_refs.remove(&(obj.id()));
            }
        }
        eventually_removed_object_refs.into_values().collect()
    }

    pub fn input_objects(&self) -> Vec<&Object> {
        self.transactions
            .iter()
            .flat_map(|tx| &tx.input_objects)
            .collect()
    }

    pub fn all_objects(&self) -> Vec<&Object> {
        self.transactions
            .iter()
            .flat_map(|tx| &tx.input_objects)
            .chain(self.transactions.iter().flat_map(|tx| &tx.output_objects))
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointTransaction {
    /// The input Transaction
    pub transaction: Transaction,
    /// The effects produced by executing this transaction
    pub effects: TransactionEffects,
    /// The events, if any, emitted by this transaction during execution
    pub events: Option<TransactionEvents>,
    /// The state of all inputs to this transaction as they were prior to
    /// execution.
    pub input_objects: Vec<Object>,
    /// The state of all output objects created or mutated or unwrapped by this
    /// transaction.
    pub output_objects: Vec<Object>,
}

impl CheckpointTransaction {
    // provide an iterator over all deleted or wrapped objects in this transaction
    pub fn removed_objects_pre_version(&self) -> impl Iterator<Item = &Object> {
        // Iterator over id and versions for all deleted or wrapped objects
        match &self.effects {
            TransactionEffects::V1(v1) => {
                v1.changed_objects().iter().filter_map(|(id, change)| {
                    match (
                        &change.input_state,
                        &change.output_state,
                        &change.id_operation,
                    ) {
                        // Deleted Objects
                        (
                            ObjectIn::Exist(((version, _d), _o)),
                            ObjectOut::NotExist,
                            IDOperation::Deleted,
                        ) => Some((id, version)),

                        // Wrapped Objects
                        (
                            ObjectIn::Exist(((version, _), _)),
                            ObjectOut::NotExist,
                            IDOperation::None,
                        ) => Some((id, version)),
                        _ => None,
                    }
                })
            }
        }
        // Use id and version to lookup in input Objects
        .map(|(id, version)| {
            self.input_objects
                .iter()
                .find(|o| &o.id() == id && &o.version() == version)
                .expect("all removed objects should show up in input objects")
        })
    }

    pub fn removed_object_refs_post_version(&self) -> impl Iterator<Item = ObjectRef> {
        let deleted = self.effects.deleted().into_iter();
        let wrapped = self.effects.wrapped().into_iter();
        let unwrapped_then_deleted = self.effects.unwrapped_then_deleted().into_iter();
        deleted.chain(wrapped).chain(unwrapped_then_deleted)
    }

    pub fn changed_objects(&self) -> impl Iterator<Item = (&Object, Option<&Object>)> {
        // Iterator over ((ObjectId, new version), Option<old version>)
        match &self.effects {
            TransactionEffects::V1(v1) => {
                v1.changed_objects().iter().filter_map(|(id, change)| {
                    match (
                        &change.input_state,
                        &change.output_state,
                        &change.id_operation,
                    ) {
                        // Created Objects
                        (ObjectIn::NotExist, ObjectOut::ObjectWrite(_), IDOperation::Created) => {
                            Some(((id, &v1.lamport_version), None))
                        }
                        (
                            ObjectIn::NotExist,
                            ObjectOut::PackageWrite((version, _)),
                            IDOperation::Created,
                        ) => Some(((id, version), None)),

                        // Unwrapped Objects
                        (ObjectIn::NotExist, ObjectOut::ObjectWrite(_), IDOperation::None) => {
                            Some(((id, &v1.lamport_version), None))
                        }

                        // Mutated Objects
                        (ObjectIn::Exist(((old_version, _), _)), ObjectOut::ObjectWrite(_), _) => {
                            Some(((id, &v1.lamport_version), Some(old_version)))
                        }
                        (
                            ObjectIn::Exist(((old_version, _), _)),
                            ObjectOut::PackageWrite((version, _)),
                            _,
                        ) => Some(((id, version), Some(old_version))),

                        _ => None,
                    }
                })
            }
        }
        // Lookup Objects in output Objects as well as old versions for mutated objects
        .map(|((id, version), old_version)| {
            let object = self
                .output_objects
                .iter()
                .find(|o| &o.id() == id && &o.version() == version)
                .expect("changed objects should show up in output objects");

            let old_object = old_version.map(|old_version| {
                self.input_objects
                    .iter()
                    .find(|o| &o.id() == id && &o.version() == old_version)
                    .expect("mutated objects should have a previous version in input objects")
            });

            (object, old_object)
        })
    }

    pub fn created_objects(&self) -> impl Iterator<Item = &Object> {
        // Iterator over (ObjectId, version) for created objects
        match &self.effects {
            TransactionEffects::V1(v1) => {
                v1.changed_objects().iter().filter_map(|(id, change)| {
                    match (
                        &change.input_state,
                        &change.output_state,
                        &change.id_operation,
                    ) {
                        // Created Objects
                        (ObjectIn::NotExist, ObjectOut::ObjectWrite(_), IDOperation::Created) => {
                            Some((id, &v1.lamport_version))
                        }
                        (
                            ObjectIn::NotExist,
                            ObjectOut::PackageWrite((version, _)),
                            IDOperation::Created,
                        ) => Some((id, version)),

                        _ => None,
                    }
                })
            }
        }
        // Lookup Objects in output Objects as well as old versions for mutated objects
        .map(|(id, version)| {
            self.output_objects
                .iter()
                .find(|o| &o.id() == id && &o.version() == version)
                .expect("created objects should show up in output objects")
        })
    }
}

impl BackingPackageStore for CheckpointData {
    fn get_package_object(
        &self,
        package_id: &crate::base_types::ObjectID,
    ) -> crate::error::IotaResult<Option<crate::storage::PackageObject>> {
        self.transactions
            .iter()
            .flat_map(|transaction| transaction.output_objects.iter())
            .find(|object| object.is_package() && &object.id() == package_id)
            .cloned()
            .map(crate::storage::PackageObject::new)
            .pipe(Ok)
    }
}
