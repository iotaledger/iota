// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{
    ObjectId, ObjectReference, TransactionEffects, TransactionEvents, TransactionKind,
    checkpoint::CheckpointContents,
};
use serde::{Deserialize, Serialize};
use tap::Pipe;

use crate::{
    base_types::ExecutionData,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    iota_system_state::{IotaSystemStateTrait, get_iota_system_state},
    messages_checkpoint::CertifiedCheckpointSummary,
    object::Object,
    storage::{BackingPackageStore, EpochInfo, error::Error as StorageError},
    transaction::{TransactionAPI, TransactionEnvelope},
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
                latest_live_objects.remove(&obj_ref.object_id);
            }
        }
        latest_live_objects.into_values().collect()
    }

    // returns the object refs that are eventually deleted or wrapped in the current
    // checkpoint
    pub fn eventually_removed_object_refs_post_version(&self) -> Vec<ObjectReference> {
        let mut eventually_removed_object_refs = BTreeMap::new();
        for tx in self.transactions.iter() {
            for obj_ref in tx.removed_object_refs_post_version() {
                eventually_removed_object_refs.insert(obj_ref.object_id, obj_ref);
            }
            for obj in tx.output_objects.iter() {
                eventually_removed_object_refs.remove(&(obj.id()));
            }
        }
        eventually_removed_object_refs.into_values().collect()
    }

    /// Checks every transaction's objects and events against the digests its
    /// effects record, so the results can be committed without re-executing
    /// them.
    ///
    /// See [`CheckpointTransaction::verify_payload_digests`] for what this
    /// does and does not establish; in particular the caller is still
    /// responsible for verifying the summary's authority signatures and its
    /// `contents_digest`.
    pub fn verify_payload_digests(&self) -> Result<(), StorageError> {
        self.transactions
            .iter()
            .try_for_each(CheckpointTransaction::verify_payload_digests)
    }

    pub fn all_objects(&self) -> Vec<&Object> {
        self.transactions
            .iter()
            .flat_map(|tx| &tx.input_objects)
            .chain(self.transactions.iter().flat_map(|tx| &tx.output_objects))
            .collect()
    }

    /// The transaction that closes this checkpoint's epoch — the `AdvanceEpoch`
    /// / `advance_epoch_safe_mode` transaction. `None` if this isn't an
    /// epoch-boundary checkpoint (genesis included), or its last transaction
    /// unexpectedly isn't an end-of-epoch transaction.
    pub fn end_of_epoch_transaction(&self) -> Option<&CheckpointTransaction> {
        // Guard: only epoch-boundary checkpoints carry a closing tx — bail otherwise.
        self.checkpoint_summary.end_of_epoch_data.as_ref()?;
        // The epoch-change tx is always ordered last, after every user tx;
        // verify rather than assume, since callers treat `None` as a hard error.
        let transaction = self.transactions.last()?;
        transaction
            .transaction
            .transaction()
            .is_end_of_epoch_tx()
            .then_some(transaction)
    }

    /// Returns the epoch boundary information for this checkpoint, paired
    /// with the events of the transaction that produced this epoch's start
    /// system state (`EndOfEpoch` for non-genesis checkpoints, `Genesis`
    /// for checkpoint 0).
    /// Returns `None` for non-epoch-boundary checkpoints.
    pub fn epoch_info(
        &self,
    ) -> Result<Option<(EpochInfo, Option<TransactionEvents>)>, StorageError> {
        // If there is no end of epoch data, return None, except for checkpoint 0
        if self.checkpoint_summary.end_of_epoch_data.is_none()
            && self.checkpoint_summary.sequence_number != 0
        {
            return Ok(None);
        }

        let (start_checkpoint, transaction) = if self.checkpoint_summary.sequence_number != 0 {
            let Some(transaction) = self.end_of_epoch_transaction() else {
                return Err(StorageError::custom(format!(
                    "Failed to get end of epoch transaction in checkpoint {} with EndOfEpochData",
                    self.checkpoint_summary.sequence_number,
                )));
            };
            (self.checkpoint_summary.sequence_number + 1, transaction)
        } else {
            // For checkpoint 0, we look for the genesis transaction
            let Some(transaction) = self.transactions.iter().find(|tx| {
                matches!(
                    tx.transaction.transaction().kind(),
                    TransactionKind::Genesis(_)
                )
            }) else {
                return Err(StorageError::custom(format!(
                    "Failed to get genesis transaction in checkpoint {}",
                    self.checkpoint_summary.sequence_number,
                )));
            };
            (0, transaction)
        };

        let system_state =
            get_iota_system_state(&transaction.output_objects.as_slice()).map_err(|e| {
                StorageError::custom(format!(
                    "Failed to find system state object output from end of epoch or genesis transaction: {e}"
                ))
            })?;

        Ok(Some((
            EpochInfo {
                epoch: system_state.epoch(),
                protocol_version: system_state.protocol_version(),
                start_timestamp_ms: system_state.epoch_start_timestamp_ms(),
                end_timestamp_ms: None,
                start_checkpoint,
                end_checkpoint: None,
                reference_gas_price: system_state.reference_gas_price(),
                system_state,
            },
            transaction.events.clone(),
        )))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointTransaction {
    /// The input transaction
    pub transaction: TransactionEnvelope,
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
        // Since each object ID can only show up once in the input_objects, we can just
        // use the ids of deleted and wrapped objects to lookup the object in
        // the input_objects.
        self.effects
            .all_removed_objects()
            .into_iter() // Use id and version to lookup in input Objects
            .map(|(object_ref, _)| {
                self.input_objects
                    .iter()
                    .find(|o| o.id() == object_ref.object_id)
                    .expect("all removed objects should show up in input objects")
            })
    }

    pub fn removed_object_refs_post_version(&self) -> impl Iterator<Item = ObjectReference> {
        let deleted = self.effects.deleted().into_iter();
        let wrapped = self.effects.wrapped().into_iter();
        let unwrapped_then_deleted = self.effects.unwrapped_then_deleted().into_iter();
        deleted.chain(wrapped).chain(unwrapped_then_deleted)
    }

    pub fn changed_objects(&self) -> impl Iterator<Item = (&Object, Option<&Object>)> {
        self.effects
            .all_changed_objects()
            .into_iter()
            .map(|(changed, _)| {
                let object_id = changed.reference.object_id;
                let object = self
                    .output_objects
                    .iter()
                    .find(|o| o.id() == object_id)
                    .expect("changed objects should show up in output objects");

                let old_object = self.input_objects.iter().find(|o| o.id() == object_id);

                (object, old_object)
            })
    }

    pub fn created_objects(&self) -> impl Iterator<Item = &Object> {
        // Iterator over (ObjectId, version) for created objects
        self.effects
            .created()
            .into_iter()
            // Lookup Objects in output Objects as well as old versions for mutated objects
            .map(|created| {
                let object_ref = created.reference;
                self.output_objects
                    .iter()
                    .find(|o| o.id() == object_ref.object_id && o.version() == object_ref.version)
                    .expect("created objects should show up in output objects")
            })
    }

    pub fn execution_data(&self) -> ExecutionData {
        ExecutionData {
            transaction: self.transaction.clone(),
            effects: self.effects.clone(),
        }
    }

    /// Checks this transaction's objects and events against the digests its
    /// effects record for them.
    ///
    /// The caller must separately have verified the effects themselves, by
    /// checking the checkpoint's `contents_digest` against a summary carrying a
    /// quorum of authority signatures. Only then do the digests read here mean
    /// anything.
    ///
    /// Both directions are checked: a payload whose contents do not hash to the
    /// recorded digest is rejected, and so is one the effects record but the
    /// checkpoint does not carry. Anything less would let an object reach the
    /// store unchecked.
    pub fn verify_payload_digests(&self) -> Result<(), StorageError> {
        let tx_digest = self.effects.transaction_digest();

        let written: BTreeMap<ObjectId, ObjectReference> = self
            .effects
            .all_changed_objects()
            .into_iter()
            .map(|(owned, _)| (owned.reference.object_id, owned.reference))
            .collect();

        if written.len() != self.output_objects.len() {
            return Err(StorageError::custom(format!(
                "transaction {tx_digest} carries {} output objects but its effects record {} \
                 writes",
                self.output_objects.len(),
                written.len()
            )));
        }

        for object in &self.output_objects {
            let Some(expected) = written.get(&object.id()) else {
                return Err(StorageError::custom(format!(
                    "transaction {tx_digest} carries output object {} which its effects do not \
                     record as written",
                    object.id()
                )));
            };
            // Recomputes the object's digest from its contents, which is what
            // makes the object itself trustworthy rather than just its id.
            let actual = object.object_ref();
            if actual.version != expected.version || actual.digest != expected.digest {
                return Err(StorageError::custom(format!(
                    "object mismatch for {} in transaction {tx_digest}: effects record version \
                     {} digest {}, contents are version {} digest {}",
                    object.id(),
                    expected.version,
                    expected.digest,
                    actual.version,
                    actual.digest
                )));
            }
        }

        // Every version the transaction superseded is relocated out of the live
        // table when its results are committed, using the pre-image the
        // checkpoint carries as an input object. Effects record the digest each
        // of those versions had, so a pre-image is checked the same way an
        // output object is: a missing one would be dropped rather than
        // relocated, and a substituted one would reach the store unchecked.
        let pre_images: BTreeMap<ObjectId, &Object> = self
            .input_objects
            .iter()
            .map(|object| (object.id(), object))
            .collect();
        for old in self.effects.old_object_metadata() {
            let expected = old.reference;
            let Some(object) = pre_images.get(&expected.object_id) else {
                return Err(StorageError::custom(format!(
                    "transaction {tx_digest} supersedes version {} of object {} but the \
                     checkpoint carries no input object for it",
                    expected.version, expected.object_id
                )));
            };
            let actual = object.object_ref();
            if actual.version != expected.version || actual.digest != expected.digest {
                return Err(StorageError::custom(format!(
                    "input object mismatch for {} in transaction {tx_digest}: effects record \
                     version {} digest {}, contents are version {} digest {}",
                    expected.object_id,
                    expected.version,
                    expected.digest,
                    actual.version,
                    actual.digest
                )));
            }
        }

        match (self.effects.events_digest(), &self.events) {
            (Some(expected), Some(events)) => {
                let actual = events.digest();
                if actual != *expected {
                    return Err(StorageError::custom(format!(
                        "events digest mismatch for transaction {tx_digest}: effects record \
                         {expected}, contents hash to {actual}"
                    )));
                }
            }
            (Some(_), None) => {
                return Err(StorageError::custom(format!(
                    "transaction {tx_digest} declares events but the checkpoint carries none"
                )));
            }
            (None, Some(events)) if !events.is_empty() => {
                return Err(StorageError::custom(format!(
                    "transaction {tx_digest} carries events but its effects declare none"
                )));
            }
            _ => {}
        }

        Ok(())
    }
}

impl BackingPackageStore for CheckpointData {
    fn get_package_object(
        &self,
        package_id: &ObjectId,
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
