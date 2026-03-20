// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, time::Duration};

use consensus_core::{BlockRef, BlockStatus};
use fastcrypto::traits::KeyPair;
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    base_types::{ExecutionDigests, ObjectID},
    crypto::deterministic_random_account_key,
    gas::GasCostSummary,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSignatureMessage,
        CheckpointSummary, SignedCheckpointSummary,
    },
    object::Object,
    transaction::{
        CallArg, CertifiedTransaction, ObjectArg, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
        TransactionData,
    },
    utils::{make_committee_key_num, to_sender_signed_transaction},
};
use move_core_types::{account_address::AccountAddress, ident_str};
use parking_lot::Mutex;
use rand::{Rng, SeedableRng, rngs::StdRng, thread_rng};
use tokio::time::sleep;

use super::*;
use crate::{
    authority::{AuthorityState, authority_tests::init_state_with_objects},
    checkpoints::CheckpointServiceNoop,
    consensus_handler::SequencedConsensusTransaction,
    mock_consensus::with_block_status,
};

/// Fixture: a few test gas objects.
pub fn test_gas_objects() -> Vec<Object> {
    thread_local! {
        static GAS_OBJECTS: Vec<Object> = (0..4)
            .map(|_| {
                let gas_object_id = ObjectID::random();
                let (owner, _) = deterministic_random_account_key();
                Object::with_id_owner_for_testing(gas_object_id, owner)
            })
            .collect();
    }

    GAS_OBJECTS.with(|v| v.clone())
}

/// Fixture: a few test certificates containing a shared object.
pub async fn test_certificates(
    authority: &AuthorityState,
    shared_object: Object,
) -> Vec<CertifiedTransaction> {
    let epoch_store = authority.load_epoch_store_one_call_per_task();
    let (sender, keypair) = deterministic_random_account_key();
    let rgp = epoch_store.reference_gas_price();

    let mut certificates = Vec::new();
    let shared_object_arg = ObjectArg::SharedObject {
        id: shared_object.id(),
        initial_shared_version: shared_object.version(),
        mutable: true,
    };
    for gas_object in test_gas_objects() {
        // Object digest may be different in genesis than originally generated.
        let gas_object = authority.get_object(&gas_object.id()).await.unwrap();
        // Make a sample transaction.
        let module = "object_basics";
        let function = "create";

        let data = TransactionData::new_move_call(
            sender,
            IOTA_FRAMEWORK_PACKAGE_ID,
            ident_str!(module).to_owned(),
            ident_str!(function).to_owned(),
            // type_args
            vec![],
            gas_object.compute_object_reference(),
            // args
            vec![
                CallArg::Object(shared_object_arg),
                CallArg::Pure(16u64.to_le_bytes().to_vec()),
                CallArg::Pure(bcs::to_bytes(&AccountAddress::from(sender)).unwrap()),
            ],
            rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
            rgp,
        )
        .unwrap();

        let transaction = epoch_store
            .verify_transaction(to_sender_signed_transaction(data, &keypair))
            .unwrap();

        // Submit the transaction and assemble a certificate.
        let response = authority
            .handle_transaction(&epoch_store, transaction.clone())
            .await
            .unwrap();
        let vote = response.status.into_signed_for_testing();
        let certificate = CertifiedTransaction::new(
            transaction.into_message(),
            vec![vote.clone()],
            &authority.clone_committee_for_testing(),
        )
        .unwrap();
        certificates.push(certificate);
    }
    certificates
}

