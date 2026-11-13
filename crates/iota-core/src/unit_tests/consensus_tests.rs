// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, time::Duration};

use iota_macros::sim_test;
use iota_protocol_config::{Chain, ProtocolConfig};
use iota_sdk_types::{
    CheckpointContents, CheckpointSummary, GasCostSummary, Identifier, ObjectId,
    SharedObjectReference, Transaction,
};
use iota_types::{
    base_types::{ExecutionDigests, random_object_ref},
    crypto::deterministic_random_account_private_key,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt, CheckpointSignatureMessage,
        CheckpointSummaryExt, SignedCheckpointSummary,
    },
    messages_consensus::AuthorityCapabilitiesV1,
    object::Object,
    supported_protocol_versions::SupportedProtocolVersions,
    transaction::{
        CallArg, CertifiedTransaction, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS, TransactionAPI,
    },
    utils::{make_committee_key_num, to_sender_signed_transaction},
};
use move_core_types::account_address::AccountAddress;
use parking_lot::Mutex;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use starfish_core::BlockStatus;
use tokio::time::sleep;

use super::*;
use crate::{
    authority::{
        AuthorityState, ExecutionEnv, authority_test_utils::drop_attestations,
        authority_tests::init_state_with_objects,
        shared_object_version_manager::AssignedTxAndVersions,
    },
    checkpoints::CheckpointServiceNoop,
    consensus_handler::SequencedConsensusTransaction,
    execution_scheduler::ExecutionSchedulerAPI,
    mock_consensus::with_block_status,
};

