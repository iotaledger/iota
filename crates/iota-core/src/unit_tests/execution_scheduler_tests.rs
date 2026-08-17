// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the `ExecutionScheduler`, run against a standalone scheduler
//! instance wired to a real authority's caches so its output can be observed
//! directly on `rx_ready_transactions` (the channel the execution driver
//! consumes in production).
//!
//! Each test pins one scheduling invariant that swapping the
//! `TransactionManager` for the `ExecutionScheduler` must preserve: dispatch
//! only after ALL inputs are available, verbatim effects-digest propagation
//! (fork detection), wrong-epoch filtering, and the RAII gauge/overload
//! accounting that feeds admission control — including its cleanup through
//! `within_alive_epoch` cancellation at an epoch boundary, which is the
//! scheduler's only reconfiguration mechanism.

use std::{time::Duration, vec};

use iota_config::node::AuthorityOverloadConfig;
use iota_sdk_types::{
    MoveAuthenticatorV1, ObjectId, RandomnessRound, SenderSignedTransaction, SharedObjectReference,
    TransactionEffectsDigest, UserSignature, VersionAssignment,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::deterministic_random_account_private_key,
    error::IotaError,
    executable_transaction::VerifiedExecutableTransaction,
    object::Object,
    storage::InputKey,
    transaction::{CallArg, TransactionEnvelope, TransactionKey, VerifiedTransaction},
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    time::sleep,
};

use crate::{
    authority::{
        AuthorityState, ExecutionEnv, authority_per_epoch_store::AuthorityPerEpochStore,
        authority_tests::init_state_with_objects, epoch_start_configuration::EpochStartConfigTrait,
        shared_object_version_manager::Schedulable,
    },
    execution_scheduler::{
        ExecutionSchedulerAPI, PendingTransaction, execution_scheduler_impl::ExecutionScheduler,
        transaction_manager::TransactionManager,
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

#[expect(clippy::disallowed_methods)] // allow unbounded_channel()
fn make_transaction_manager(
    state: &AuthorityState,
) -> (TransactionManager, UnboundedReceiver<PendingTransaction>) {
    // A standalone manager, so the test can observe its output on
    // rx_ready_transactions.
    let (tx_ready_transactions, rx_ready_transactions) = unbounded_channel();
    let transaction_manager = TransactionManager::new(
        state.get_object_cache_reader().clone(),
        state.get_transaction_cache_reader().clone(),
        &state.epoch_store_for_testing(),
        tx_ready_transactions,
        state.metrics.clone(),
    );
    (transaction_manager, rx_ready_transactions)
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
    let (sender, sender_key) = deterministic_random_account_private_key();
    let transaction = TestTransactionBuilder::new(sender, gas_object.object_ref(), rgp)
        .move_call(ObjectId::FRAMEWORK, "counter", "assert_value", input)
        .build_and_sign(&sender_key);
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
    let (owner, _key) = deterministic_random_account_private_key();
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

    execution_scheduler.enqueue_transactions(
        vec![(transaction.clone(), ExecutionEnv::new())],
        &state.epoch_store_for_testing(),
    );

    // The scheduler must NOT release the transaction while its input is missing.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_transactions(), 1);

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
    assert_eq!(execution_scheduler.num_pending_transactions(), 1);
    drop(ready);
    assert_eq!(execution_scheduler.num_pending_transactions(), 0);
}

/// A transaction whose own input is only the (available) gas object, but whose
/// `MoveAuthenticator` authenticates the given shared object.
fn make_shared_authenticator_transaction(
    gas_object: &Object,
    shared_object: &Object,
) -> VerifiedExecutableTransaction {
    let (sender, _sender_key) = deterministic_random_account_private_key();
    let tx_data = TestTransactionBuilder::new(sender, gas_object.object_ref(), 100)
        .move_call(ObjectId::FRAMEWORK, "counter", "assert_value", vec![])
        .build();
    let authenticator = MoveAuthenticatorV1::new_with_shared_account_object(
        vec![],
        vec![],
        SharedObjectReference::new(shared_object.id(), 0.into(), false),
    );
    let signed = TransactionEnvelope::new(SenderSignedTransaction::new(
        tx_data,
        vec![UserSignature::MoveAuthenticator(authenticator.into())],
    ));
    VerifiedExecutableTransaction::new_system(VerifiedTransaction::new_unchecked(signed), 0)
}

/// Enqueues `transaction`, asserts it is held while its authenticator's shared
/// input is missing, then releases the input via `make_shared_input_available`
/// and asserts the transaction is dispatched.
async fn assert_awaits_authenticator_input(
    who: &str,
    state: &AuthorityState,
    scheduler: &dyn ExecutionSchedulerAPI,
    rx_ready_transactions: &mut UnboundedReceiver<PendingTransaction>,
    transaction: &VerifiedExecutableTransaction,
    env: ExecutionEnv,
    make_shared_input_available: impl FnOnce(),
) {
    scheduler.enqueue_transactions(
        vec![(transaction.clone(), env)],
        &state.epoch_store_for_testing(),
    );

    sleep(Duration::from_secs(1)).await;
    assert!(
        rx_ready_transactions.try_recv().is_err(),
        "{who} dispatched before its authenticator's shared input became available"
    );
    assert_eq!(scheduler.num_pending_transactions(), 1);

    make_shared_input_available();
    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), transaction.digest());
}

