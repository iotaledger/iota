// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use iota_common::debug_fatal;
use iota_sdk_types::{
    ObjectId, ObjectReference, ObjectVersion, Owner, TransactionEffects, TransactionEvents, Version,
};
use iota_types::{
    base_types::VersionDigest,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    full_checkpoint_content::CheckpointTransaction,
    inner_temporary_store::{InnerTemporaryStore, WrittenObjects},
    object::{Object, ObjectSet},
    storage::{MarkerValue, ObjectKey},
    transaction::{TransactionAPI, VerifiedTransaction},
};

use crate::authority::AuthorityMetrics;

/// TransactionOutputs
pub struct TransactionOutputs {
    pub transaction: Arc<VerifiedTransaction>,
    pub effects: TransactionEffects,
    pub events: TransactionEvents,
    /// Pre-images of the versions this transaction superseded, carried in
    /// memory so checkpoint commit can relocate them into the historic
    /// bucket in the same atomic batch, without reading them back.
    pub superseded: Vec<(ObjectKey, Object)>,

    pub markers: Vec<(ObjectKey, MarkerValue)>,
    pub wrapped: Vec<ObjectKey>,
    pub deleted: Vec<ObjectKey>,
    pub live_object_markers_to_delete: Vec<ObjectReference>,
    pub new_live_object_markers_to_init: Vec<ObjectReference>,
    pub written: WrittenObjects,
}

impl TransactionOutputs {
    // Convert InnerTemporaryStore + Effects into the exact set of updates to the
    // store
    pub fn build_transaction_outputs(
        transaction: VerifiedTransaction,
        effects: TransactionEffects,
        inner_temporary_store: InnerTemporaryStore,
        read_objects: ObjectSet,
        metrics: &AuthorityMetrics,
    ) -> TransactionOutputs {
        let InnerTemporaryStore {
            input_objects,
            mutable_inputs,
            written,
            events,
            loaded_runtime_objects: _,
            binary_config: _,
            runtime_packages_loaded_from_db: _,
            lamport_version,
        } = inner_temporary_store;

        let tx_digest = *transaction.digest();
        let modified_at_versions = effects.modified_at_versions();

        let updates = derive_store_updates(
            &transaction,
            &effects,
            &written,
            mutable_inputs,
            |id| {
                input_objects
                    .get(id)
                    .is_some_and(|object| object.is_shared())
            },
            lamport_version,
        );

        let mut capture_misses = 0;
        let superseded = build_superseded_counting(
            &modified_at_versions,
            &input_objects,
            &read_objects,
            &mut capture_misses,
        );
        if capture_misses > 0 {
            // A miss loses no data — the same set drives both the bucket
            // insert and the live delete, so the version simply stays in the
            // live table — but relocation then stops for that shape of
            // object, which only a crash in a test build makes visible.
            debug_fatal!(
                "{capture_misses} of the versions {tx_digest} superseded have no pre-image among \
                 its input objects or the objects it read"
            );
        }
        metrics
            .superseded_capture_misses
            .inc_by(capture_misses as u64);

        TransactionOutputs {
            transaction: Arc::new(transaction),
            effects,
            events,
            superseded,
            markers: updates.markers,
            wrapped: updates.wrapped,
            deleted: updates.deleted,
            live_object_markers_to_delete: updates.live_object_markers_to_delete,
            new_live_object_markers_to_init: updates.new_live_object_markers_to_init,
            written,
        }
    }

