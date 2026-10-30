// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use iota_sdk_types::{
    Address, ObjectId, ObjectReference, ObjectVersion, OwnedObjectReference, Owner,
    TransactionEffects,
};
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    executable_transaction::{
        CertificateProof, ExecutableTransaction, VerifiedExecutableTransaction,
    },
    full_checkpoint_content::CheckpointTransaction,
    inner_temporary_store::InnerTemporaryStore,
    object::{Object, ObjectSet},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    storage::{MarkerValue, ObjectKey},
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_PUBLISH, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI,
        VerifiedTransaction,
    },
};

use super::{build_superseded, build_superseded_counting};
use crate::{
    authority::{
        AuthorityState,
        authority_per_epoch_store::TxLockGuard,
        authority_test_utils::{init_state_with_ids, init_transfer_transaction},
        authority_tests::{build_programmable_transaction, execute_programmable_transaction},
        move_integration_tests::build_and_publish_test_package_with_upgrade_cap,
        test_authority_builder::TestAuthorityBuilder,
    },
    move_call,
    transaction_outputs::TransactionOutputs,
};

/// A version superseded by a mutation of a runtime-loaded object, such as a
/// dynamic field, is captured from the read objects, as it is never in
/// `input_objects`.
#[test]
fn test_superseded_captures_runtime_loaded_objects() {
    let child = Object::immutable_with_id_for_testing(ObjectId::random());
    let child_key = ObjectKey(child.id(), child.version());

    let input_objects = BTreeMap::new();
    let mut read_objects = ObjectSet::default();
    read_objects.insert(child.clone());
    let modified_at = vec![ObjectVersion::new(child.id(), child.version())];

    let superseded = build_superseded(&modified_at, &input_objects, &read_objects);

    assert_eq!(superseded, vec![(child_key, child)]);
}

/// When a key is present in both sources, the pre-image comes from
/// `input_objects`; `read_objects` is only a fallback.
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
}

/// Executes a transfer without committing it, and returns the authority it ran
/// on, the outputs execution produced, and the checkpoint data a node would
/// receive for the same transaction.
async fn execute_transfer_both_ways_on() -> (
    Arc<AuthorityState>,
    TransactionOutputs,
    CheckpointTransaction,
) {
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
    let (inner_store, effects, error, read_objects) = authority
        .prepare_transaction_for_benchmark(&executable, input_objects, &epoch_store)
        .unwrap();
    assert!(error.is_none(), "the transfer must execute successfully");

    let checkpoint_tx =
        checkpoint_transaction_for(&authority, &transaction, &effects, &inner_store);
    let executed = TransactionOutputs::build_transaction_outputs(
        transaction,
        effects,
        inner_store,
        read_objects,
        &authority.metrics,
    );

    (authority, executed, checkpoint_tx)
}

/// Shapes the checkpoint data the way the archive shapes it:
/// `get_transaction_input_objects` selects by `modified_at_versions` and reads
/// the pre-execution objects from the store, and
/// `get_transaction_output_objects` selects by the writes the effects record.
///
/// Reading the inputs back from the store is what makes the comparison mean
/// anything. Taking the execution store's own map instead would hand the
/// applying path the same objects execution derived its answer from, and a
/// transaction that receives an object would then agree by construction.
fn checkpoint_transaction_for(
    authority: &AuthorityState,
    transaction: &VerifiedTransaction,
    effects: &TransactionEffects,
    inner_store: &InnerTemporaryStore,
) -> CheckpointTransaction {
    let input_objects = effects
        .modified_at_versions()
        .into_iter()
        .map(|modified| {
            authority
                .get_object_store()
                .get_object_by_key(modified.object_id(), modified.version())
                .expect("the pre-execution object is still in the store")
        })
        .collect();
    let output_objects = effects
        .all_changed_objects()
        .into_iter()
        .map(|(owned, _)| {
            inner_store
                .written
                .get(&owned.reference().object_id)
                .expect("a recorded write must have been written")
                .clone()
        })
        .collect();
    CheckpointTransaction {
        transaction: transaction.clone().into_inner(),
        effects: effects.clone(),
        events: (!inner_store.events.is_empty()).then(|| inner_store.events.clone()),
        input_objects,
        output_objects,
    }
}

