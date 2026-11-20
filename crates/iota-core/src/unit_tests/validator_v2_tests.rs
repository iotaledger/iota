// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{num::NonZeroUsize, sync::Arc, time::Duration};

use iota_config::node::AuthorityOverloadConfig;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, ObjectId, SharedObjectReference, Transaction, TransactionDigest};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    attestation::Attestation,
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    messages_grpc::TxStatusUpdate,
    object::{OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    traffic_control::Weight,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI, TransactionEnvelope},
    utils::to_sender_signed_transaction,
};
use tokio::sync::mpsc;
use typed_store::Map;

use super::{ValidatorService, weigh_owned_input_rejection};
use crate::{
    authority::{
        AuthorityState, ExecutionEnv, abstract_account_test_utils::AbstractAccountTestEnv,
        authority_per_epoch_store::AuthorityPerEpochStore,
        test_authority_builder::TestAuthorityBuilder,
    },
    authority_server::{ValidatorServiceMetrics, soft_lock::PreConsensusSoftLocks},
    checkpoints::CheckpointStore,
    consensus_adapter::{
        ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics,
        MockConsensusClient,
    },
    mock_consensus::with_block_status,
};

/// Submits a transaction to `submit_single_tx` with
/// `enable_validator_attestation` on and asserts that the message reaching the
/// consensus adapter is a `UserTransactionV1` carrying an
/// `Attestation::Authority` whose `authority_index` matches this validator's
/// own position in the consensus committee.
#[tokio::test]
async fn test_submit_single_tx_produces_an_attested_user_transaction() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ])
        .build()
        .await;

    // Intercept whatever reaches the consensus adapter.
    let (captured_tx, mut rx) = mpsc::channel::<ConsensusTransaction>(1);
    let mut mock = MockConsensusClient::new();
    mock.expect_submit().returning(move |transactions, _| {
        let _ = captured_tx.try_send(transactions[0].clone());
        Ok(with_block_status(starfish_core::BlockStatus::Sequenced(
            starfish_core::TransactionRef::default(),
        )))
    });

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(mock),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
        50,
    ));

    let epoch_store = authority_state.load_epoch_store_one_call_per_task();
    let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());
    let soft_locks = Arc::new(PreConsensusSoftLocks::new());

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).unwrap();
    let gas = authority_state.get_object(&gas_id).unwrap();

    let tx_data = Transaction::new_transfer(
        dbg_addr(2),
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let (update, _weight) = ValidatorService::submit_single_tx(
        &authority_state,
        &consensus_adapter,
        &metrics,
        &epoch_store,
        &soft_locks,
        tx,
    )
    .await;

    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}",
    );

    // Assert the consensus message is a `UserTransactionV1` carrying
    // `Attestation::Authority`.
    let consensus_tx = rx
        .recv()
        .await
        .expect("consensus message should have been captured");
    let ConsensusTransactionKind::UserTransactionV1(attested) = consensus_tx.kind else {
        panic!("expected UserTransactionV1, got {:?}", consensus_tx.kind);
    };
    let Some(Attestation::Authority {
        authority_index, ..
    }) = &attested.attestation
    else {
        panic!(
            "expected an authority attestation, got {:?}",
            attested.attestation
        );
    };

    // The authority_index must match this validator's position in the consensus
    // committee — mirrors the lookup performed in submit_single_tx.
    let expected_index = epoch_store
        .committee()
        .authority_index(&authority_state.name)
        .and_then(|i| u8::try_from(i).ok())
        .expect("authority must be present in the consensus committee");
    assert_eq!(*authority_index, expected_index);
}

/// Submits a transaction to `submit_single_tx` with
/// `enable_validator_attestation` on, where the gas object referenced by the
/// transaction does not exist in the authority store. Asserts that the call
/// returns `TxStatusUpdate::Rejected` and that no message is ever forwarded to
/// the consensus adapter.
#[tokio::test]
async fn test_submit_single_tx_attest_failure_rejected_without_reaching_consensus() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    // Only register object_id; gas_id is intentionally absent from the store.
    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[Object::with_id_owner_for_testing(object_id, sender)])
        .build()
        .await;

    // Consensus must never be reached — any submit call panics the test.
    let mut mock = MockConsensusClient::new();
    mock.expect_submit().never();

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(mock),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
        50,
    ));

    let epoch_store = authority_state.load_epoch_store_one_call_per_task();
    let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());
    let soft_locks = Arc::new(PreConsensusSoftLocks::new());

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).unwrap();
    // Build a gas object reference locally so the transaction is structurally
    // valid, but never store it in the authority — attest_transaction must fail.
    let gas_ref = Object::with_id_owner_for_testing(gas_id, sender).object_ref();

    let tx_data = Transaction::new_transfer(
        dbg_addr(2),
        object.object_ref(),
        sender,
        gas_ref,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let (update, _weight) = ValidatorService::submit_single_tx(
        &authority_state,
        &consensus_adapter,
        &metrics,
        &epoch_store,
        &soft_locks,
        tx,
    )
    .await;

    assert!(
        matches!(update, TxStatusUpdate::Rejected { .. }),
        "expected Rejected, got {update:?}",
    );
}

