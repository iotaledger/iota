// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{time::Duration, vec};

use iota_config::node::AuthorityOverloadConfig;
use iota_sdk_types::{ObjectId, VersionAssignment};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::deterministic_random_account_key,
    digests::TransactionEffectsDigest,
    executable_transaction::VerifiedExecutableTransaction,
    object::Object,
    transaction::{CallArg, SharedObjectRef, VerifiedTransaction},
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    time::sleep,
};

use crate::{
    authority::{AuthorityState, authority_tests::init_state_with_objects},
    execution_scheduler::{
        ExecutionSchedulerAPI, PendingTransaction, execution_scheduler_impl::ExecutionScheduler,
    },
};

#[expect(clippy::disallowed_methods)] // allow unbounded_channel()
fn make_execution_scheduler(
    state: &AuthorityState,
) -> (ExecutionScheduler, UnboundedReceiver<PendingTransaction>) {
    // A standalone scheduler (not the authority's) so the test can observe its
    // output on rx_ready_transactions.
    let (tx_ready_transactions, rx_ready_transactions) = unbounded_channel();
    let execution_scheduler = ExecutionScheduler::new(
        state.get_object_cache_reader().clone(),
        state.get_transaction_cache_reader().clone(),
        tx_ready_transactions,
        state.metrics.clone(),
    );
    (execution_scheduler, rx_ready_transactions)
}

fn make_transaction(gas_object: Object, input: Vec<CallArg>) -> VerifiedExecutableTransaction {
    make_transaction_for_epoch(gas_object, input, 0)
}

fn make_transaction_for_epoch(
    gas_object: Object,
    input: Vec<CallArg>,
    epoch: u64,
) -> VerifiedExecutableTransaction {
    // Fake module/function/gas price: irrelevant to scheduling.
    let rgp = 100;
    let (sender, keypair) = deterministic_random_account_key();
    let transaction = TestTransactionBuilder::new(sender, gas_object.object_ref(), rgp)
        .move_call(ObjectId::FRAMEWORK, "counter", "assert_value", input)
        .build_and_sign(&keypair);
    VerifiedExecutableTransaction::new_system(
        VerifiedTransaction::new_unchecked(transaction),
        epoch,
    )
}

/// The new scheduler must hold a transaction whose owned input object is not
/// yet available, then release it once the object is written. This exercises
/// the `notify_read_input_objects` wait path (`execution_scheduler_impl.rs`),
/// which the e2e tests — whose inputs are always immediately available — never
/// reach.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_waits_for_missing_owned_input() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let owned_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);

    let state = init_state_with_objects(vec![gas_object.clone(), owned_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
    assert!(rx_ready_transactions.try_recv().is_err());

    // Reference the owned object at a version not present in the cache, so the
    // transaction cannot be ready until that version is written.
    let awaited_version = 2000.into();
    let mut owned_ref = owned_object.object_ref();
    owned_ref.version = awaited_version;
    let transaction = make_transaction(gas_object, vec![CallArg::ImmutableOrOwned(owned_ref)]);

    execution_scheduler.enqueue(vec![transaction.clone()], &state.epoch_store_for_testing());

    // The scheduler must NOT release the transaction while its input is missing.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_certificates(), 1);

    // Make the owned object available at the awaited version. The scheduler's
    // readiness check keys on (id, version) only, so a fresh object at that
    // version is enough to satisfy the input.
    let new_owned_object = Object::with_id_owner_version_for_testing(
        owned_object.id(),
        awaited_version,
        iota_sdk_types::Owner::Address(owner),
    );
    state
        .get_cache_writer()
        .write_object_entry_for_test(new_owned_object);

    // The scheduler now releases the transaction for execution.
    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), transaction.digest());

    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());

    // The pending gauge was released when the transaction was sent; the executing
    // gauge is held by the ready certificate's `ExecutingGuard` until execution
    // completes. Dropping it must return the count to 0 — pins the RAII gauge
    // accounting that feeds overload admission.
    assert_eq!(execution_scheduler.num_pending_certificates(), 1);
    drop(ready);
    assert_eq!(execution_scheduler.num_pending_certificates(), 0);
}