/// A transaction's readiness set must include the objects read by its
/// `MoveAuthenticator`s, not only the transaction's own inputs: the gas input
/// is available but the authenticator authenticates a shared object that is
/// not, so the scheduler must hold the transaction until that object becomes
/// available.
///
/// - `ExecutionScheduler` fails this without scheduling on
///   `collect_all_input_object_kind_for_reading`: it dispatches early and would
///   later panic in the input loader.
/// - `TransactionManager` already passes: it schedules on the envelope's
///   `SenderSignedTransaction::input_objects()`, which already includes
///   authenticator inputs.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn schedulers_wait_for_authenticator_inputs() {
    let shared_version = 2000.into();

    // ExecutionScheduler: released by writing the shared object into the cache,
    // which the scheduler observes through `notify_read_input_objects`.
    {
        let (owner, _key) = deterministic_random_account_private_key();
        let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
        let shared_object = Object::shared_for_testing();
        let state = init_state_with_objects(vec![gas_object.clone(), shared_object.clone()]).await;
        let (scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

        let transaction = make_shared_authenticator_transaction(&gas_object, &shared_object);
        let env = ExecutionEnv::new().with_assigned_versions(vec![VersionAssignment::new(
            shared_object.id(),
            shared_version,
        )]);

        assert_awaits_authenticator_input(
            "ExecutionScheduler",
            &state,
            &scheduler,
            &mut rx_ready_transactions,
            &transaction,
            env,
            || {
                state.get_cache_writer().write_object_entry_for_test(
                    Object::with_id_owner_version_for_testing(
                        shared_object.id(),
                        shared_version,
                        iota_sdk_types::Owner::Shared(0.into()),
                    ),
                );
            },
        )
        .await;
    }

    // TransactionManager: released by an explicit `objects_available` push.
    {
        let (owner, _key) = deterministic_random_account_private_key();
        let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
        let shared_object = Object::shared_for_testing();
        let state = init_state_with_objects(vec![gas_object.clone(), shared_object.clone()]).await;
        let (scheduler, mut rx_ready_transactions) = make_transaction_manager(&state);

        let transaction = make_shared_authenticator_transaction(&gas_object, &shared_object);
        let env = ExecutionEnv::new().with_assigned_versions(vec![VersionAssignment::new(
            shared_object.id(),
            shared_version,
        )]);

        assert_awaits_authenticator_input(
            "TransactionManager",
            &state,
            &scheduler,
            &mut rx_ready_transactions,
            &transaction,
            env,
            || {
                scheduler.objects_available(
                    vec![InputKey::VersionedObject {
                        id: shared_object.id(),
                        version: shared_version,
                    }],
                    &state.epoch_store_for_testing(),
                );
            },
        )
        .await;
    }
}