pub fn make_consensus_adapter_for_test(
    state: Arc<AuthorityState>,
    process_via_checkpoint: HashSet<TransactionDigest>,
    execute: bool,
    mock_block_status_receivers: Vec<BlockStatusReceiver>,
) -> Arc<ConsensusAdapter> {
    let metrics = ConsensusAdapterMetrics::new_test();

    #[derive(Clone)]
    struct SubmitDirectly {
        state: Arc<AuthorityState>,
        process_via_checkpoint: HashSet<TransactionDigest>,
        execute: bool,
        mock_block_status_receivers: Arc<Mutex<Vec<BlockStatusReceiver>>>,
    }

    #[async_trait::async_trait]
    impl ConsensusClient for SubmitDirectly {
        async fn submit(
            &self,
            transactions: &[ConsensusTransaction],
            epoch_store: &Arc<AuthorityPerEpochStore>,
        ) -> IotaResult<BlockStatusReceiver> {
            let sequenced_transactions: Vec<SequencedConsensusTransaction> = transactions
                .iter()
                .map(|txn| SequencedConsensusTransaction::new_test(txn.clone()))
                .collect();

            let checkpoint_service = Arc::new(CheckpointServiceNoop {});
            let mut transactions = Vec::new();
            let mut executed_via_checkpoint = 0;

            for tx in sequenced_transactions {
                if let Some(transaction_digest) = tx.transaction.executable_transaction_digest() {
                    if self.process_via_checkpoint.contains(&transaction_digest) {
                        epoch_store
                            .insert_finalized_transactions(vec![transaction_digest].as_slice(), 10)
                            .expect("Should not fail");
                        executed_via_checkpoint += 1;
                    } else {
                        transactions.extend(
                            epoch_store
                                .process_consensus_transactions_for_tests(
                                    vec![tx],
                                    &checkpoint_service,
                                    self.state.get_object_cache_reader().as_ref(),
                                    self.state.get_transaction_cache_reader().as_ref(),
                                    &self.state.metrics,
                                    true,
                                    self.state.as_ref(),
                                )
                                .await?,
                        );
                    }
                } else {
                    transactions.extend(
                        epoch_store
                            .process_consensus_transactions_for_tests(
                                vec![tx],
                                &checkpoint_service,
                                self.state.get_object_cache_reader().as_ref(),
                                self.state.get_transaction_cache_reader().as_ref(),
                                &self.state.metrics,
                                true,
                                self.state.as_ref(),
                            )
                            .await?,
                    );
                }
            }

            assert_eq!(
                executed_via_checkpoint,
                self.process_via_checkpoint.len(),
                "Some transactions were not executed via checkpoint"
            );

            if self.execute {
                self.state
                    .transaction_manager()
                    .enqueue(transactions, epoch_store);
            }

            assert!(
                !self.mock_block_status_receivers.lock().is_empty(),
                "No mock submit responses left"
            );
            Ok(self.mock_block_status_receivers.lock().remove(0))
        }
    }
    // Make a new consensus adapter instance.
    Arc::new(ConsensusAdapter::new(
        Arc::new(SubmitDirectly {
            state: state.clone(),
            process_via_checkpoint,
            execute,
            mock_block_status_receivers: Arc::new(Mutex::new(mock_block_status_receivers)),
        }),
        state.checkpoint_store.clone(),
        state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        metrics,
    ))
}

#[tokio::test]
async fn submit_transaction_to_consensus_adapter() {
    telemetry_subscribers::init_for_testing();

    // Initialize an authority with a (owned) gas object and a shared object; then
    // make a test certificate.
    let mut objects = test_gas_objects();
    let shared_object = Object::shared_for_testing();
    objects.push(shared_object.clone());
    let state = init_state_with_objects(objects).await;
    let certificate = test_certificates(&state, shared_object)
        .await
        .pop()
        .unwrap();
    let epoch_store = state.epoch_store_for_testing();

    // Make a new consensus adapter instance.
    let block_status_receivers = vec![
        with_block_status(BlockStatus::GarbageCollected(BlockRef::MIN)),
        with_block_status(BlockStatus::GarbageCollected(BlockRef::MIN)),
        with_block_status(BlockStatus::GarbageCollected(BlockRef::MIN)),
        with_block_status(BlockStatus::Sequenced(BlockRef::MIN)),
    ];
    let adapter = make_consensus_adapter_for_test(
        state.clone(),
        HashSet::new(),
        false,
        block_status_receivers,
    );

    // Submit the transaction and ensure the adapter reports success to the caller.
    // Note that consensus may drop some transactions (so we may need to
    // resubmit them).
    let transaction = ConsensusTransaction::new_certificate_message(&state.name, certificate);
    let waiter = adapter
        .submit(
            transaction.clone(),
            Some(&epoch_store.get_reconfig_state_read_lock_guard()),
            &epoch_store,
        )
        .unwrap();
    waiter.await.unwrap();
}