/// Fixture: a few test gas objects.
pub fn test_gas_objects() -> Vec<Object> {
    thread_local! {
        static GAS_OBJECTS: Vec<Object> = (0..4)
            .map(|_| {
                let gas_object_id = ObjectId::random();
                let (owner, _) = deterministic_random_account_private_key();
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
    let (sender, sender_key) = deterministic_random_account_private_key();
    let rgp = epoch_store.reference_gas_price();

    let mut certificates = Vec::new();
    let shared_object_arg = CallArg::Shared(SharedObjectReference::new(
        shared_object.id(),
        shared_object.version(),
        true,
    ));
    for gas_object in test_gas_objects() {
        // Object digest may be different in genesis than originally generated.
        let gas_object = authority.get_object(&gas_object.id()).unwrap();
        // Make a sample transaction.
        let module = "object_basics";
        let function = "create";

        let tx = Transaction::new_move_call(
            sender,
            ObjectId::FRAMEWORK,
            Identifier::from_static(module),
            Identifier::from_static(function),
            // type_args
            vec![],
            gas_object.object_ref(),
            // args
            vec![
                shared_object_arg.clone(),
                CallArg::Pure(16u64.to_le_bytes().to_vec()),
                CallArg::pure(&AccountAddress::new(sender.into_bytes())),
            ],
            rgp * TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
            rgp,
        )
        .unwrap();

        let transaction = epoch_store
            .verify_transaction(to_sender_signed_transaction(tx, &sender_key))
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
            let mut assigned_versions = Vec::new();
            let mut executed_via_checkpoint = 0;

            for tx in sequenced_transactions {
                if let Some(transaction_digest) = tx.transaction.executable_transaction_digest() {
                    if self.process_via_checkpoint.contains(&transaction_digest) {
                        epoch_store
                            .insert_finalized_transactions(
                                vec![transaction_digest].as_slice(),
                                10,
                                0,
                            )
                            .expect("Should not fail");
                        executed_via_checkpoint += 1;
                    } else {
                        let (txns, versions) = epoch_store
                            .process_consensus_transactions_for_tests(
                                vec![tx],
                                &checkpoint_service,
                                self.state.get_object_cache_reader().as_ref(),
                                &self.state.metrics,
                                true,
                                self.state.as_ref(),
                            )
                            .await?;
                        transactions.extend(drop_attestations(txns));
                        assigned_versions.extend(versions.0);
                    }
                } else {
                    let (txns, versions) = epoch_store
                        .process_consensus_transactions_for_tests(
                            vec![tx],
                            &checkpoint_service,
                            self.state.get_object_cache_reader().as_ref(),
                            &self.state.metrics,
                            true,
                            self.state.as_ref(),
                        )
                        .await?;
                    transactions.extend(drop_attestations(txns));
                    assigned_versions.extend(versions.0);
                }
            }

            assert_eq!(
                executed_via_checkpoint,
                self.process_via_checkpoint.len(),
                "Some transactions were not executed via checkpoint"
            );

            let assigned_versions = AssignedTxAndVersions::new(assigned_versions).into_map();
            let transactions: Vec<_> = transactions
                .into_iter()
                .map(|tx| {
                    let key = tx.key();
                    (
                        tx,
                        ExecutionEnv::new().with_assigned_versions(
                            assigned_versions.get(&key).cloned().unwrap_or_default(),
                        ),
                    )
                })
                .collect();

            if self.execute {
                self.state
                    .execution_scheduler()
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
        50,
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
        with_block_status(BlockStatus::GarbageCollected(
            starfish_core::TransactionRef::default(),
        )),
        with_block_status(BlockStatus::GarbageCollected(
            starfish_core::TransactionRef::default(),
        )),
        with_block_status(BlockStatus::GarbageCollected(
            starfish_core::TransactionRef::default(),
        )),
        with_block_status(BlockStatus::Sequenced(
            starfish_core::TransactionRef::default(),
        )),
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
        vec![with_block_status(BlockStatus::Sequenced(
            starfish_core::TransactionRef::default(),
        ))],
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
        vec![with_block_status(BlockStatus::Sequenced(
            starfish_core::TransactionRef::default(),
        ))],
    );

    let checkpoint_summary = CheckpointSummary::new_with_protocol_config(
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
        Vec::new(),
        Default::default(),
    );

    let authority_key = &keys[0];
    let authority = authority_key.into();
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
            let delay = Duration::from_millis(rand::rng().random_range(0..1000));
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
    use starfish_core::BlockStatus;
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

            Ok(with_block_status(BlockStatus::Sequenced(
                starfish_core::TransactionRef::default(),
            )))
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
            50,
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
            50,
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

/// Builds a capability notification from `authority` that lists
/// `num_system_packages` system packages, so that its size grows with that
/// number.
fn make_capability_notification(
    authority: AuthorityName,
    num_system_packages: usize,
) -> ConsensusTransaction {
    ConsensusTransaction::new_capability_notification_v1(AuthorityCapabilitiesV1::new(
        authority,
        Chain::Unknown,
        SupportedProtocolVersions::SYSTEM_DEFAULT,
        vec![random_object_ref(); num_system_packages],
    ))
}

/// Builds a capability notification from `authority` larger than
/// [`ProtocolConfig::max_transaction_size_bytes()`].
fn make_oversized_capability_notification(
    authority: AuthorityName,
    protocol_config: &ProtocolConfig,
) -> ConsensusTransaction {
    let max_size = protocol_config.max_transaction_size_bytes();
    // An object reference takes more than 64 bytes.
    let transaction = make_capability_notification(authority, (max_size / 64) as usize);
    assert!(bcs::serialized_size(&transaction).unwrap() as u64 > max_size);

    transaction
}

/// Returns whether `result` is the error for a submission larger than
/// consensus accepts.
fn is_size_limit_exceeded<T>(result: &IotaResult<T>) -> bool {
    matches!(
        result,
        Err(IotaError::UserInput {
            error: UserInputError::SizeLimitExceeded { .. }
        })
    )
}

/// A consensus client that records every submission, then reports it as
/// sequenced or, with `refuses_as_too_large` set, returns the error the
/// Starfish client reports for an oversized submission.
struct RecordingConsensusClient {
    refuses_as_too_large: bool,
    submitted: Arc<Mutex<Vec<ConsensusTransaction>>>,
    notify: Arc<tokio::sync::Notify>,
}

impl RecordingConsensusClient {
    fn new(refuses_as_too_large: bool) -> Self {
        Self {
            refuses_as_too_large,
            submitted: Arc::new(Mutex::new(vec![])),
            notify: Arc::new(tokio::sync::Notify::new()),
        }
    }
}

#[async_trait::async_trait]
impl ConsensusClient for RecordingConsensusClient {
    async fn submit(
        &self,
        transactions: &[ConsensusTransaction],
        _epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<BlockStatusReceiver> {
        self.submitted.lock().extend_from_slice(transactions);
        self.notify.notify_one();
        fp_ensure!(
            !self.refuses_as_too_large,
            IotaError::UserInput {
                error: UserInputError::SizeLimitExceeded {
                    limit: "serialized consensus transaction size exceeded maximum of 0"
                        .to_string(),
                    value: "1".to_string(),
                }
            }
        );

        Ok(with_block_status(BlockStatus::Sequenced(
            starfish_core::TransactionRef::default(),
        )))
    }
}

/// Builds a consensus adapter for `state` over `consensus_client`, with
/// `submit_permits` submit permits.
fn make_consensus_adapter_with_client(
    state: &AuthorityState,
    consensus_client: Arc<dyn ConsensusClient>,
    submit_permits: usize,
) -> Arc<ConsensusAdapter> {
    Arc::new(ConsensusAdapter::new(
        consensus_client,
        state.checkpoint_store.clone(),
        state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        submit_permits,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
        50,
    ))
}

/// A submission fits only within every limit Starfish applies: the size of
/// each transaction, and the count and the bytes of one block.
#[test]
fn consensus_transactions_fit_only_within_the_starfish_limits() {
    let protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    let authority = AuthorityName::ZERO;

    let small = ConsensusTransaction::new_end_of_publish(authority);
    check_consensus_transactions_fit(std::slice::from_ref(&small), &protocol_config)
        .expect("a small transaction should fit");

    let oversized = make_oversized_capability_notification(authority, &protocol_config);
    assert!(
        is_size_limit_exceeded(&check_consensus_transactions_fit(
            std::slice::from_ref(&oversized),
            &protocol_config
        )),
        "a transaction over the size limit should not fit"
    );

    let too_many = vec![small; protocol_config.max_num_transactions_in_block() as usize + 1];
    assert!(
        is_size_limit_exceeded(&check_consensus_transactions_fit(
            &too_many,
            &protocol_config
        )),
        "more transactions than a block holds should not fit"
    );

    // Each one within the size limit, all of them over the bytes of a block.
    let max_size = protocol_config.max_transaction_size_bytes();
    let half_size = make_capability_notification(authority, (max_size / 2 / 80) as usize);
    let half_size_bytes = bcs::serialized_size(&half_size).unwrap() as u64;
    assert!(half_size_bytes <= max_size);
    let copies = protocol_config.max_transactions_in_block_bytes() / half_size_bytes + 1;
    assert!(copies <= protocol_config.max_num_transactions_in_block());
    let too_large_together = vec![half_size; copies as usize];
    assert!(
        is_size_limit_exceeded(&check_consensus_transactions_fit(
            &too_large_together,
            &protocol_config
        )),
        "transactions over the bytes of a block together should not fit"
    );
}

/// A submission exactly at a limit fits, as in Starfish, which refuses only
/// what is over a limit.
#[test]
fn consensus_transactions_fit_exactly_at_the_starfish_limits() {
    let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    let small = ConsensusTransaction::new_end_of_publish(AuthorityName::ZERO);

    let max_count = protocol_config.max_num_transactions_in_block() as usize;
    check_consensus_transactions_fit(&vec![small.clone(); max_count], &protocol_config)
        .expect("as many transactions as a block holds should fit");

    let size = bcs::serialized_size(&small).unwrap() as u64;
    protocol_config.set_consensus_max_transaction_size_bytes_for_testing(size);
    check_consensus_transactions_fit(std::slice::from_ref(&small), &protocol_config)
        .expect("a transaction of exactly the size limit should fit");

    protocol_config.set_consensus_max_transaction_size_bytes_for_testing(size - 1);
    assert!(
        is_size_limit_exceeded(&check_consensus_transactions_fit(
            std::slice::from_ref(&small),
            &protocol_config
        )),
        "a transaction one byte over the size limit should not fit"
    );
}

/// [`ConsensusAdapter::submit()`] refuses a transaction larger than consensus
/// accepts: it is neither stored nor handed to consensus, and takes no submit
/// permit.
#[tokio::test]
async fn submit_refuses_a_transaction_larger_than_consensus_accepts() {
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();
    let mut consensus_client = MockConsensusClient::new();
    consensus_client.expect_submit().never();
    let adapter = make_consensus_adapter_with_client(&state, Arc::new(consensus_client), 1);
    let oversized =
        make_oversized_capability_notification(state.name, epoch_store.protocol_config());

    let result = adapter.submit(oversized.clone(), None, &epoch_store);

    assert!(
        is_size_limit_exceeded(&result),
        "an oversized transaction should be refused"
    );
    assert!(
        epoch_store
            .get_all_pending_consensus_transactions()
            .iter()
            .all(|transaction| transaction.key() != oversized.key()),
        "a refused transaction must not be stored for submission"
    );
    assert_eq!(adapter.num_inflight_transactions(), 0);
    adapter
        .check_consensus_overload()
        .expect("a refused transaction must hold no submit permit");
}

/// [`ConsensusAdapter::submit_recovered()`] removes stored transactions larger
/// than consensus accepts instead of submitting them, and still submits the
/// others. Only a build that did not check sizes could have stored them.
#[tokio::test]
async fn submit_recovered_drops_stored_transactions_larger_than_consensus_accepts() {
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();
    let oversized =
        make_oversized_capability_notification(state.name, epoch_store.protocol_config());
    let end_of_publish = ConsensusTransaction::new_end_of_publish(state.name);
    epoch_store
        .insert_pending_consensus_transactions(std::slice::from_ref(&oversized), None)
        .unwrap();
    epoch_store
        .insert_pending_consensus_transactions(&[end_of_publish], None)
        .unwrap();
    let consensus_client = RecordingConsensusClient::new(false);
    let submitted = consensus_client.submitted.clone();
    let notify = consensus_client.notify.clone();
    let adapter = make_consensus_adapter_with_client(&state, Arc::new(consensus_client), 1);

    adapter.submit_recovered(&epoch_store);

    assert!(
        epoch_store
            .get_all_pending_consensus_transactions()
            .iter()
            .all(|transaction| transaction.key() != oversized.key()),
        "the oversized transaction should be removed from the pending table"
    );
    tokio::time::timeout(Duration::from_secs(5), notify.notified())
        .await
        .expect("the stored EndOfPublish should still be submitted");
    let submitted = submitted.lock();
    assert!(
        submitted
            .iter()
            .any(ConsensusTransaction::is_end_of_publish)
    );
    assert!(
        submitted
            .iter()
            .all(|transaction| transaction.key() != oversized.key()),
        "the oversized transaction must not be submitted"
    );
}

/// When consensus refuses a submission as larger than it accepts, the task
/// submitting it stops instead of retrying: it removes the transaction from
/// the pending table and frees its submit permit.
#[tokio::test]
async fn a_submission_consensus_refuses_as_too_large_is_not_retried() {
    let state = init_state_with_objects(vec![]).await;
    let epoch_store = state.epoch_store_for_testing();
    // The client refuses even a small transaction, as consensus would if its
    // limits were lower than those the adapter checks.
    let consensus_client = RecordingConsensusClient::new(true);
    let submitted = consensus_client.submitted.clone();
    let adapter = make_consensus_adapter_with_client(&state, Arc::new(consensus_client), 1);
    let end_of_publish = ConsensusTransaction::new_end_of_publish(state.name);

    let task = adapter
        .submit(end_of_publish.clone(), None, &epoch_store)
        .expect("a small transaction should be accepted for submission");
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("the submission task should stop after the refusal")
        .expect("the submission task should not panic");

    assert_eq!(
        submitted.lock().len(),
        1,
        "a refused submission must not be retried"
    );
    assert!(
        epoch_store
            .get_all_pending_consensus_transactions()
            .iter()
            .all(|transaction| transaction.key() != end_of_publish.key()),
        "a refused transaction should be removed from the pending table"
    );
    assert_eq!(adapter.num_inflight_transactions(), 0);
    adapter
        .check_consensus_overload()
        .expect("the submit permit should be free again");
}