/// One object becoming available must release ALL transactions waiting on it.
/// This exercises the NotifyRead fan-out across the scheduler's independent
/// per-transaction tasks (each registers its own waiter on the shared input),
/// and confirms the pending/executing gauges return to 0 afterwards.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_releases_all_waiters_on_one_object() {
    let (owner, _key) = deterministic_random_account_private_key();
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
    let shared_arg = CallArg::Shared(SharedObjectReference::new(
        shared_object.id(),
        initial_shared_version,
        false,
    ));
    let mut txns = Vec::new();
    for gas in &gas_objects {
        let txn = make_transaction(gas.clone(), vec![shared_arg.clone()]);
        let env = ExecutionEnv::new().with_assigned_versions(vec![VersionAssignment::new(
            shared_object.id(),
            shared_version,
        )]);
        execution_scheduler
            .enqueue_transactions(vec![(txn.clone(), env)], &state.epoch_store_for_testing());
        txns.push(txn);
    }

    // None are ready while the shared object version is missing.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_transactions(), txns.len());

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
    assert_eq!(execution_scheduler.num_pending_transactions(), txns.len());
    drop(ready);
    assert_eq!(execution_scheduler.num_pending_transactions(), 0);
}

/// When executing from a checkpoint the scheduler is handed a certified
/// `expected_effects_digest` that the execution driver uses to detect forks.
/// The scheduler must copy it verbatim into the `PendingTransaction` it emits
/// on the fast path (inputs already available). If it were dropped or defaulted
/// to `None`, fork detection would be silently disabled for every state-synced
/// checkpoint, with no metric or log to notice.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn scheduler_propagates_expected_effects_digest_fast_path() {
    let (owner, _key) = deterministic_random_account_private_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // Inputs (gas + framework package) are already available, so the transaction
    // takes the fast path in `schedule_transaction`.
    let transaction = make_transaction(gas_object, vec![]);
    let expected = TransactionEffectsDigest::new([7; 32]);
    execution_scheduler.enqueue_transactions(
        vec![(
            transaction.clone(),
            ExecutionEnv::new().with_expected_effects_digest(expected),
        )],
        &state.epoch_store_for_testing(),
    );

    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), transaction.digest());
    assert_eq!(ready.execution_env.expected_effects_digest, Some(expected));
}

/// The certified `expected_effects_digest` must also survive the `notify_read`
/// wait path — the scheduler sets it at a second call site, reached only when
/// an input is initially missing. A regression dropping it there would disable
/// fork detection for exactly the synced transactions whose inputs arrive late.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn scheduler_propagates_expected_effects_digest_wait_path() {
    let (owner, _key) = deterministic_random_account_private_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let owned_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone(), owned_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    let awaited_version = 2000.into();
    let mut owned_ref = owned_object.object_ref();
    owned_ref.version = awaited_version;
    let transaction = make_transaction(gas_object, vec![CallArg::ImmutableOrOwned(owned_ref)]);

    let expected = TransactionEffectsDigest::new([9; 32]);
    execution_scheduler.enqueue_transactions(
        vec![(
            transaction.clone(),
            ExecutionEnv::new().with_expected_effects_digest(expected),
        )],
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
    assert_eq!(ready.execution_env.expected_effects_digest, Some(expected));
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
    let (owner, _key) = deterministic_random_account_private_key();
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
    execution_scheduler.enqueue_transactions(
        vec![(transaction.clone(), ExecutionEnv::new())],
        &state.epoch_store_for_testing(),
    );

    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_transactions(), 1);

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
    assert_eq!(execution_scheduler.num_pending_transactions(), 1);

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
    let (owner, _key) = deterministic_random_account_private_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // The epoch store is at epoch 0; tag the transaction for epoch 1.
    let epoch_store = state.epoch_store_for_testing();
    assert_eq!(epoch_store.epoch(), 0);
    let transaction = make_transaction_for_epoch(gas_object, vec![], 1);

    execution_scheduler
        .enqueue_transactions(vec![(transaction, ExecutionEnv::new())], &epoch_store);

    // A same-epoch transaction with these inputs would be ready immediately; this
    // one must be filtered out and leave no pending or ready work behind.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
    assert_eq!(execution_scheduler.num_pending_transactions(), 0);
}