#[tokio::test]
async fn submit_multiple_transactions_to_consensus_adapter() {
    telemetry_subscribers::init_for_testing();

    // Initialize an authority with a (owned) gas object and a shared object; then
    // make a test certificate.
    let mut objects = test_gas_objects();
    let shared_object = Object::shared_for_testing();
    objects.push(shared_object.clone());
    let state = init_state_with_objects(objects).await;
    let certificates = test_certificates(&state, shared_object).await;
    let epoch_store = state.epoch_store_for_testing();

    // Mark the first two transactions to be "executed via checkpoint" and the other
    // two to appear via consensus output.
    assert_eq!(certificates.len(), 4);

    let mut process_via_checkpoint = HashSet::new();
    process_via_checkpoint.insert(*certificates[0].digest());
    process_via_checkpoint.insert(*certificates[1].digest());

    // Make a new consensus adapter instance.
    let adapter = make_consensus_adapter_for_test(
        state.clone(),
        process_via_checkpoint,
        false,
        vec![with_block_status(BlockStatus::Sequenced(BlockRef::MIN))],
    );

    // Submit the transaction and ensure the adapter reports success to the caller.
    // Note that consensus may drop some transactions (so we may need to
    // resubmit them).
    let transactions = certificates
        .into_iter()
        .map(|certificate| ConsensusTransaction::new_certificate_message(&state.name, certificate))
        .collect::<Vec<_>>();

    let waiter = adapter
        .submit_batch(
            &transactions,
            Some(&epoch_store.get_reconfig_state_read_lock_guard()),
            &epoch_store,
        )
        .unwrap();
    waiter.await.unwrap();
}

#[sim_test]
async fn submit_checkpoint_signature_to_consensus_adapter() {
    telemetry_subscribers::init_for_testing();

    let mut rng = StdRng::seed_from_u64(1_100);
    let (keys, committee) = make_committee_key_num(1, &mut rng);

    // Initialize an authority
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();

    // Make a new consensus adapter instance.
    let adapter = make_consensus_adapter_for_test(
        state.clone(),
        HashSet::new(),
        false,
        vec![with_block_status(BlockStatus::Sequenced(BlockRef::MIN))],
    );

    let checkpoint_summary = CheckpointSummary::new(
        &ProtocolConfig::get_for_max_version_UNSAFE(),
        0,
        2,
        10,
        &CheckpointContents::new_with_digests_only_for_tests([ExecutionDigests::random()]),
        None,
        GasCostSummary::default(),
        None,
        100,
        Vec::new(),
    );

    let authority_key = &keys[0];
    let authority = authority_key.public().into();
    let signed_checkpoint_summary = SignedCheckpointSummary::new(
        committee.epoch,
        checkpoint_summary.clone(),
        authority_key,
        authority,
    );

    let checkpoint_cert = CertifiedCheckpointSummary::new(
        checkpoint_summary,
        vec![signed_checkpoint_summary.auth_sig().clone()],
        &committee,
    )
    .unwrap();

    let verified_checkpoint_summary = checkpoint_cert.try_into_verified(&committee).unwrap();

    let t1 = tokio::spawn({
        let state = state.clone();
        let verified_checkpoint_summary = verified_checkpoint_summary.clone();

        async move {
            let delay = Duration::from_millis(thread_rng().gen_range(0..1000));
            sleep(delay).await;
            state
                .checkpoint_store
                .insert_verified_checkpoint(&verified_checkpoint_summary)
                .unwrap();
            state
                .checkpoint_store
                .update_highest_synced_checkpoint(&verified_checkpoint_summary)
                .unwrap();
        }
    });

    let t2 = tokio::spawn(async move {
        let transactions = vec![ConsensusTransaction::new_checkpoint_signature_message(
            CheckpointSignatureMessage {
                summary: signed_checkpoint_summary,
            },
        )];

        let waiter = adapter
            .submit_batch(
                &transactions,
                Some(&epoch_store.get_reconfig_state_read_lock_guard()),
                &epoch_store,
            )
            .unwrap();
        waiter.await.unwrap();
    });

    t1.await.unwrap();
    t2.await.unwrap();
}