/// Submits to `submit_single_tx`, with `enable_validator_attestation` on, a
/// transaction that passes its own size check, but whose attestation, one
/// object reference per shared input, makes the consensus transaction larger
/// than consensus accepts. Asserts that the validator rejects it with the size
/// limit and a traffic weight of one, without taking a soft lock or handing
/// anything to the consensus adapter.
#[tokio::test]
async fn test_submit_single_tx_rejects_an_attested_transaction_too_large_for_consensus() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config
    });

    // A read-only shared input takes about 43 bytes in the transaction and 73
    // in the attestation's object reference. With the pure padding, the
    // transaction stays under `max_tx_size_bytes` and the attested transaction
    // exceeds `max_transaction_size_bytes`.
    const SHARED_INPUTS: usize = 2_040;
    const PADDING_INPUTS: u8 = 3;
    const PADDING_BYTES: usize = 12_000;

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let shared_objects: Vec<_> = (0..SHARED_INPUTS)
        .map(|_| Object::shared_for_testing())
        .collect();
    let mut objects = shared_objects.clone();
    objects.push(Object::with_id_owner_for_testing(gas_id, sender));

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&objects)
        .build()
        .await;

    // Consensus must never be reached — any submit call panics the test.
    let mut mock = MockConsensusClient::new();
    mock.expect_submit().never();

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(mock),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
        50,
    ));

    let epoch_store = authority_state.load_epoch_store_one_call_per_task();
    let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());
    let soft_locks = Arc::new(PreConsensusSoftLocks::new());

    let mut builder = ProgrammableTransactionBuilder::new();
    for object in &shared_objects {
        builder
            .input(CallArg::Shared(SharedObjectReference::new(
                object.id(),
                OBJECT_START_VERSION,
                false,
            )))
            .unwrap();
    }
    for padding in 0..PADDING_INPUTS {
        builder.pure_bytes(vec![padding; PADDING_BYTES], false);
    }
    builder.transfer_iota(dbg_addr(2), Some(1));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let gas = authority_state.get_object(&gas_id).unwrap();
    let tx_data = Transaction::new_programmable(
        sender,
        vec![gas.object_ref()],
        builder.finish(),
        rgp * 1_000_000,
        rgp,
    );
    let protocol_config = epoch_store.protocol_config();
    tx_data
        .check_serialized_size(protocol_config)
        .expect("the transaction alone must pass its size check");
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let (update, weight) = ValidatorService::submit_single_tx(
        &authority_state,
        &consensus_adapter,
        &metrics,
        &epoch_store,
        &soft_locks,
        tx,
    )
    .await;

    let TxStatusUpdate::Rejected {
        error:
            IotaError::UserInput {
                error: UserInputError::SizeLimitExceeded { limit, value },
            },
    } = &update
    else {
        panic!("expected the consensus transaction size limit, got {update:?}");
    };
    assert!(
        limit.contains("consensus transaction"),
        "expected the consensus transaction size limit, got {limit}"
    );
    let size: u64 = value.parse().unwrap();
    assert!(
        size > protocol_config.max_transaction_size_bytes(),
        "the attested transaction must exceed the consensus limit, got {size}"
    );
    assert_eq!(
        weight.value(),
        Weight::one().value(),
        "the rejection must weigh"
    );
    assert_eq!(soft_locks.lock_count(), 0, "no soft lock may be kept");
    assert!(
        epoch_store
            .get_all_pending_consensus_transactions()
            .is_empty(),
        "nothing may reach the consensus adapter"
    );
}

/// Submits a transfer to `submit_single_tx`, first with the consensus queue
/// full, then with room in it. With the queue full, asserts that the transfer
/// is rejected before anything reaches the consensus adapter, and that the
/// number of transactions in the queue is unchanged. With room, asserts that
/// the transfer holds one slot as soon as the call returns `Submitted`, before
/// its submission task starts, and still that one slot once it reaches
/// consensus.
#[tokio::test]
async fn test_submit_single_tx_counts_a_transaction_in_the_consensus_queue_from_admission() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ])
        .build()
        .await;

    // Intercept whatever reaches the consensus adapter.
    let (captured_tx, mut rx) = mpsc::channel::<ConsensusTransaction>(1);
    let mut mock = MockConsensusClient::new();
    mock.expect_submit().returning(move |transactions, _| {
        let _ = captured_tx.try_send(transactions[0].clone());
        Ok(with_block_status(starfish_core::BlockStatus::Sequenced(
            starfish_core::TransactionRef::default(),
        )))
    });

    let max_pending_transactions: usize = 10;
    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(mock),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        max_pending_transactions,
        max_pending_transactions,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
        50,
    ));

    let epoch_store = authority_state.load_epoch_store_one_call_per_task();
    let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());
    let soft_locks = Arc::new(PreConsensusSoftLocks::new());

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).unwrap();
    let gas = authority_state.get_object(&gas_id).unwrap();

    let tx_data = Transaction::new_transfer(
        dbg_addr(2),
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    // The queue is full: the transfer is rejected at admission, and its slot
    // is released.
    consensus_adapter.set_num_inflight_transactions_for_testing(max_pending_transactions as u64);
    let (update, _weight) = ValidatorService::submit_single_tx(
        &authority_state,
        &consensus_adapter,
        &metrics,
        &epoch_store,
        &soft_locks,
        tx.clone(),
    )
    .await;

    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::TooManyTransactionsPendingConsensus
            }
        ),
        "expected a rejection for the full consensus queue, got {update:?}",
    );
    assert_eq!(
        consensus_adapter.num_inflight_transactions(),
        max_pending_transactions as u64
    );
    assert!(
        epoch_store
            .get_all_pending_consensus_transactions()
            .is_empty(),
        "nothing may reach the consensus adapter"
    );

    // The queue has room: the transfer is submitted. The test runtime runs one
    // task at a time, and `submit_single_tx` returns right after it spawns the
    // submission task, so that task has not started yet. The count includes
    // the slot taken at admission; a submission that took its own slot when
    // its task starts would leave it at 0.
    consensus_adapter.set_num_inflight_transactions_for_testing(0);
    let (update, _weight) = ValidatorService::submit_single_tx(
        &authority_state,
        &consensus_adapter,
        &metrics,
        &epoch_store,
        &soft_locks,
        tx,
    )
    .await;

    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}",
    );
    assert_eq!(consensus_adapter.num_inflight_transactions(), 1);

    // The submission task keeps that one slot, without taking another, until
    // consensus has processed the transfer.
    rx.recv()
        .await
        .expect("the transfer should have reached consensus");
    assert_eq!(consensus_adapter.num_inflight_transactions(), 1);
}