/// The `ExecutionScheduler` keeps no per-epoch state to reset explicitly: it
/// relies on `within_alive_epoch` cancelling its per-transaction tasks at an
/// epoch boundary, which drops each `PendingGuard` and so clears both the
/// pending gauge (feeding overload admission) and the per-object overload
/// tracker. A regression that leaked either would carry false congestion into
/// the next epoch and permanently reject transactions.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_reconfigure_clears_pending_and_overload() {
    let (owner, _key) = deterministic_random_account_private_key();
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
    let shared_arg = CallArg::Shared(SharedObjectReference::new(
        shared_object.id(),
        initial_shared_version,
        true,
    ));
    let mut txns = Vec::new();
    for gas in &gas_objects {
        let txn = make_transaction(gas.clone(), vec![shared_arg.clone()]);
        let env = ExecutionEnv::new().with_assigned_versions(vec![VersionAssignment::new(
            shared_object.id(),
            shared_version,
        )]);
        execution_scheduler
            .enqueue_transactions(vec![(txn.clone(), env)], &state.epoch_store_for_testing());
        txns.push(txn);
    }

    // Both sit pending, and the overload tracker reports the object as congested.
    sleep(Duration::from_secs(1)).await;
    assert_eq!(execution_scheduler.num_pending_transactions(), txns.len());
    let overload_config = AuthorityOverloadConfig {
        max_transaction_manager_per_object_queue_length: txns.len(),
        ..Default::default()
    };
    // Pin the exact error: the per-object queue-length check fires before the
    // age check in `OverloadTracker::check_execution_overload`, so a plain
    // `is_err()` could pass for the wrong reason (global limit, transaction age).
    let err = execution_scheduler
        .check_execution_overload(&overload_config, txns[0].data())
        .unwrap_err();
    assert!(
        matches!(err, IotaError::TooManyTransactionsPendingOnObject { .. }),
        "the hot shared object must read as overloaded while its transactions are pending; \
         got {err:?}"
    );

    // Terminating the epoch cancels the per-transaction tasks; their PendingGuards
    // drop, clearing both the pending gauge and the overload tracker.
    state.epoch_store_for_testing().epoch_terminated().await;

    assert_eq!(execution_scheduler.num_pending_transactions(), 0);
    assert!(rx_ready_transactions.try_recv().is_err());
    assert!(
        execution_scheduler
            .check_execution_overload(&overload_config, txns[0].data())
            .is_ok(),
        "after reconfiguration the overload tracker must be clear"
    );
}

/// Both schedulers must account for a transaction becoming ready the same way.
/// The overload monitor derives latency-based load shedding from
/// `txn_ready_rate_tracker`, and the execution driver decrements
/// `execution_driver_dispatch_queue` for every transaction it receives, no
/// matter which scheduler produced it. A scheduler that skips this accounting
/// leaves the ready rate at zero — which turns latency-based shedding off, as
/// `calculate_load_shedding_percentage` returns 0% for a zero rate — and drives
/// the dispatch gauge negative.
#[tokio::test]
async fn schedulers_record_ready_transaction_accounting() {
    async fn assert_records_ready_accounting(
        who: &str,
        state: &AuthorityState,
        scheduler: &dyn ExecutionSchedulerAPI,
        rx_ready_transactions: &mut UnboundedReceiver<PendingTransaction>,
        transaction: &VerifiedExecutableTransaction,
    ) {
        let num_ready_before = state.metrics.transaction_manager_num_ready.get();
        let dispatch_queue_before = state.metrics.execution_driver_dispatch_queue.get();

        scheduler.enqueue_transactions(
            vec![(transaction.clone(), ExecutionEnv::new())],
            &state.epoch_store_for_testing(),
        );
        let ready = rx_ready_transactions.recv().await.unwrap();
        assert_eq!(ready.transaction.digest(), transaction.digest());

        assert_eq!(
            state.metrics.transaction_manager_num_ready.get() - num_ready_before,
            1,
            "{who} did not count the transaction as ready"
        );
        assert_eq!(
            state.metrics.execution_driver_dispatch_queue.get() - dispatch_queue_before,
            1,
            "{who} did not increment the dispatch queue that the execution driver decrements"
        );
        assert!(
            state.metrics.txn_ready_rate_tracker.lock().rate() > 0.0,
            "{who} did not record the ready rate that latency-based load shedding reads"
        );
    }

    let (owner, _key) = deterministic_random_account_private_key();

    // Inputs (gas + framework package) are already available, so the transaction
    // is dispatched right away by either scheduler.
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone()]).await;
    let transaction = make_transaction(gas_object, vec![]);
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);
    assert_records_ready_accounting(
        "ExecutionScheduler",
        &state,
        &execution_scheduler,
        &mut rx_ready_transactions,
        &transaction,
    )
    .await;

    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone()]).await;
    let transaction = make_transaction(gas_object, vec![]);
    let (transaction_manager, mut rx_ready_transactions) = make_transaction_manager(&state);
    assert_records_ready_accounting(
        "TransactionManager",
        &state,
        &transaction_manager,
        &mut rx_ready_transactions,
        &transaction,
    )
    .await;
}

