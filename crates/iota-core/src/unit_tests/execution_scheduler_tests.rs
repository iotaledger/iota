// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{time::Duration, vec};

use iota_sdk_types::ObjectId;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::deterministic_random_account_key,
    executable_transaction::VerifiedExecutableTransaction,
    object::Object,
    transaction::{CallArg, VerifiedTransaction},
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, unbounded_channel},
    time::sleep,
};

use crate::{
    authority::{AuthorityState, authority_tests::init_state_with_objects},
    execution_scheduler::{
        ExecutionSchedulerAPI, PendingCertificate, execution_scheduler_impl::ExecutionScheduler,
    },
};

#[expect(clippy::disallowed_methods)] // allow unbounded_channel()
fn make_execution_scheduler(
    state: &AuthorityState,
) -> (ExecutionScheduler, UnboundedReceiver<PendingCertificate>) {
    // A standalone scheduler (not the authority's) so the test can observe its
    // output on rx_ready_certificates.
    let (tx_ready_certificates, rx_ready_certificates) = unbounded_channel();
    let execution_scheduler = ExecutionScheduler::new(
        state.get_object_cache_reader().clone(),
        state.get_transaction_cache_reader().clone(),
        tx_ready_certificates,
        state.metrics.clone(),
    );
    (execution_scheduler, rx_ready_certificates)
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
    let (execution_scheduler, mut rx_ready_certificates) = make_execution_scheduler(&state);
    assert!(rx_ready_certificates.try_recv().is_err());

    // Reference the owned object at a version not present in the cache, so the
    // transaction cannot be ready until that version is written.
    let awaited_version = 2000.into();
    let mut owned_ref = owned_object.object_ref();
    owned_ref.version = awaited_version;
    let transaction = make_transaction(gas_object, vec![CallArg::ImmutableOrOwned(owned_ref)]);

    execution_scheduler.enqueue(vec![transaction.clone()], &state.epoch_store_for_testing());

    // The scheduler must NOT release the transaction while its input is missing.
    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_certificates.try_recv().is_err());
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
    let ready = rx_ready_certificates.recv().await.unwrap().certificate;
    assert_eq!(ready.digest(), transaction.digest());

    sleep(Duration::from_secs(1)).await;
    assert!(rx_ready_certificates.try_recv().is_err());
}
