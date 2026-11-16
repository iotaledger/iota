// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectId, SharedObjectReference, Transaction};
use iota_types::{
    attestation::Attestation,
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_key_pair},
    error::{IotaError, UserInputError},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    messages_grpc::TxStatusUpdate,
    object::{OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    traffic_control::Weight,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
    utils::to_sender_signed_transaction,
};
use tokio::sync::mpsc;

use super::ValidatorService;
use crate::{
    authority::test_authority_builder::TestAuthorityBuilder,
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
    assert_eq!(soft_locks.lock_count(), 0, "no soft lock may be taken");
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