    /// Converts the results a checkpoint carries for one transaction into the
    /// exact set of updates to the store, without executing it.
    ///
    /// The caller must already have checked the transaction's payloads against
    /// the digests the checkpoint commits to, with
    /// [`CheckpointData::verify_payload_digests`](iota_types::full_checkpoint_content::CheckpointData::verify_payload_digests);
    /// the effects and objects given here are trusted.
    pub fn build_from_checkpoint_transaction(tx: &CheckpointTransaction) -> TransactionOutputs {
        let transaction = VerifiedTransaction::new_unchecked(tx.transaction.clone());
        let effects = tx.effects.clone();
        let lamport_version = effects.lamport_version();

        // The version, digest and owner every changed object had before the
        // transaction ran, which is what execution records as `mutable_inputs`.
        // Objects that did not exist beforehand are absent, matching the input
        // objects execution would have loaded.
        let inputs: BTreeMap<ObjectId, (VersionDigest, Owner)> = effects
            .old_object_metadata()
            .into_iter()
            .map(|old| {
                (
                    old.reference.object_id,
                    ((old.reference.version, old.reference.digest), old.owner),
                )
            })
            .collect();

        let shared_inputs: HashSet<ObjectId> = inputs
            .iter()
            .filter_map(|(id, (_, owner))| owner.is_shared().then_some(*id))
            .collect();

        let written: WrittenObjects = tx
            .output_objects
            .iter()
            .map(|object| (object.id(), object.clone()))
            .collect();

        // A transaction that emitted no events has no events blob in checkpoint
        // data, where the store keeps an empty one.
        let events = tx.events.clone().unwrap_or_default();

        let updates = derive_store_updates(
            &transaction,
            &effects,
            &written,
            inputs,
            |id| shared_inputs.contains(id),
            lamport_version,
        );

        // Checkpoint data carries one input object per modified version — the
        // same set the superseded pre-images are keyed by — so unlike
        // execution there is nothing to fall back to and nothing to miss.
        // `verify_payload_digests` has already rejected any checkpoint whose
        // input objects don't cover that set.
        let pre_images: BTreeMap<ObjectKey, &Object> = tx
            .input_objects
            .iter()
            .map(|object| (ObjectKey(object.id(), object.version()), object))
            .collect();
        let superseded = effects
            .modified_at_versions()
            .into_iter()
            .map(|modified| {
                let key = ObjectKey(modified.object_id, modified.version);
                let pre_image = pre_images
                    .get(&key)
                    .expect("verified checkpoint data carries every superseded version");
                (key, (*pre_image).clone())
            })
            .collect();

        TransactionOutputs {
            transaction: Arc::new(transaction),
            effects,
            events,
            superseded,
            markers: updates.markers,
            wrapped: updates.wrapped,
            deleted: updates.deleted,
            live_object_markers_to_delete: updates.live_object_markers_to_delete,
            new_live_object_markers_to_init: updates.new_live_object_markers_to_init,
            written,
        }
    }
}

/// Pre-images of the object versions a transaction superseded, keyed by
/// the version each pre-image belonged to. Every modified version whose
/// pre-image is in neither source is counted into `capture_misses` and left
/// out, so the gap is visible instead of silently dropped.
///
/// A modified object's pre-image is normally in `input_objects`. Objects
/// loaded at runtime — dynamic fields, in particular — never appear there,
/// so a lookup that misses in `input_objects` falls back to `read_objects`,
/// which tracks every object loaded during execution.
fn build_superseded_counting(
    modified_at: &[ObjectVersion],
    input_objects: &BTreeMap<ObjectId, Object>,
    read_objects: &ObjectSet,
    capture_misses: &mut usize,
) -> Vec<(ObjectKey, Object)> {
    modified_at
        .iter()
        .filter_map(|modified| {
            let (id, version) = (&modified.object_id, modified.version);
            let pre_image = input_objects
                .get(id)
                .filter(|object| object.version() == version)
                .or_else(|| read_objects.get(&ObjectKey(*id, version)));
            match pre_image {
                Some(object) => Some((ObjectKey(*id, version), object.clone())),
                None => {
                    *capture_misses += 1;
                    None
                }
            }
        })
        .collect()
}

/// [`build_superseded_counting`] with the miss count discarded, so a test
/// that only cares about the captured set doesn't have to thread a counter
/// through it.
#[cfg(test)]
fn build_superseded(
    modified_at: &[ObjectVersion],
    input_objects: &BTreeMap<ObjectId, Object>,
    read_objects: &ObjectSet,
) -> Vec<(ObjectKey, Object)> {
    let mut capture_misses = 0;
    build_superseded_counting(
        modified_at,
        input_objects,
        read_objects,
        &mut capture_misses,
    )
}