/// One object becoming available must release ALL transactions waiting on it.
/// This exercises the NotifyRead fan-out across the scheduler's independent
/// per-transaction tasks (each registers its own waiter on the shared input),
/// and confirms the pending/executing gauges return to 0 afterwards.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_releases_all_waiters_on_one_object() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_objects: Vec<Object> = (0..3)
        .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), owner))
        .collect();
    let shared_object = Object::shared_for_testing();
    let initial_shared_version = shared_object.version();

    let mut objects = gas_objects.clone();
    objects.push(shared_object.clone());
    let state = init_state_with_objects(objects).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // Three read-only transactions, each waiting on the same shared object at a
    // consensus-assigned version that is not yet available in the cache.
    let shared_version = 1000.into();
    let shared_arg = CallArg::Shared(SharedObjectRef::new(
        shared_object.id(),
        initial_shared_version,
        false,
    ));
    let mut txns = Vec::new();
    for gas in &gas_objects {
        let txn = make_transaction(gas.clone(), vec![shared_arg.clone()]);
        state
            .epoch_store_for_testing()
            .set_shared_object_versions_for_testing(
                txn.digest(),
                &[VersionAssignment::new(shared_object.id(), shared_version)],
            )
            .unwrap();
        execution_scheduler.enqueue(vec![txn.clone()], &state.epoch_store_for_testing());
        txns.push(txn);
    }

    // None are ready while the shared object version is missing.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_certificates(), txns.len());

    // Make the shared object available at the awaited version: all three release.
    let new_shared_object = Object::with_id_owner_version_for_testing(
        shared_object.id(),
        shared_version,
        iota_sdk_types::Owner::Shared(initial_shared_version),
    );
    state
        .get_cache_writer()
        .write_object_entry_for_test(new_shared_object);

    let mut ready = Vec::new();
    for _ in 0..txns.len() {
        ready.push(rx_ready_transactions.recv().await.unwrap());
    }
    let mut want: Vec<_> = txns.iter().map(|t| *t.digest()).collect();
    want.sort();
    let mut got: Vec<_> = ready.iter().map(|p| *p.transaction.digest()).collect();
    got.sort();
    assert_eq!(want, got);

    // All three are now executing; dropping them returns the gauge to 0.
    assert_eq!(execution_scheduler.num_pending_certificates(), txns.len());
    drop(ready);
    assert_eq!(execution_scheduler.num_pending_certificates(), 0);
}

/// When executing from a checkpoint the scheduler is handed a certified
/// `expected_effects_digest` that the execution driver uses to detect forks.
/// The scheduler must copy it verbatim into the `PendingTransaction` it emits
/// on the fast path (inputs already available). If it were dropped or defaulted
/// to `None`, fork detection would be silently disabled for every state-synced
/// checkpoint, with no metric or log to notice.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn scheduler_propagates_expected_effects_digest_fast_path() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // Inputs (gas + framework package) are already available, so the transaction
    // takes the fast path in `schedule_transaction`.
    let transaction = make_transaction(gas_object, vec![]);
    let expected = TransactionEffectsDigest::new([7; 32]);
    execution_scheduler.enqueue_with_expected_effects_digest(
        vec![(transaction.clone(), expected)],
        &state.epoch_store_for_testing(),
    );

    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), transaction.digest());
    assert_eq!(ready.expected_effects_digest, Some(expected));
}

/// The certified `expected_effects_digest` must also survive the `notify_read`
/// wait path — the scheduler sets it at a second call site, reached only when
/// an input is initially missing. A regression dropping it there would disable
/// fork detection for exactly the synced transactions whose inputs arrive late.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn scheduler_propagates_expected_effects_digest_wait_path() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let owned_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone(), owned_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    let awaited_version = 2000.into();
    let mut owned_ref = owned_object.object_ref();
    owned_ref.version = awaited_version;
    let transaction = make_transaction(gas_object, vec![CallArg::ImmutableOrOwned(owned_ref)]);

    let expected = TransactionEffectsDigest::new([9; 32]);
    execution_scheduler.enqueue_with_expected_effects_digest(
        vec![(transaction.clone(), expected)],
        &state.epoch_store_for_testing(),
    );

    // Parked on the missing input: nothing emitted yet.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());

    let new_owned_object = Object::with_id_owner_version_for_testing(
        owned_object.id(),
        awaited_version,
        iota_sdk_types::Owner::Address(owner),
    );
    state
        .get_cache_writer()
        .write_object_entry_for_test(new_owned_object);

    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), transaction.digest());
    assert_eq!(ready.expected_effects_digest, Some(expected));
}

