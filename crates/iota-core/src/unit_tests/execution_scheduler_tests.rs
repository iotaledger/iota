// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{time::Duration, vec};

use iota_sdk_types::{ObjectId, VersionAssignment};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::deterministic_random_account_key,
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
    // Fake module/function/gas price: irrelevant to scheduling.
    let rgp = 100;
    let (sender, keypair) = deterministic_random_account_key();
    let transaction = TestTransactionBuilder::new(sender, gas_object.object_ref(), rgp)
        .move_call(ObjectId::FRAMEWORK, "counter", "assert_value", input)
        .build_and_sign(&keypair);
    VerifiedExecutableTransaction::new_system(VerifiedTransaction::new_unchecked(transaction), 0)
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