/// The parts of a [`TransactionOutputs`] that follow from a transaction's
/// effects rather than from execution's temporary store.
struct StoreUpdates {
    markers: Vec<(ObjectKey, MarkerValue)>,
    wrapped: Vec<ObjectKey>,
    deleted: Vec<ObjectKey>,
    live_object_markers_to_delete: Vec<ObjectReference>,
    new_live_object_markers_to_init: Vec<ObjectReference>,
}

/// Derives the updates a transaction makes to the store.
///
/// `input_was_shared` answers whether an object was shared before the
/// transaction ran, which decides whether its deletion is marked as owned or
/// shared. A caller that executed the transaction answers from the input
/// objects it loaded; a caller applying results from a checkpoint answers from
/// the input state recorded in the effects.
fn derive_store_updates<F>(
    transaction: &VerifiedTransaction,
    effects: &TransactionEffects,
    written: &WrittenObjects,
    mutable_inputs: BTreeMap<ObjectId, (VersionDigest, Owner)>,
    input_was_shared: F,
    lamport_version: Version,
) -> StoreUpdates
where
    F: Fn(&ObjectId) -> bool,
{
    let tx_digest = *transaction.digest();

    let deleted: HashMap<_, _> = effects.all_tombstones().into_iter().collect();

    // Get the actual set of objects that have been received -- any received
    // object will show up in the modified-at set.
    let modified_at: HashSet<_> = effects
        .modified_at_versions()
        .into_iter()
        .map(|modified| (modified.object_id, modified.version))
        .collect();
    let possible_to_receive = transaction.transaction().receiving_objects();
    let received_objects = possible_to_receive
        .into_iter()
        .filter(|obj_ref| modified_at.contains(&(obj_ref.object_id, obj_ref.version)));

    // We record any received or deleted objects since they could be pruned, and
    // smear shared object deletions in the marker table. For deleted
    // entries in the marker table we need to make sure we don't
    // accidentally overwrite entries.
    let markers: Vec<_> = {
        let received = received_objects
            .clone()
            .map(|objref| (ObjectKey::from(objref), MarkerValue::Received));

        let deleted = deleted.into_iter().map(|(object_id, version)| {
            let object_key = ObjectKey(object_id, version);
            if input_was_shared(&object_id) {
                (object_key, MarkerValue::SharedDeleted(tx_digest))
            } else {
                (object_key, MarkerValue::OwnedDeleted)
            }
        });

        // We "smear" shared deleted objects in the marker table to allow for proper
        // sequencing of transactions that are submitted after the deletion
        // of the shared object. NB: that we do _not_ smear shared objects
        // that were taken immutably in the transaction.
        let smeared_objects = effects.deleted_mutably_accessed_shared_objects();
        let shared_smears = smeared_objects.into_iter().map(move |object_id| {
            (
                ObjectKey(object_id, lamport_version),
                MarkerValue::SharedDeleted(tx_digest),
            )
        });

        received.chain(deleted).chain(shared_smears).collect()
    };

    let live_object_markers_to_delete: Vec<_> = mutable_inputs
        .into_iter()
        .filter_map(|(id, ((version, digest), owner))| {
            owner
                .is_address()
                .then_some(ObjectReference::new(id, version, digest))
        })
        .chain(received_objects)
        .collect();

    let new_live_object_markers_to_init: Vec<_> = written
        .values()
        .filter_map(|new_object| {
            if new_object.is_address_owned() {
                Some(new_object.object_ref())
            } else {
                None
            }
        })
        .collect();

    let deleted = effects
        .deleted()
        .into_iter()
        .chain(effects.unwrapped_then_deleted())
        .map(ObjectKey::from)
        .collect();

    let wrapped = effects.wrapped().into_iter().map(ObjectKey::from).collect();

    StoreUpdates {
        markers,
        wrapped,
        deleted,
        live_object_markers_to_delete,
        new_live_object_markers_to_init,
    }
}

#[cfg(test)]
#[path = "unit_tests/transaction_outputs_tests.rs"]
pub(crate) mod tests;