/// Runs a transaction that takes a `Receiving` argument, and returns both
/// constructors' results for it.
///
/// This is the one shape where the two paths disagree about which objects the
/// transaction took: execution reaches its list through the transaction's
/// declared inputs, which never name a received object, while the effects
/// record it among the objects modified. A transfer exercises neither side of
/// that, so it cannot show the difference.
async fn execute_receive_both_ways() -> (TransactionOutputs, CheckpointTransaction) {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let authority = TestAuthorityBuilder::new().build().await;
    let gas_object_id = ObjectId::random();
    authority.insert_genesis_object(Object::with_id_owner_for_testing(gas_object_id, sender));

    let (package, _) = build_and_publish_test_package_with_upgrade_cap(
        &authority,
        &sender,
        &sender_key,
        &gas_object_id,
        "tto",
        false,
    )
    .await;

    // Creates the parent and sends the child to it, so the child can be
    // received below. This one is committed; only the receive is held back.
    let effects = execute_programmable_transaction(
        &authority,
        &gas_object_id,
        &sender,
        &sender_key,
        {
            let mut builder = ProgrammableTransactionBuilder::new();
            move_call! { builder, (package.object_id)::M1::start() };
            builder.finish()
        },
        TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
    )
    .await
    .unwrap();
    let (parent, child) = parent_and_child(effects.created());

    let transaction = build_programmable_transaction(
        &authority,
        &gas_object_id,
        &sender,
        &sender_key,
        {
            let mut builder = ProgrammableTransactionBuilder::new();
            let parent = builder.obj(CallArg::ImmutableOrOwned(parent)).unwrap();
            let child = builder.obj(CallArg::Receiving(child)).unwrap();
            move_call! { builder, (package.object_id)::M1::receiver(parent, child) };
            builder.finish()
        },
        TEST_ONLY_GAS_UNIT_FOR_PUBLISH,
    )
    .await
    .unwrap();
    let transaction = VerifiedTransaction::new_unchecked(transaction);

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
            // Receiving a child touches no shared objects.
            Vec::new(),
            &epoch_store,
        )
        .unwrap();
    let (inner_store, effects, error, read_objects) = authority
        .prepare_transaction_for_benchmark(&executable, input_objects, &epoch_store)
        .unwrap();
    assert!(error.is_none(), "the receive must execute successfully");
    assert!(
        !transaction.transaction().receiving_objects().is_empty(),
        "the fixture must carry a receiving argument, or it proves nothing"
    );

    let checkpoint_tx =
        checkpoint_transaction_for(&authority, &transaction, &effects, &inner_store);
    let executed = TransactionOutputs::build_transaction_outputs(
        transaction,
        effects,
        inner_store,
        read_objects,
        &authority.metrics,
    );

    (executed, checkpoint_tx)
}

/// Picks the created object owned by another created object, and that owner.
fn parent_and_child(created: Vec<OwnedObjectReference>) -> (ObjectReference, ObjectReference) {
    let created_ids: HashSet<_> = created
        .iter()
        .map(|created| created.reference().object_id)
        .collect();
    let (child, parent_id) = created
        .iter()
        .find_map(|created| match created.owner() {
            Owner::Address(owner) if created_ids.contains(&ObjectId::from(*owner)) => {
                Some((*created.reference(), ObjectId::from(*owner)))
            }
            _ => None,
        })
        .expect("`start` creates a child owned by the parent it creates");
    let parent = created
        .iter()
        .find(|created| created.reference().object_id == parent_id)
        .expect("the owner is one of the created objects");
    (*parent.reference(), child)
}

/// The two constructors' results for a transfer, without the authority.
async fn execute_transfer_both_ways() -> (TransactionOutputs, CheckpointTransaction) {
    let (_, executed, checkpoint_tx) = execute_transfer_both_ways_on().await;
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

    assert_applied_matches_executed(&checkpoint_tx, &executed);
}

/// The same equality for a transaction that receives an object. Execution
/// derives its input list from the transaction's declared inputs, which never
/// name a received object, while the applying path derives it from the
/// effects, which do record one. Without the filter that reconciles them, the
/// received object is listed twice and this fails.
#[tokio::test]
async fn build_from_checkpoint_transaction_matches_execution_for_a_received_object() {
    let (executed, checkpoint_tx) = execute_receive_both_ways().await;

    assert_applied_matches_executed(&checkpoint_tx, &executed);
}

fn assert_applied_matches_executed(
    checkpoint_tx: &CheckpointTransaction,
    executed: &TransactionOutputs,
) {
    let applied = TransactionOutputs::build_from_checkpoint_transaction(checkpoint_tx);

    assert_eq!(applied.transaction.digest(), executed.transaction.digest());
    assert_eq!(applied.effects, executed.effects);
    assert_eq!(applied.events, executed.events);
    assert_eq!(applied.written, executed.written);
    assert_eq!(
        sorted_markers(applied.markers),
        sorted_markers(executed.markers.clone())
    );
    assert_eq!(sorted(applied.wrapped), sorted(executed.wrapped.clone()));
    assert_eq!(sorted(applied.deleted), sorted(executed.deleted.clone()));
    // Execution reaches this list through the transaction's declared inputs,
    // the applying path through the effects' record of what was modified.
    // The two disagree about received objects, which the effects record and
    // the declared inputs do not, so a duplicate here is the shape that
    // divergence takes.
    let mut seen = std::collections::HashSet::new();
    assert!(
        applied
            .live_object_markers_to_delete
            .iter()
            .all(|reference| seen.insert(*reference)),
        "the applying path listed a live object marker twice"
    );

    assert_eq!(
        sorted(applied.live_object_markers_to_delete),
        sorted(executed.live_object_markers_to_delete.clone()),
        "the input owner is resolved from the effects rather than the loaded \
         input objects, so this is where the two constructors can diverge"
    );
    assert_eq!(
        sorted(applied.new_live_object_markers_to_init),
        sorted(executed.new_live_object_markers_to_init.clone())
    );
    assert_eq!(
        sorted_superseded(applied.superseded),
        sorted_superseded(executed.superseded.clone()),
        "the pre-images come from the checkpoint's input objects rather than \
         from execution, so this is where the two constructors can diverge"
    );
}