/// Regression test for the inverted condition to re-submit
/// `EndOfPublish` in `ConsensusAdapter::submit_recovered`.
///
/// The original condition was:
/// ```rust
/// if recovered
///     .iter()
///     .any(ConsensusTransaction::is_end_of_publish)
/// {
///     recovered.push(ConsensusTransaction::EndOfPublish)
/// }
/// ```
/// This was a bug since the logic is backwards - it added a duplicate
/// `EndOfPublish` when one was already in the DB, and did nothing when
/// `EndOfPublish` was missing (the exact crash recovery case).
///
/// The fix adds `!` to make the condition correct.
///
/// This test covers two crash scenarios:
///
/// Scenario 1: crash between pending consensus certificates removal and
/// `EndOfPublish` submission. In this case, a node is in `RejectUserCerts`
/// state, pending consensus certificates are empty, but `EndOfPublish` was
/// never persisted. Without the fix, `submit_recovered` submits nothing,
/// in which case epoch stalls permanently. Scenario 1 covers both cases
/// described in the comments in `ConsensusAdapter::submit_recovered`.
///
/// Scenario 2: crash after `EndOfPublish` was persisted but before it was
/// sequenced. Without the fix, a duplicate `EndOfPublish` is added and two
/// are submitted instead of one.
#[tokio::test]
async fn submit_recovered_end_of_publish_crash_recovery() {
    use consensus_core::{BlockRef, BlockStatus};
    use tokio::sync::Notify;

    use crate::mock_consensus::with_block_status;

    /// A minimal consensus client that records what was submitted to it.
    /// The `notify` fires each time `submit` is called, allowing the test
    /// to synchronize without polling.
    struct RecordingClient {
        submitted: Arc<Mutex<Vec<ConsensusTransaction>>>,
        notify: Arc<Notify>,
    }

    #[async_trait::async_trait]
    impl ConsensusClient for RecordingClient {
        /// Return `BlockStatus::Sequenced` so `submit_inner` resolves.
        /// The task then waits on the consensus-processed notification,
        /// which is harmless for this test.
        async fn submit(
            &self,
            transactions: &[ConsensusTransaction],
            _epoch_store: &Arc<AuthorityPerEpochStore>,
        ) -> IotaResult<BlockStatusReceiver> {
            self.submitted.lock().extend_from_slice(transactions);
            self.notify.notify_one();

            Ok(with_block_status(BlockStatus::Sequenced(BlockRef::MIN)))
        }
    }

    // -----------------------------------------------------------------------
    // Scenario 1: crash after all pending consensus certificates were removed
    // but before `EndOfPublish` was ever persisted.
    //
    // State on restart:
    //   - Reconfig state: `RejectUserCerts`.
    //   - Pending consensus certificates are empty.
    //   - `EndOfPublish` was not persisted before crash.
    //
    // Expected behavior: `ConsensusAdapter::submit_recovered` synthesizes a new
    // `EndOfPublish` and submits it.
    // -----------------------------------------------------------------------
    {
        let state = init_state_with_objects(vec![]).await;
        let epoch_store = state.epoch_store_for_testing();
        epoch_store.close_user_certs(epoch_store.get_reconfig_state_write_lock_guard());

        // Verify that all pre-conditions match the crash scenario.
        assert!(
            epoch_store
                .get_reconfig_state_read_lock_guard()
                .is_reject_user_certs(),
            "Scenario 1: reconfig state must be RejectUserCerts"
        );
        assert!(
            epoch_store.pending_consensus_certificates_empty(),
            "Scenario 1: pending consensus certificates must be empty"
        );
        assert!(
            !epoch_store
                .get_all_pending_consensus_transactions()
                .iter()
                .any(|tx| tx.is_end_of_publish()),
            "Scenario 1: `EndOfPublish` must not be persisted in DB before crash"
        );

        let submitted = Arc::new(Mutex::new(vec![]));
        let notify = Arc::new(Notify::new());
        let adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(RecordingClient {
                submitted: submitted.clone(),
                notify: notify.clone(),
            }),
            state.checkpoint_store.clone(),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
        ));

        adapter.submit_recovered(&epoch_store);

        // Wait for the spawned task to reach the mock's `submit`.
        // A timeout here means `EndOfPublish` was never submitted,
        // in which case the epoch stalls - this was the bug the
        // original condition caused.
        tokio::time::timeout(Duration::from_secs(5), notify.notified())
            .await
            .expect(
                "Scenario 1: ConsensusAdapter::submit_recovered did not submit \
                    `EndOfPublish`, so epoch stalls",
            );

        assert!(
            submitted
                .lock()
                .iter()
                .any(|tx: &ConsensusTransaction| tx.is_end_of_publish()),
            "Scenario 1: ConsensusAdapter::submit_recovered must submit EndOfPublish after crash"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 2: crash after `EndOfPublish` was persisted but before it was
    // sequenced.
    //
    // State on restart:
    //   - Reconfig state: `RejectUserCerts`.
    //   - Pending consensus certificates are empty.
    //   - `EndOfPublish` was persisted before crash.
    //
    // Expected behavior: exactly one `EndOfPublish` submitted (the one
    // recovered from DB).
    // -----------------------------------------------------------------------
    {
        let state = init_state_with_objects(vec![]).await;
        let epoch_store = state.epoch_store_for_testing();
        epoch_store.close_user_certs(epoch_store.get_reconfig_state_write_lock_guard());

        // Simulate `EndOfPublish` persisted to DB before the crash.
        let end_of_publish = ConsensusTransaction::new_end_of_publish(state.name);
        epoch_store
            .insert_pending_consensus_transactions(&[end_of_publish], None)
            .expect("Scenario 2: failed to insert EndOfPublish");

        // Verify that all pre-conditions match the crash scenario.
        assert!(
            epoch_store
                .get_reconfig_state_read_lock_guard()
                .is_reject_user_certs(),
            "Scenario 2: reconfig state must be RejectUserCerts"
        );
        assert!(
            epoch_store.pending_consensus_certificates_empty(),
            "Scenario 2: pending consensus certificates must be empty"
        );
        assert!(
            epoch_store
                .get_all_pending_consensus_transactions()
                .iter()
                .any(|tx| tx.is_end_of_publish()),
            "Scenario 2: `EndOfPublish` must be persisted in DB before crash"
        );

        let submitted = Arc::new(Mutex::new(vec![]));
        let notify = Arc::new(Notify::new());
        let adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(RecordingClient {
                submitted: submitted.clone(),
                notify: notify.clone(),
            }),
            state.checkpoint_store.clone(),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
        ));

        adapter.submit_recovered(&epoch_store);

        tokio::time::timeout(Duration::from_secs(5), notify.notified())
            .await
            .expect(
                "Scenario 2: ConsensusAdapter::submit_recovered did not submit \
                    `EndOfPublish`, so epoch stalls",
            );

        // Allow any potential second submission task to also reach the mock.
        tokio::task::yield_now().await;

        let end_of_publish_count = submitted
            .lock()
            .iter()
            .filter(|tx: &&ConsensusTransaction| tx.is_end_of_publish())
            .count();

        // Without the fix, a duplicate `EndOfPublish` would be pushed.
        assert_eq!(
            end_of_publish_count, 1,
            "Scenario 2: ConsensusAdapter::submit_recovered must not duplicate `EndOfPublish` \
                if it was already in DB, got {end_of_publish_count} EndOfPublish"
        );
    }
}