/// Enables the P-COOL flow and validator attestation, with the checkpoint
/// summary version that attestation requires, until the returned guard is
/// dropped.
fn enable_attestation() -> impl Drop {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);

        config
    })
}

/// Enables the P-COOL flow and transaction dependencies, with validator
/// attestation as `attestation` says, until the returned guard is dropped.
fn enable_transaction_dependencies(attestation: bool) -> impl Drop {
    ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(attestation);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config.set_enable_transaction_dependencies_for_testing(true);

        config
    })
}

/// Returns an overload config that allows `max_concurrent` attestation dry
/// runs at once and `max_queued` submissions waiting for one, with a wait
/// limit longer than any test waits.
fn dry_run_limits(max_concurrent: usize, max_queued: usize) -> AuthorityOverloadConfig {
    AuthorityOverloadConfig {
        max_concurrent_attestations: Some(
            NonZeroUsize::new(max_concurrent).expect("a test should allow at least one dry run"),
        ),
        max_queued_attestations: Some(max_queued),
        max_attestation_wait: Some(Duration::from_secs(30)),
        ..Default::default()
    }
}

/// A validator to submit transactions to, whose consensus adapter forwards
/// every message it submits to a channel.
#[derive(Clone)]
struct TestValidator {
    state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    epoch_store: Arc<AuthorityPerEpochStore>,
    metrics: Arc<ValidatorServiceMetrics>,
    soft_locks: Arc<PreConsensusSoftLocks>,
}

impl TestValidator {
    /// A validator that starts with `starting_objects` and uses
    /// `overload_config`, with the receiver of what it submits to consensus.
    async fn new(
        starting_objects: &[Object],
        overload_config: AuthorityOverloadConfig,
    ) -> (Self, mpsc::Receiver<ConsensusTransaction>) {
        let state = TestAuthorityBuilder::new()
            .with_starting_objects(starting_objects)
            .with_authority_overload_config(overload_config)
            .build()
            .await;

        Self::from_state(state)
    }

    /// Like [`Self::new()`], for a validator the caller built.
    fn from_state(state: Arc<AuthorityState>) -> (Self, mpsc::Receiver<ConsensusTransaction>) {
        // Each test submits far fewer messages than this.
        let (sender, submitted) = mpsc::channel(16);
        let mut consensus_client = MockConsensusClient::new();
        consensus_client
            .expect_submit()
            .returning(move |transactions, _| {
                for transaction in transactions {
                    let _ = sender.try_send(transaction.clone());
                }

                Ok(with_block_status(starfish_core::BlockStatus::Sequenced(
                    starfish_core::TransactionRef::default(),
                )))
            });
        let consensus_adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(consensus_client),
            CheckpointStore::new_for_tests(),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
            50,
        ));
        let epoch_store = Arc::clone(&state.load_epoch_store_one_call_per_task());

        let validator = Self {
            state,
            consensus_adapter,
            epoch_store,
            metrics: Arc::new(ValidatorServiceMetrics::new_for_tests()),
            soft_locks: Arc::new(PreConsensusSoftLocks::new()),
        };

        (validator, submitted)
    }

    async fn submit(&self, transaction: TransactionEnvelope) -> (TxStatusUpdate, Weight) {
        ValidatorService::submit_single_tx(
            &self.state,
            &self.consensus_adapter,
            &self.metrics,
            &self.epoch_store,
            &self.soft_locks,
            transaction,
        )
        .await
    }
}

/// A transfer of `object` to another address, paid with `gas`, both owned by
/// `sender`, and signed with `signing_key`.
fn transfer(
    state: &AuthorityState,
    sender: Address,
    signing_key: &AccountPrivateKey,
    object: ObjectId,
    gas: ObjectId,
) -> TransactionEnvelope {
    let rgp = state.reference_gas_price_for_testing().unwrap();
    let tx_data = Transaction::new_transfer(
        dbg_addr(2),
        state.get_object(&object).unwrap().object_ref(),
        sender,
        state.get_object(&gas).unwrap().object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );

    to_sender_signed_transaction(tx_data, signing_key)
}

/// `count` new objects owned by `owner`, with their ids.
fn objects_owned_by(owner: Address, count: usize) -> (Vec<ObjectId>, Vec<Object>) {
    let ids: Vec<ObjectId> = (0..count).map(|_| ObjectId::random()).collect();
    let objects = ids
        .iter()
        .map(|id| Object::with_id_owner_for_testing(*id, owner))
        .collect();

    (ids, objects)
}

fn digest_of(consensus_transaction: ConsensusTransaction) -> TransactionDigest {
    let ConsensusTransactionKind::UserTransactionV1(attested) = consensus_transaction.kind else {
        panic!(
            "expected UserTransactionV1, got {:?}",
            consensus_transaction.kind
        );
    };

    *attested.transaction.digest()
}

