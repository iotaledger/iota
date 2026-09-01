// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{Address, ObjectId, ObjectVersion};
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    executable_transaction::{
        CertificateProof, ExecutableTransaction, VerifiedExecutableTransaction,
    },
    full_checkpoint_content::CheckpointTransaction,
    object::{Object, ObjectSet},
    storage::{MarkerValue, ObjectKey},
    transaction::TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
};

use super::{build_superseded, build_superseded_counting};
use crate::{
    authority::{
        authority_per_epoch_store::TxLockGuard,
        authority_test_utils::{init_state_with_ids, init_transfer_transaction},
    },
    transaction_outputs::TransactionOutputs,
};

/// A version superseded by a mutation of a runtime-loaded object — a
/// dynamic field — is captured from the tracked read objects. Such an
/// object never appears in `input_objects`, which is why sourcing the
/// pre-images from inputs alone silently dropped it.
#[test]
fn test_superseded_captures_runtime_loaded_objects() {
    let child = Object::immutable_with_id_for_testing(ObjectId::random());
    let child_key = ObjectKey(child.id(), child.version());

    // The tracked set holds it; the declared inputs do not.
    let input_objects = BTreeMap::new();
    let mut read_objects = ObjectSet::default();
    read_objects.insert(child.clone());
    let modified_at = vec![ObjectVersion::new(child.id(), child.version())];

    let superseded = build_superseded(&modified_at, &input_objects, &read_objects);

    assert_eq!(superseded, vec![(child_key, child)]);
}

/// When a key is present in both sources, the pre-image comes from
/// `input_objects` rather than `read_objects` — the declared input is
/// checked first, and the tracked read objects are only a fallback for
/// what the inputs don't cover.
#[test]
fn test_superseded_prefers_input_objects_over_read_objects() {
    let id = ObjectId::random();
    let from_input = Object::with_id_owner_gas_for_testing(id, Address::ZERO, 1);
    let from_read = Object::with_id_owner_gas_for_testing(id, Address::ZERO, 2);
    let key = ObjectKey(id, from_input.version());

    let mut input_objects = BTreeMap::new();
    input_objects.insert(id, from_input.clone());
    let mut read_objects = ObjectSet::default();
    read_objects.insert(from_read);
    let modified_at = vec![ObjectVersion::new(id, from_input.version())];

    let superseded = build_superseded(&modified_at, &input_objects, &read_objects);

    assert_eq!(superseded, vec![(key, from_input)]);
}

/// A modified version present in neither source is dropped rather than
/// guessed at, and counted so the gap is visible.
#[test]
fn test_superseded_counts_what_it_cannot_capture() {
    let missing = ObjectId::random();
    let mut misses = 0;

    let superseded = build_superseded_counting(
        &[ObjectVersion::new(missing, 7.into())],
        &BTreeMap::new(),
        &ObjectSet::default(),
        &mut misses,
    );

    assert!(superseded.is_empty());
    assert_eq!(misses, 1);

/// Executes a transfer without committing it, and returns the outputs
/// execution produced alongside the checkpoint data a node would receive for
/// the same transaction.
async fn execute_transfer_both_ways() -> (TransactionOutputs, CheckpointTransaction) {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (recipient, _): (_, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_object_id = ObjectId::random();
    let authority = init_state_with_ids(vec![(sender, object_id), (sender, gas_object_id)]).await;
    let rgp = authority.reference_gas_price_for_testing().unwrap();
    let object = authority.get_object(&object_id).unwrap();
    let gas_object = authority.get_object(&gas_object_id).unwrap();

    let transaction = init_transfer_transaction(
        &authority,
        sender,
        &sender_key,
        recipient,
        object.object_ref(),
        gas_object.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );

    let epoch_store = authority.load_epoch_store_one_call_per_task().clone();
    let executable =
        VerifiedExecutableTransaction::new_unchecked(ExecutableTransaction::new_from_data_and_sig(
            transaction.data().clone(),
            CertificateProof::Checkpoint(0, 0),
        ));
    let (input_objects, _) = authority
        .read_objects_for_execution(
            &TxLockGuard::guard_for_tests(),
            &executable,
            // A transfer touches no shared objects, so it needs no assignments.
            Vec::new(),
            &epoch_store,
        )
        .unwrap();
    let (inner_store, effects, error) = authority
        .prepare_transaction_for_benchmark(&executable, input_objects, &epoch_store)
        .unwrap();
    assert!(error.is_none(), "the transfer must execute successfully");

    // The same results a checkpoint would carry for this transaction.
    let checkpoint_tx = CheckpointTransaction {
        transaction: transaction.clone().into_inner(),
        effects: effects.clone(),
        events: (!inner_store.events.is_empty()).then(|| inner_store.events.clone()),
        input_objects: inner_store.input_objects.values().cloned().collect(),
        output_objects: inner_store.written.values().cloned().collect(),
    };

    let executed = TransactionOutputs::build_transaction_outputs(
        transaction,
        effects,
        inner_store,
        // A transfer modifies only objects it declares as inputs, so execution
        // recovers no pre-image from the objects it read.
        ObjectSet::default(),
        &authority.metrics,
    );

    (executed, checkpoint_tx)
}

fn sorted<T: Ord>(mut v: Vec<T>) -> Vec<T> {
    v.sort();
    v
}

/// `MarkerValue` is not `Ord`, so markers are ordered by their key alone. Keys
/// are unique within a transaction's markers, so this is a total order.
fn sorted_superseded(mut v: Vec<(ObjectKey, Object)>) -> Vec<(ObjectKey, Object)> {
    v.sort_by_key(|(key, _)| *key);
    v
}

fn sorted_markers(mut v: Vec<(ObjectKey, MarkerValue)>) -> Vec<(ObjectKey, MarkerValue)> {
    v.sort_by_key(|(key, _)| *key);
    v
}

/// Building `TransactionOutputs` from checkpoint data must produce exactly what
/// execution produces. Both paths feed the same `write_transaction_outputs`, so
/// this equality is what makes the applied state match the executed state.
#[tokio::test]
async fn build_from_checkpoint_transaction_matches_execution() {
    let (executed, checkpoint_tx) = execute_transfer_both_ways().await;

    let applied = TransactionOutputs::build_from_checkpoint_transaction(&checkpoint_tx);

    assert_eq!(applied.transaction.digest(), executed.transaction.digest());
    assert_eq!(applied.effects, executed.effects);
    assert_eq!(applied.events, executed.events);
    assert_eq!(applied.written, executed.written);
    assert_eq!(
        sorted_markers(applied.markers),
        sorted_markers(executed.markers)
    );
    assert_eq!(sorted(applied.wrapped), sorted(executed.wrapped));
    assert_eq!(sorted(applied.deleted), sorted(executed.deleted));
    assert_eq!(
        sorted(applied.live_object_markers_to_delete),
        sorted(executed.live_object_markers_to_delete),
        "the input owner is resolved from the effects rather than the loaded \
         input objects, so this is where the two constructors can diverge"
    );
    assert_eq!(
        sorted(applied.new_live_object_markers_to_init),
        sorted(executed.new_live_object_markers_to_init)
    );
    assert_eq!(
        sorted_superseded(applied.superseded),
        sorted_superseded(executed.superseded),
        "the pre-images come from the checkpoint's input objects rather than \
         from execution, so this is where the two constructors can diverge"
    );