/// A randomness state update transaction for `round`, as
/// `RandomnessRoundReceiver` would build it.
fn make_randomness_state_update(
    epoch_store: &AuthorityPerEpochStore,
    round: u64,
) -> VerifiedExecutableTransaction {
    VerifiedExecutableTransaction::new_system(
        VerifiedTransaction::new_randomness_state_update(
            epoch_store.epoch(),
            RandomnessRound::new(round),
            vec![round as u8],
            epoch_store
                .epoch_start_config()
                .randomness_obj_initial_shared_version(),
        ),
        epoch_store.epoch(),
    )
}

/// Builds the randomness state update for `round`, persists it and records its
/// key, which is what `RandomnessRoundReceiver` does before it notifies.
fn resolve_randomness_round(
    state: &AuthorityState,
    epoch_store: &AuthorityPerEpochStore,
    round: u64,
) -> VerifiedExecutableTransaction {
    let transaction = make_randomness_state_update(epoch_store, round);
    state.get_cache_commit().persist_transaction(&transaction);
    epoch_store
        .insert_tx_key(
            TransactionKey::RandomnessRound(epoch_store.epoch(), RandomnessRound::new(round)),
            *transaction.digest(),
        )
        .unwrap();
    transaction
}

fn randomness_assigned_versions(epoch_store: &AuthorityPerEpochStore) -> Vec<VersionAssignment> {
    vec![VersionAssignment::new(
        ObjectId::RANDOMNESS_STATE,
        epoch_store
            .epoch_start_config()
            .randomness_obj_initial_shared_version(),
    )]
}

/// A schedulable that has no transaction yet, only its `TransactionKey`, waits
/// for that key's digest. Two rounds enqueued together must each be released
/// with the env parked for it: the digests come back from
/// `notify_read_tx_key_to_digest` and are zipped positionally with the envs, so
/// a swapped pairing would execute a round on another round's versions.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_resolves_schedulables_by_key_with_matching_envs() {
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    let expected_1 = TransactionEffectsDigest::new([1; 32]);
    let expected_2 = TransactionEffectsDigest::new([2; 32]);
    execution_scheduler.enqueue(
        vec![
            (
                Schedulable::RandomnessStateUpdate(epoch_store.epoch(), RandomnessRound::new(1)),
                ExecutionEnv::new()
                    .with_assigned_versions(randomness_assigned_versions(&epoch_store))
                    .with_expected_effects_digest(expected_1),
            ),
            (
                Schedulable::RandomnessStateUpdate(epoch_store.epoch(), RandomnessRound::new(2)),
                ExecutionEnv::new()
                    .with_assigned_versions(randomness_assigned_versions(&epoch_store))
                    .with_expected_effects_digest(expected_2),
            ),
        ],
        &epoch_store,
    );

    // Neither transaction exists yet: nothing must be dispatched.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());

    let transaction_1 = resolve_randomness_round(&state, &epoch_store, 1);
    let transaction_2 = resolve_randomness_round(&state, &epoch_store, 2);

    let first = rx_ready_transactions.recv().await.unwrap();
    let second = rx_ready_transactions.recv().await.unwrap();
    // Without this, one round arriving twice while the other is lost would pass
    // the loop below.
    assert_ne!(first.transaction.digest(), second.transaction.digest());
    for pending in [first, second] {
        let expected = if pending.transaction.digest() == transaction_1.digest() {
            expected_1
        } else {
            assert_eq!(pending.transaction.digest(), transaction_2.digest());
            expected_2
        };
        assert_eq!(
            pending.execution_env.expected_effects_digest,
            Some(expected),
            "schedulable released with another round's env"
        );
    }
}

/// A key that already resolves to a digest at enqueue time (crash recovery:
/// the persistent `transaction_key_to_digest` table survived a restart) must
/// be scheduled immediately with its env.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_schedules_already_resolved_randomness_key() {
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    let round = RandomnessRound::new(3);
    let transaction = resolve_randomness_round(&state, &epoch_store, 3);
    let digest = *transaction.digest();

    let assigned_versions = randomness_assigned_versions(&epoch_store);
    execution_scheduler.enqueue(
        vec![(
            Schedulable::RandomnessStateUpdate(epoch_store.epoch(), round),
            ExecutionEnv::new().with_assigned_versions(assigned_versions.clone()),
        )],
        &epoch_store,
    );

    let pending = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(pending.transaction.digest(), &digest);
    assert_eq!(pending.execution_env.assigned_versions, assigned_versions);
}