/// Executes `transaction` on `validator` with success, without recording it
/// as a root of a commit.
fn execute_without_root_commit(validator: &TestValidator, transaction: TransactionEnvelope) {
    let executable = VerifiedExecutableTransaction::new_from_checkpoint(
        validator
            .epoch_store
            .verify_transaction(transaction)
            .unwrap(),
        validator.epoch_store.epoch(),
        1,
    );
    validator
        .epoch_store
        .assign_to_a_new_commit_for_testing(&[executable.key()]);
    let (effects, _) = validator
        .state
        .try_execute_immediately(&executable, ExecutionEnv::new(), &validator.epoch_store)
        .unwrap();
    assert!(effects.status().is_success(), "{effects:?}");
}

/// Asserts that a submission was rejected with the retriable
/// `TransactionDependencyNotFound` for `dependency`, and returns its weight.
fn assert_dependency_not_found(
    (update, weight): (TxStatusUpdate, Weight),
    dependency: TransactionDigest,
) -> f32 {
    assert!(
        matches!(
            &update,
            TxStatusUpdate::Rejected {
                error: IotaError::UserInput {
                    error: UserInputError::TransactionDependencyNotFound { digest },
                },
            } if *digest == dependency
        ),
        "expected the dependency not to be found, got {update:?}"
    );

    weight.value()
}

/// Waits until `count` submissions wait for a dry run on `validator`.
async fn wait_until_queued(validator: &TestValidator, count: i64) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while validator.state.metrics.attestations_waiting.get() < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the submissions should wait for a dry run");
}

/// Submits `first` and `second`, which use the same coin, at the same time on
/// `validator`. Asserts that one is submitted to consensus, that the other is
/// refused for the soft lock conflict at no weight, and that only one dry run
/// ran.
async fn assert_conflicting_submissions_get_one_dry_run(
    validator: &TestValidator,
    first: TransactionEnvelope,
    second: TransactionEnvelope,
) {
    let submissions = [first, second].map(|transaction| {
        let validator = validator.clone();

        tokio::spawn(async move { validator.submit(transaction).await })
    });
    let mut results = Vec::new();
    for submission in submissions {
        results.push(
            submission
                .await
                .expect("the submission task must not panic"),
        );
    }

    let submitted = results
        .iter()
        .filter(|(update, _)| matches!(update, TxStatusUpdate::Submitted))
        .count();
    let refused: Vec<_> = results
        .iter()
        .filter(|(update, _)| {
            matches!(
                update,
                TxStatusUpdate::Rejected {
                    error: IotaError::ObjectLockConflict { .. }
                }
            )
        })
        .collect();
    assert_eq!(submitted, 1);
    assert_eq!(refused.len(), 1);
    assert_eq!(
        refused[0].1.value(),
        0.0,
        "a refusal before the dry run weighs nothing"
    );
    assert_eq!(validator.state.metrics.attestation_dry_runs_total.get(), 1);
}

fn assert_overload_rejection((update, weight): &(TxStatusUpdate, Weight)) {
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::ValidatorOverloadedRetryAfter { .. }
            }
        ),
        "expected an overload rejection, got {update:?}"
    );
    assert_eq!(
        weight.value(),
        0.0,
        "an overload rejection weighs nothing in the traffic tally"
    );
}

/// With every dry run taken and no place left to wait, a submission is
/// refused for overload before its signature is checked, and the refusal
/// weighs nothing. Once the dry run frees, the signature decides: the copy
/// signed with another key is refused, and the signed one reaches consensus.
#[tokio::test]
async fn submission_finding_every_dry_run_taken_is_refused_before_its_signature_is_checked() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (_, other_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 2);
    let (validator, mut submitted) = TestValidator::new(&objects, dry_run_limits(1, 0)).await;
    let signed = transfer(&validator.state, sender, &sender_key, ids[0], ids[1]);
    let badly_signed = transfer(&validator.state, sender, &other_key, ids[0], ids[1]);

    let held = validator.state.acquire_attestation_permit().await.unwrap();
    assert_overload_rejection(&validator.submit(badly_signed.clone()).await);
    assert_overload_rejection(&validator.submit(signed.clone()).await);
    assert_eq!(
        validator
            .metrics
            .num_rejected_tx_during_overload
            .with_label_values(&["ValidatorOverloadedRetryAfter"])
            .get(),
        2
    );
    assert_eq!(
        validator
            .state
            .metrics
            .transaction_overload_sources
            .with_label_values(&["attestation"])
            .get(),
        2
    );

    drop(held);
    let (update, _) = validator.submit(badly_signed).await;
    assert!(
        matches!(&update, TxStatusUpdate::Rejected { error } if !error.is_retryable_overload()),
        "with a dry run free, the signature decides, got {update:?}"
    );
    let (update, _) = validator.submit(signed.clone()).await;
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}"
    );
    assert_eq!(digest_of(submitted.recv().await.unwrap()), *signed.digest());
}

/// A submission that finds every dry run taken waits for one while the queue
/// has room, and the next submission is refused for overload.
#[tokio::test]
async fn submission_waits_for_free_dry_run_while_queue_has_room() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 4);
    let (validator, mut submitted) = TestValidator::new(&objects, dry_run_limits(1, 1)).await;
    let first = transfer(&validator.state, sender, &sender_key, ids[0], ids[1]);
    let second = transfer(&validator.state, sender, &sender_key, ids[2], ids[3]);

    let held = validator.state.acquire_attestation_permit().await.unwrap();
    let waiting = tokio::spawn({
        let validator = validator.clone();
        let first = first.clone();

        async move { validator.submit(first).await }
    });
    wait_until_queued(&validator, 1).await;

    assert_overload_rejection(&validator.submit(second).await);

    drop(held);
    let (update, _) = waiting.await.unwrap();
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}"
    );
    assert_eq!(digest_of(submitted.recv().await.unwrap()), *first.digest());
    assert_eq!(validator.state.metrics.attestations_waiting.get(), 0);
    assert_eq!(validator.state.metrics.attestations_in_progress.get(), 0);
    assert_eq!(validator.state.metrics.attestations_rejected_total.get(), 1);
}