/// Well-formed checkpoint data must verify.
#[tokio::test]
async fn verify_payload_digests_accepts_untampered_data() {
    let (_, checkpoint_tx) = execute_transfer_both_ways().await;

    checkpoint_tx
        .verify_payload_digests()
        .expect("data produced by execution must verify against its own effects");
}

/// An output object whose contents do not hash to the digest its effects record
/// must be rejected. This is the check that lets a node trust objects it did
/// not compute itself.
#[tokio::test]
async fn verify_payload_digests_rejects_tampered_object() {
    let (_, mut checkpoint_tx) = execute_transfer_both_ways().await;
    let mut tampered = checkpoint_tx.output_objects[0].as_inner().clone();
    tampered.storage_rebate += 1;
    checkpoint_tx.output_objects[0] = tampered.into();

    let err = checkpoint_tx
        .verify_payload_digests()
        .expect_err("a tampered object must be rejected");
    assert!(
        format!("{err}").contains("object mismatch"),
        "the error must name the failure, got: {err}"
    );
}

/// An input object whose contents do not hash to the digest its effects record
/// must be rejected. The pre-images of superseded versions are relocated into
/// the historic buckets on commit, so an unchecked one would put fabricated
/// history in the store.
#[tokio::test]
async fn verify_payload_digests_rejects_tampered_input_object() {
    let (_, mut checkpoint_tx) = execute_transfer_both_ways().await;
    let mut tampered = checkpoint_tx.input_objects[0].as_inner().clone();
    tampered.storage_rebate += 1;
    checkpoint_tx.input_objects[0] = tampered.into();

    let err = checkpoint_tx
        .verify_payload_digests()
        .expect_err("a tampered input object must be rejected");
    assert!(
        format!("{err}").contains("input object mismatch"),
        "the error must name the failure, got: {err}"
    );
}

/// A superseded version whose pre-image the checkpoint omits must be rejected;
/// committing such a checkpoint would drop the version instead of relocating
/// it.
#[tokio::test]
async fn verify_payload_digests_rejects_missing_input_object() {
    let (_, mut checkpoint_tx) = execute_transfer_both_ways().await;
    checkpoint_tx
        .input_objects
        .pop()
        .expect("a transfer modifies at least one object");

    assert!(
        checkpoint_tx.verify_payload_digests().is_err(),
        "a checkpoint missing a superseded version's pre-image must be rejected"
    );
}

/// A checkpoint that repeats one output object in place of another must be
/// rejected. Counting the payloads against the number of recorded writes is
/// not enough on its own: a repeat keeps the count right while a write the
/// effects record goes missing, and the object it displaced would then never
/// reach the store.
#[tokio::test]
async fn verify_payload_digests_rejects_a_repeated_output_object() {
    let (_, mut checkpoint_tx) = execute_transfer_both_ways().await;
    assert!(
        checkpoint_tx.output_objects.len() > 1,
        "a transfer writes the gas coin and the transferred object"
    );

    // Stand the first object in for the last, keeping the count unchanged.
    let repeated = checkpoint_tx.output_objects[0].clone();
    *checkpoint_tx
        .output_objects
        .last_mut()
        .expect("checked above") = repeated;

    let err = checkpoint_tx
        .verify_payload_digests()
        .expect_err("a repeated output object must be rejected");
    assert!(
        format!("{err}").contains("more than once"),
        "the error must name the failure, got: {err}"
    );
}

/// An object the effects record as written but the checkpoint does not carry
/// must be rejected; applying such a checkpoint would silently drop a write.
#[tokio::test]
async fn verify_payload_digests_rejects_missing_output_object() {
    let (_, mut checkpoint_tx) = execute_transfer_both_ways().await;
    checkpoint_tx
        .output_objects
        .pop()
        .expect("a transfer writes at least one object");

    assert!(
        checkpoint_tx.verify_payload_digests().is_err(),
        "a checkpoint missing a recorded write must be rejected"
    );
}