/// A mixed batch must not couple plain transactions to the ones that only have
/// a key: the plain transaction (inputs available) is dispatched immediately,
/// while the other stays parked until its key resolves. This pins the
/// partition in `enqueue` — a real consensus commit mixes user transactions
/// and the round's `RandomnessStateUpdate` in one call, and funneling the
/// whole batch through the key-resolution task would stall every user
/// transaction of the commit behind the randomness round.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_mixed_batch_dispatches_plain_transaction_immediately() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let state = init_state_with_objects(vec![gas_object.clone()]).await;
    let epoch_store = state.epoch_store_for_testing();
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    let round = RandomnessRound::new(5);
    let transaction = make_transaction(gas_object, vec![]);
    execution_scheduler.enqueue(
        vec![
            (
                Schedulable::Transaction(transaction.clone()),
                ExecutionEnv::new(),
            ),
            (
                Schedulable::RandomnessStateUpdate(epoch_store.epoch(), round),
                ExecutionEnv::new()
                    .with_assigned_versions(randomness_assigned_versions(&epoch_store)),
            ),
        ],
        &epoch_store,
    );

    // The plain transaction is dispatched immediately...
    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), transaction.digest());

    // ...while the schedulable stays parked on its unresolved key.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());

    // Resolving the key releases it.
    let randomness_tx = resolve_randomness_round(&state, &epoch_store, 5);
    let ready = rx_ready_transactions.recv().await.unwrap();
    assert_eq!(ready.transaction.digest(), randomness_tx.digest());
}

/// The two schedulers fail differently on a missing shared version assignment,
/// and there is a test for each of them. Such a transaction panics in
/// `get_input_object_keys`, the check that stops execution on unassigned
/// versions. Here that panic happens inside the detached per-transaction task,
/// where the runtime swallows it and the transaction is dropped; the
/// `TransactionManager` panics in the caller instead
/// (`transaction_manager_missing_shared_version_assignment_panics`). This test
/// cannot tell a dropped transaction from one parked forever — it observes only
/// that nothing was dispatched. If the scheduler ever learns to report this
/// error, this test must change with it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_missing_shared_version_assignment_drops_transaction() {
    let (owner, _keypair) = deterministic_random_account_key();
    let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), owner);
    let shared_object = Object::shared_for_testing();
    let state = init_state_with_objects(vec![gas_object.clone(), shared_object.clone()]).await;
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    // The shared object is available at the version the call arg declares; only
    // the assignment is missing. So if the panic ever became a fallback to the
    // declared version, the transaction would dispatch and the assertion below
    // would fail.
    let shared_arg = CallArg::Shared(SharedObjectReference::new(
        shared_object.id(),
        shared_object.version(),
        true,
    ));
    let transaction = make_transaction(gas_object, vec![shared_arg]);
    execution_scheduler.enqueue_transactions(
        vec![(transaction, ExecutionEnv::new())],
        &state.epoch_store_for_testing(),
    );

    // The per-transaction task panics; a backtrace in the test output is
    // expected. Nothing is dispatched.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
}

/// The task waiting on a key runs under `within_alive_epoch`:
/// terminating the epoch must cancel it, so a key resolved after the epoch
/// ended dispatches nothing.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn execution_scheduler_drops_unresolved_key_wait_on_epoch_termination() {
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();
    let (execution_scheduler, mut rx_ready_transactions) = make_execution_scheduler(&state);

    let round = RandomnessRound::new(4);
    execution_scheduler.enqueue(
        vec![(
            Schedulable::RandomnessStateUpdate(epoch_store.epoch(), round),
            ExecutionEnv::new().with_assigned_versions(randomness_assigned_versions(&epoch_store)),
        )],
        &epoch_store,
    );
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());

    epoch_store.epoch_terminated().await;

    // Resolving the key after the epoch ended must not dispatch anything.
    resolve_randomness_round(&state, &epoch_store, 4);
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_transactions.try_recv().is_err());
}