/// Once a transaction holds its soft locks, a second transaction on the same
/// gas coin is refused, and so is a resubmission of the first, both before a
/// dry run of their own.
#[tokio::test]
async fn second_transaction_on_same_gas_coin_is_refused() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 3);
    let (validator, mut submitted) =
        TestValidator::new(&objects, AuthorityOverloadConfig::default()).await;
    let gas = ids[2];
    let first = transfer(&validator.state, sender, &sender_key, ids[0], gas);
    let second = transfer(&validator.state, sender, &sender_key, ids[1], gas);

    let (update, _) = validator.submit(first.clone()).await;
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}"
    );
    assert_eq!(digest_of(submitted.recv().await.unwrap()), *first.digest());

    let (update, _) = validator.submit(second).await;
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::ObjectLockConflict { .. }
            }
        ),
        "expected a soft lock conflict, got {update:?}"
    );
    let (update, _) = validator.submit(first).await;
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::RecentlyResubmitted { .. }
            }
        ),
        "expected a duplicate, got {update:?}"
    );

    assert_eq!(
        validator.metrics.num_rejected_tx_soft_lock_conflict.get(),
        1
    );
    assert_eq!(
        validator.metrics.num_rejected_tx_recently_resubmitted.get(),
        1
    );
    // The first transaction's object and gas coin; the refused one left none.
    assert_eq!(validator.soft_locks.lock_count(), 2);
    assert_eq!(
        validator.state.metrics.attestation_dry_runs_total.get(),
        1,
        "only the first transaction may get a dry run"
    );
    assert!(submitted.try_recv().is_err());
}

/// When an earlier transaction of a Move account holds a soft lock on a coin,
/// the account's next transaction on that coin is refused before its signature
/// is checked and before any dry run, as a transaction signed with keys is.
#[tokio::test]
async fn move_account_transaction_on_held_coin_gets_no_dry_run() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let mut env = AbstractAccountTestEnv::new().await;
    let (first, second) = env.account_transactions_on_one_coin();
    let (validator, mut submitted) = TestValidator::from_state(env.authority.clone());

    let (update, _) = validator.submit(first.clone()).await;
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}"
    );
    assert_eq!(digest_of(submitted.recv().await.unwrap()), *first.digest());

    let (update, weight) = validator.submit(second).await;
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::ObjectLockConflict { .. }
            }
        ),
        "expected a soft lock conflict, got {update:?}"
    );
    assert_eq!(
        weight.value(),
        0.0,
        "a refusal before any dry run weighs nothing"
    );
    assert_eq!(
        validator.state.metrics.attestation_dry_runs_total.get(),
        1,
        "only the first transaction may get a dry run"
    );
    assert!(submitted.try_recv().is_err());
}

/// A Move account transaction whose authenticator needs more gas than
/// `max_auth_gas` is refused by the authenticators' own run, before its locks
/// are taken and before any dry run, with the error admission gives, which
/// weighs one.
#[tokio::test]
async fn move_account_transaction_over_max_auth_gas_is_refused_before_any_dry_run() {
    telemetry_subscribers::init_for_testing();
    // Ten gas units at the reference gas price, far below what the ed25519
    // verification of the account's authenticator costs.
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config.set_max_auth_gas_for_testing(10_000);

        config
    });

    let mut env = AbstractAccountTestEnv::new().await;
    let transaction = env.account_transaction();
    let (validator, mut submitted) = TestValidator::from_state(env.authority.clone());

    let (update, weight) = validator.submit(transaction).await;
    let TxStatusUpdate::Rejected {
        error: IotaError::MoveAuthenticatorExecutionFailure { error },
    } = &update
    else {
        panic!("expected a failed Move authentication, got {update:?}");
    };
    assert!(
        error.contains("Insufficient Gas"),
        "the authenticator must run out of max_auth_gas, got: {error}"
    );
    assert_eq!(
        weight.value(),
        1.0,
        "a failed Move authentication weighs one"
    );
    assert_eq!(
        validator.state.metrics.attestation_dry_runs_total.get(),
        0,
        "the authenticators' own run must refuse it before any dry run"
    );
    assert_eq!(validator.soft_locks.lock_count(), 0);
    assert!(submitted.try_recv().is_err());
}

/// A transaction whose inputs another transaction locks while it waits for a
/// dry run is refused once a dry run frees, before its own dry run.
#[tokio::test]
async fn lock_taken_while_waiting_for_dry_run_refuses_transaction_before_it() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 2);
    let (validator, mut submitted) = TestValidator::new(&objects, dry_run_limits(1, 1)).await;
    let tx = transfer(&validator.state, sender, &sender_key, ids[0], ids[1]);
    let gas = validator.state.get_object(&ids[1]).unwrap().object_ref();

    let held = validator.state.acquire_attestation_permit().await.unwrap();
    let waiting = tokio::spawn({
        let validator = validator.clone();

        async move { validator.submit(tx).await }
    });
    wait_until_queued(&validator, 1).await;

    validator
        .soft_locks
        .try_acquire(TransactionDigest::new([9; 32]), &[gas])
        .unwrap();
    drop(held);

    let (update, weight) = waiting.await.unwrap();
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::ObjectLockConflict { .. }
            }
        ),
        "expected a soft lock conflict, got {update:?}"
    );
    assert_eq!(
        weight.value(),
        0.0,
        "a refusal before the dry run weighs nothing"
    );
    assert_eq!(validator.state.metrics.attestation_dry_runs_total.get(), 0);
    assert!(submitted.try_recv().is_err());
}