/// Verify that UserTransactionV1 (pcool flow) is NOT persisted to the
/// pending_consensus_transactions WAL, while CertifiedTransactions still are.
#[tokio::test]
async fn user_transaction_v1_not_persisted_to_pending_consensus() {
    telemetry_subscribers::init_for_testing();

    let mut objects = test_gas_objects();
    let shared_object = Object::shared_for_testing();
    objects.push(shared_object.clone());
    let state = init_state_with_objects(objects).await;
    let epoch_store = state.epoch_store_for_testing();

    // Create a UserTransactionV1 from one of the test gas objects.
    let (sender, keypair) = deterministic_random_account_key();
    let rgp = epoch_store.reference_gas_price();
    let gas_object = state
        .get_object(&test_gas_objects()[0].id())
        .await
        .unwrap();
    let data = TransactionData::new_move_call(
        sender,
        IOTA_FRAMEWORK_PACKAGE_ID,
        ident_str!("object_basics").to_owned(),
        ident_str!("create").to_owned(),
        vec![],
        gas_object.compute_object_reference(),
        vec![
            CallArg::Object(ObjectArg::SharedObject {
                id: shared_object.id(),
                initial_shared_version: shared_object.version(),
                mutable: true,
            }),
            CallArg::Pure(16u64.to_le_bytes().to_vec()),
            CallArg::Pure(bcs::to_bytes(&AccountAddress::from(sender)).unwrap()),
        ],
        rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
        rgp,
    )
    .unwrap();
    let transaction = to_sender_signed_transaction(data, &keypair);
    let user_tx = ConsensusTransaction::new_user_transaction(transaction);

    // Insert via insert_pending_consensus_transactions (same path as submit_batch).
    epoch_store
        .insert_pending_consensus_transactions(&[user_tx], None)
        .expect("insert should succeed");

    // UserTransactionV1 must NOT appear in the WAL.
    let pending = epoch_store.get_all_pending_consensus_transactions();
    assert!(
        !pending.iter().any(|tx| tx.kind.is_user_transaction()),
        "UserTransactionV1 must not be persisted to pending_consensus_transactions"
    );

    // Verify that pending_consensus_certificates is unaffected.
    assert!(
        epoch_store.pending_consensus_certificates_empty(),
        "UserTransactionV1 must not appear in pending_consensus_certificates"
    );

    // Now insert a CertifiedTransaction - it SHOULD be persisted.
    let certificate = test_certificates(&state, shared_object)
        .await
        .pop()
        .unwrap();
    let cert_tx = ConsensusTransaction::new_certificate_message(&state.name, certificate);
    epoch_store
        .insert_pending_consensus_transactions(
            &[cert_tx],
            Some(&epoch_store.get_reconfig_state_read_lock_guard()),
        )
        .expect("insert should succeed");

    let pending = epoch_store.get_all_pending_consensus_transactions();
    assert_eq!(
        pending.len(),
        1,
        "Only the CertifiedTransaction should be persisted"
    );
    assert!(
        !pending[0].kind.is_user_transaction(),
        "The persisted transaction must be a CertifiedTransaction, not UserTransactionV1"
    );
}