/// A transaction with several missing inputs must wait for ALL of them, not
/// just the first to arrive. This pins the `missing_input_keys` computation and
/// the wait-for-all semantics of `notify_read_input_objects`: releasing after
/// only one input became available would dispatch a transaction before the rest
/// of its inputs are durably present — a data-availability hazard. The existing
/// ES wait tests each park on a single input, so multi-input all-of is
/// uncovered.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_awaits_all_missing_inputs() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let owned_a = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let owned_b = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state =
        init_state_with_objects(vec![gas_object.clone(), owned_a.clone(), owned_b.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // Reference both owned inputs at a version not present in the cache.
    let awaited_version = 2000.into();
    let mut ref_a = owned_a.object_ref();
    ref_a.version = awaited_version;
    let mut ref_b = owned_b.object_ref();
    ref_b.version = awaited_version;
    let transaction = make_transaction(
        gas_object,
        vec![
            CallArg::ImmutableOrOwned(ref_a),
            CallArg::ImmutableOrOwned(ref_b),
        ],
    );
    execution_scheduler.enqueue(vec![transaction.clone()], &state.epoch_store_for_testing());

    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_certificates(), 1);

    // Writing only the FIRST input must NOT release the transaction.
    let a_ready = Object::with_id_owner_version_for_testing(
        owned_a.id(),
        awaited_version,
        iota_sdk_types::Owner::Address(owner),
    );
    state
        .get_cache_writer()
        .write_object_entry_for_test(a_ready);
    sleep(Duration::from_secs(1)).await;
    assert!(
        rx_ready_transactions.try_recv().is_err(),
        "transaction released before its second input became available"
    );
    assert_eq!(execution_scheduler.num_pending_certificates(), 1);

    // Writing the SECOND input releases it exactly once.
    let b_ready = Object::with_id_owner_version_for_testing(
        owned_b.id(),
        awaited_version,
        iota_sdk_types::Owner::Address(owner),
    );
    state
        .get_cache_writer()
        .write_object_entry_for_test(b_ready);

    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), transaction.digest());
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
}

/// A certificate from a different epoch must be dropped at enqueue and never
/// sent for execution — even when all of its inputs are available, so that
/// absent the epoch filter it would be dispatched immediately. Executing a
/// stale-epoch certificate would be a consensus-safety violation the scheduler
/// swap must not introduce.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn enqueue_wrong_epoch_transaction_is_dropped() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // The epoch store is at epoch 0; tag the transaction for epoch 1.
    let epoch_store = state.epoch_store_for_testing();
    assert_eq!(epoch_store.epoch(), 0);
    let transaction = make_transaction_for_epoch(gas_object, vec![], 1);

    execution_scheduler.enqueue(vec![transaction], &epoch_store);

    // A same-epoch transaction with these inputs would be ready immediately; this
    // one must be filtered out and leave no pending or ready work behind.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_certificates(), 0);
}

/// The `ExecutionScheduler` keeps no per-epoch state to reset explicitly: it
/// relies on `within_alive_epoch` cancelling its per-transaction tasks at an
/// epoch boundary, which drops each `PendingGuard` and so clears both the
/// pending gauge (feeding overload admission) and the per-object overload
/// tracker. A regression that leaked either would carry false congestion into
/// the next epoch and permanently reject transactions.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_reconfigure_clears_pending_and_overload() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_objects: Vec<Object> = (0..2)
        .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), owner))
        .collect();
    let shared_object = Object::shared_for_testing();
    let initial_shared_version = shared_object.version();

    let mut objects = gas_objects.clone();
    objects.push(shared_object.clone());
    let state = init_state_with_objects(objects).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // Two transactions waiting on the same MUTABLE shared object at a
    // consensus-assigned version that is not yet available — the overload tracker
    // only counts mutable shared inputs.
    let shared_version = 1000.into();
    let shared_arg = CallArg::Shared(SharedObjectRef::new(
        shared_object.id(),
        initial_shared_version,
        true,
    ));
    let mut txns = Vec::new();
    for gas in &gas_objects {
        let txn = make_transaction(gas.clone(), vec![shared_arg.clone()]);
        state
            .epoch_store_for_testing()
            .set_shared_object_versions_for_testing(
                txn.digest(),
                &[VersionAssignment::new(shared_object.id(), shared_version)],
            )
            .unwrap();
        execution_scheduler.enqueue(vec![txn.clone()], &state.epoch_store_for_testing());
        txns.push(txn);
    }

    // Both sit pending, and the overload tracker reports the object as congested.
    sleep(Duration::from_secs(1)).await;
    assert_eq!(execution_scheduler.num_pending_certificates(), txns.len());
    let overload_config = AuthorityOverloadConfig {
        max_transaction_manager_per_object_queue_length: txns.len(),
        ..Default::default()
    };
    assert!(
        execution_scheduler
            .check_execution_overload(&overload_config, txns[0].data())
            .is_err(),
        "the hot shared object must read as overloaded while its transactions are pending"
    );

    // Terminating the epoch cancels the per-transaction tasks; their PendingGuards
    // drop, clearing both the pending gauge and the overload tracker.
    state.epoch_store_for_testing().epoch_terminated().await;

    assert_eq!(execution_scheduler.num_pending_certificates(), 0);
    assert!(rx_ready_transactions.try_recv().is_err());
    assert!(
        execution_scheduler
            .check_execution_overload(&overload_config, txns[0].data())
            .is_ok(),
        "after reconfiguration the overload tracker must be clear"
    );
}