/// A dry run's permit is freed only once its transaction holds its soft locks,
/// so a transaction on the same coin that waits for that permit is refused
/// before a dry run of its own.
#[tokio::test]
async fn dry_run_permit_is_held_until_soft_locks_are_taken() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 3);
    let (validator, mut submitted) = TestValidator::new(&objects, dry_run_limits(1, 2)).await;
    let gas = ids[2];
    let first = transfer(&validator.state, sender, &sender_key, ids[0], gas);
    let second = transfer(&validator.state, sender, &sender_key, ids[1], gas);

    // Both wait for the one permit, the first ahead of the second.
    let held = validator.state.acquire_attestation_permit().await.unwrap();
    let first_submission = tokio::spawn({
        let validator = validator.clone();
        let first = first.clone();

        async move { validator.submit(first).await }
    });
    wait_until_queued(&validator, 1).await;
    let second_submission = tokio::spawn({
        let validator = validator.clone();

        async move { validator.submit(second).await }
    });
    wait_until_queued(&validator, 2).await;
    drop(held);

    let (update, _) = first_submission.await.unwrap();
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}"
    );
    assert_eq!(digest_of(submitted.recv().await.unwrap()), *first.digest());

    let (update, weight) = second_submission.await.unwrap();
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::ObjectLockConflict { .. }
            }
        ),
        "expected a soft lock conflict, got {update:?}"
    );
    assert_eq!(
        weight.value(),
        0.0,
        "a refusal before the dry run weighs nothing"
    );
    assert_eq!(validator.state.metrics.attestation_dry_runs_total.get(), 1);
    assert!(submitted.try_recv().is_err());
}

/// A transaction whose inputs another transaction locks after its two checks
/// for conflicts is refused when it takes its own locks, before its dry run,
/// and the refusal weighs nothing. Not under the simulator: it has no blocking
/// pool to fill and cannot build a runtime of its own.
#[cfg(not(msim))]
#[test]
fn lock_taken_after_both_checks_refuses_transaction_before_dry_run() {
    // Validation and the dry run need a thread of the blocking pool. The pool
    // gets one thread, and the test holds it, so the transaction cannot take
    // its locks before the test has taken the conflicting one.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        telemetry_subscribers::init_for_testing();
        let _guard = enable_attestation();

        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let (ids, objects) = objects_owned_by(sender, 2);
        let (validator, mut submitted) =
            TestValidator::new(&objects, AuthorityOverloadConfig::default()).await;
        let tx = transfer(&validator.state, sender, &sender_key, ids[0], ids[1]);
        let gas = validator.state.get_object(&ids[1]).unwrap().object_ref();

        // Hold the pool's only thread until the conflicting lock is taken.
        let (running, is_running) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel::<()>();
        let holder = tokio::task::spawn_blocking(move || {
            running.send(()).unwrap();
            released.recv().unwrap();
        });
        is_running.await.unwrap();

        let submission = tokio::spawn({
            let validator = validator.clone();

            async move { validator.submit(tx).await }
        });
        // A submission holds its permit from before its second check for
        // conflicts until its dry run has ended. In between, it awaits only
        // the blocking task that validates it, takes its locks and runs its
        // dry run. The submission's task shares the test's single thread, so
        // once the count rises, the submission has passed both checks and its
        // blocking task waits for the pool's thread.
        tokio::time::timeout(Duration::from_secs(30), async {
            while validator.state.metrics.attestations_in_progress.get() < 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the submission should get a dry run permit");
        validator
            .soft_locks
            .try_acquire(TransactionDigest::new([9; 32]), &[gas])
            .unwrap();
        release.send(()).unwrap();
        holder.await.unwrap();

        let (update, weight) = submission.await.unwrap();
        assert!(
            matches!(
                update,
                TxStatusUpdate::Rejected {
                    error: IotaError::ObjectLockConflict { .. }
                }
            ),
            "expected a soft lock conflict, got {update:?}"
        );
        assert_eq!(
            weight.value(),
            0.0,
            "a refusal before the dry run weighs nothing"
        );
        assert_eq!(validator.state.metrics.attestation_dry_runs_total.get(), 0);
        assert_eq!(validator.soft_locks.lock_count(), 1);
        assert!(submitted.try_recv().is_err());
    });
}

/// Two transactions that pay with the same coin and arrive together get one
/// dry run between them, also when each can have a permit. Whichever passes
/// validation first takes the locks before its dry run. The other is refused,
/// at one of its checks for conflicts or when it tries to take the locks.
#[tokio::test]
async fn conflicting_submissions_get_one_dry_run_with_two_permits() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 3);
    let (validator, _submitted) = TestValidator::new(&objects, dry_run_limits(2, 0)).await;
    let gas = ids[2];
    let first = transfer(&validator.state, sender, &sender_key, ids[0], gas);
    let second = transfer(&validator.state, sender, &sender_key, ids[1], gas);

    assert_conflicting_submissions_get_one_dry_run(&validator, first, second).await;
}