/// Verify that submit_recovered filters out any stale UserTransactionV1
/// entries that may exist in the DB from before this change was deployed.
#[tokio::test]
async fn submit_recovered_filters_user_transaction_v1() {
    use consensus_core::{BlockRef, BlockStatus};
    use tokio::sync::Notify;

    use crate::mock_consensus::with_block_status;

    struct RecordingClient {
        submitted: Arc<Mutex<Vec<ConsensusTransaction>>>,
        notify: Arc<Notify>,
    }

    #[async_trait::async_trait]
    impl ConsensusClient for RecordingClient {
        async fn submit(
            &self,
            transactions: &[ConsensusTransaction],
            _epoch_store: &Arc<AuthorityPerEpochStore>,
        ) -> IotaResult<BlockStatusReceiver> {
            self.submitted.lock().extend_from_slice(transactions);
            self.notify.notify_one();
            Ok(with_block_status(BlockStatus::Sequenced(BlockRef::MIN)))
        }
    }

    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();

    // Simulate a stale UserTransactionV1 in the DB (from before the filtering
    // was deployed). We bypass insert_pending_consensus_transactions and write
    // directly to the DB table.
    let (sender, keypair) = deterministic_random_account_key();
    let rgp = epoch_store.reference_gas_price();
    let gas_object_id = ObjectID::random();
    let gas_object = Object::with_id_owner_for_testing(gas_object_id, sender);
    let data = TransactionData::new_transfer_iota(
        sender,
        sender,
        Some(1),
        gas_object.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
        rgp,
    );
    let transaction = to_sender_signed_transaction(data, &keypair);
    let user_tx = ConsensusTransaction::new_user_transaction(transaction);

    // Write directly to the underlying DB table, bypassing the filter
    // (simulating pre-upgrade data that was persisted before the filtering).
    epoch_store
        .tables()
        .expect("tables")
        .insert_pending_consensus_transaction_for_test(&user_tx)
        .expect("direct DB insert should succeed");

    // Also insert an EndOfPublish so submit_recovered has something to submit.
    let end_of_publish = ConsensusTransaction::new_end_of_publish(state.name);
    epoch_store
        .insert_pending_consensus_transactions(&[end_of_publish], None)
        .expect("insert should succeed");

    // Enter RejectUserCerts state so EndOfPublish is expected.
    epoch_store.close_user_certs(epoch_store.get_reconfig_state_write_lock_guard());

    let submitted = Arc::new(Mutex::new(vec![]));
    let notify = Arc::new(Notify::new());
    let adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(RecordingClient {
            submitted: submitted.clone(),
            notify: notify.clone(),
        }),
        state.checkpoint_store.clone(),
        state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    adapter.submit_recovered(&epoch_store);

    // Wait for submit_recovered to reach the mock.
    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("submit_recovered did not submit anything");

    tokio::task::yield_now().await;

    let submitted = submitted.lock();
    assert!(
        !submitted.iter().any(|tx| tx.kind.is_user_transaction()),
        "submit_recovered must not resubmit UserTransactionV1"
    );
    assert!(
        submitted.iter().any(|tx| tx.is_end_of_publish()),
        "submit_recovered must still submit EndOfPublish"
    );
}