/// Like [`conflicting_submissions_get_one_dry_run_with_two_permits()`], for
/// two transactions of a Move account, which takes its locks once its
/// authenticators have passed their own run.
#[tokio::test]
async fn conflicting_move_account_submissions_get_one_dry_run_with_two_permits() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_attestation();

    let mut env = AbstractAccountTestEnv::new_with_overload_config(dry_run_limits(2, 0)).await;
    let (first, second) = env.account_transactions_on_one_coin();
    let (validator, _submitted) = TestValidator::from_state(env.authority.clone());

    assert_conflicting_submissions_get_one_dry_run(&validator, first, second).await;
}

/// The check of the owned inputs' versions also fails when the validator
/// cannot read its own store. The sender does not cause that, so the rejection
/// weighs nothing although the dry run ran.
#[test]
fn failed_store_read_after_dry_run_weighs_nothing() {
    let error = IotaError::Storage("read failed".to_string());

    assert_eq!(weigh_owned_input_rejection(&error, true).value(), 0.0);
}

/// The check reports an owned input that another transaction of the sender
/// used meanwhile as a user input error, here a missing object. Without a dry
/// run, the same rejection weighs nothing.
#[test]
fn used_owned_input_weighs_one_after_dry_run() {
    let error = IotaError::UserInput {
        error: UserInputError::ObjectNotFound {
            object_id: ObjectId::new([1; 32]),
            version: None,
        },
    };

    assert_eq!(weigh_owned_input_rejection(&error, true).value(), 1.0);
    assert_eq!(weigh_owned_input_rejection(&error, false).value(), 0.0);
}

/// Submits to `submit_single_tx`, with `enable_validator_attestation` on,
/// transactions whose dependency this validator does not know. One that
/// another check rejects for good gets that error, a valid one the retriable
/// `TransactionDependencyNotFound`, and neither reaches consensus.
#[tokio::test]
async fn test_submit_single_tx_reports_an_unknown_dependency_after_the_other_checks() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        config.set_enable_transaction_dependencies_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas_id = ObjectId::random();
    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[Object::with_id_owner_for_testing(gas_id, sender)])
        .build()
        .await;

    let mut mock = MockConsensusClient::new();
    mock.expect_submit().never();

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(mock),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
        50,
    ));

    let epoch_store = authority_state.load_epoch_store_one_call_per_task();
    let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());
    let soft_locks = Arc::new(PreConsensusSoftLocks::new());

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let gas = authority_state.get_object(&gas_id).unwrap();
    let dependency = TransactionDigest::random();
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.transfer_iota(dbg_addr(2), Some(1));
    let transfer = builder.finish();

    let submit = async |gas_price: u64| {
        let tx = TestTransactionBuilder::new(sender, gas.object_ref(), gas_price)
            .programmable(transfer.clone())
            .with_transaction_dependency(dependency, epoch_store.epoch())
            .build_and_sign(&sender_key);
        let (update, _weight) = ValidatorService::submit_single_tx(
            &authority_state,
            &consensus_adapter,
            &metrics,
            &epoch_store,
            &soft_locks,
            tx,
        )
        .await;
        match update {
            TxStatusUpdate::Rejected {
                error: IotaError::UserInput { error },
            } => error,
            update => panic!("expected a user input error, got {update:?}"),
        }
    };

    let error = submit(rgp - 1).await;
    assert!(
        matches!(error, UserInputError::GasPriceUnderRGP { .. }),
        "{error:?}"
    );
    assert_eq!(
        submit(rgp).await,
        UserInputError::TransactionDependencyNotFound { digest: dependency }
    );
}

/// A dependency that executed with success here does not admit its dependent
/// by itself: post-consensus validation drops the dependent unless the
/// dependency is a root of a commit. Until this validator has it as one, the
/// dependent gets the retriable `TransactionDependencyNotFound` without a dry
/// run and does not reach consensus.
#[tokio::test]
async fn dependent_is_admitted_only_once_its_executed_dependency_is_a_commit_root() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_transaction_dependencies(true);

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 3);
    let (validator, mut submitted) =
        TestValidator::new(&objects, AuthorityOverloadConfig::default()).await;

    let dependency = transfer(&validator.state, sender, &sender_key, ids[0], ids[1]);
    let dependency_digest = *dependency.digest();
    execute_without_root_commit(&validator, dependency);

    let rgp = validator.state.reference_gas_price_for_testing().unwrap();
    let gas = validator.state.get_object(&ids[2]).unwrap();
    let mut builder = ProgrammableTransactionBuilder::new();
    builder.transfer_iota(dbg_addr(2), Some(1));
    let dependent = TestTransactionBuilder::new(sender, gas.object_ref(), rgp)
        .programmable(builder.finish())
        .with_transaction_dependency(dependency_digest, validator.epoch_store.epoch())
        .build_and_sign(&sender_key);
    let dependent_digest = *dependent.digest();

    assert_dependency_not_found(validator.submit(dependent.clone()).await, dependency_digest);
    assert_eq!(validator.state.metrics.attestation_dry_runs_total.get(), 0);
    assert!(submitted.try_recv().is_err());

    validator
        .epoch_store
        .tables()
        .unwrap()
        .root_commits
        .insert(&dependency_digest, &1)
        .unwrap();

    let (update, _) = validator.submit(dependent).await;
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}"
    );
    assert_eq!(digest_of(submitted.recv().await.unwrap()), dependent_digest);
    assert_eq!(validator.state.metrics.attestation_dry_runs_total.get(), 1);
}

/// A dependent whose dependency is not found is rejected at weight zero
/// without running its Move authenticators, here ones that would fail.
#[tokio::test]
async fn dependent_of_a_dependency_not_found_is_rejected_without_running_its_authenticators() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_transaction_dependencies(true);

    let mut env = AbstractAccountTestEnv::new().await;
    let (validator, mut submitted) = TestValidator::from_state(env.authority.clone());
    // Signed with a key the account has never held.
    let (_, unrelated_key): (_, AccountPrivateKey) = get_key_pair();

    let unknown = TransactionDigest::random();
    let dependent = env.dependent_account_transaction_signed_with(unknown, &unrelated_key);
    let weight = assert_dependency_not_found(validator.submit(dependent).await, unknown);
    assert_eq!(weight, 0.0);

    // With a dependency that has no effects here, a failed authentication is
    // reported as the dependency not found as well. One that executed without
    // being a root of a commit is not found either, and its effects make a
    // failed authentication reported as such, so only this rejection shows
    // that the authenticators did not run.
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 2);
    validator.state.insert_genesis_objects(&objects);
    let executed = transfer(&validator.state, sender, &sender_key, ids[0], ids[1]);
    let executed_digest = *executed.digest();
    execute_without_root_commit(&validator, executed);
    let dependent = env.dependent_account_transaction_signed_with(executed_digest, &unrelated_key);
    let weight = assert_dependency_not_found(validator.submit(dependent).await, executed_digest);
    assert_eq!(weight, 0.0);

    assert!(submitted.try_recv().is_err());
}

/// Submits a Move account's key rotation and, while it is in flight, a
/// dependent of it signed with the new key. The dependent's authentication
/// fails until the rotation executes here, so it is rejected with the
/// retriable `TransactionDependencyNotFound`. The rejection weighs one: the
/// validator ran the authenticators for nothing.
async fn assert_failed_authentication_weighs_one_while_dependency_is_in_flight(attestation: bool) {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_transaction_dependencies(attestation);

    let mut env = AbstractAccountTestEnv::new().await;
    let (_, new_key): (_, AccountPrivateKey) = get_key_pair();
    let rotation = env.rotate_owner_key_transaction(&new_key);
    let rotation_digest = *rotation.digest();
    let dependent = env.dependent_account_transaction_signed_with(rotation_digest, &new_key);
    let (validator, mut submitted) = TestValidator::from_state(env.authority.clone());

    let (update, _) = validator.submit(rotation).await;
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}"
    );
    assert_eq!(digest_of(submitted.recv().await.unwrap()), rotation_digest);

    let weight = assert_dependency_not_found(validator.submit(dependent).await, rotation_digest);
    assert_eq!(weight, 1.0);
    assert!(submitted.try_recv().is_err());
}

/// [`assert_failed_authentication_weighs_one_while_dependency_is_in_flight()`]
/// without validator attestation, where the validation checks run the
/// authenticators.
#[tokio::test]
async fn failed_authentication_weighs_one_while_dependency_is_in_flight() {
    assert_failed_authentication_weighs_one_while_dependency_is_in_flight(false).await;
}

/// Like [`failed_authentication_weighs_one_while_dependency_is_in_flight()`],
/// with validator attestation, where the attestation runs the authenticators.
#[tokio::test]
async fn failed_authentication_weighs_one_while_dependency_is_in_flight_with_attestation() {
    assert_failed_authentication_weighs_one_while_dependency_is_in_flight(true).await;
}

/// A dependent of a transaction this validator recorded as dropped is
/// rejected for good, unless the deny rules or the coin deny list dropped the
/// transaction: both can change in the epoch, so it can be kept when it is
/// submitted again, and the dependent gets the retriable
/// `TransactionDependencyNotFound`.
#[tokio::test]
async fn dependent_of_a_denied_dependency_gets_a_retriable_error() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_transaction_dependencies(true);

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let (ids, objects) = objects_owned_by(sender, 1);
    let (validator, mut submitted) =
        TestValidator::new(&objects, AuthorityOverloadConfig::default()).await;
    let rgp = validator.state.reference_gas_price_for_testing().unwrap();
    let gas = validator.state.get_object(&ids[0]).unwrap();
    let submit_dependent_of = async |drop: UserInputError| {
        let dependency = TransactionDigest::random();
        validator
            .epoch_store
            .insert_dropped_digests_for_testing(&[(dependency, drop.into())]);
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_iota(dbg_addr(2), Some(1));
        let dependent = TestTransactionBuilder::new(sender, gas.object_ref(), rgp)
            .programmable(builder.finish())
            .with_transaction_dependency(dependency, validator.epoch_store.epoch())
            .build_and_sign(&sender_key);

        (dependency, validator.submit(dependent).await)
    };

    let (dependency, (update, _)) =
        submit_dependent_of(UserInputError::InputObjectNotAvailableAtCommit {
            object_id: ids[0],
            version: Some(gas.version()),
        })
        .await;
    assert!(
        matches!(
            &update,
            TxStatusUpdate::Rejected {
                error: IotaError::UserInput {
                    error: UserInputError::TransactionDependencyFailed { digest },
                },
            } if *digest == dependency
        ),
        "expected the dependency to have failed, got {update:?}"
    );

    let coin_type = "0x2::iota::IOTA".to_string();
    for denial in [
        UserInputError::TransactionDenied {
            error: "denied".to_string(),
        },
        UserInputError::AddressDeniedForCoin {
            address: sender,
            coin_type: coin_type.clone(),
        },
        UserInputError::CoinTypeGlobalPause { coin_type },
    ] {
        let (dependency, result) = submit_dependent_of(denial).await;
        assert_eq!(assert_dependency_not_found(result, dependency), 0.0);
    }
    assert!(submitted.try_recv().is_err());
}
