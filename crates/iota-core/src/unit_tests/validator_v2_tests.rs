// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::ObjectId;
use iota_types::{
    attestation::Attestation,
    base_types::dbg_addr,
    crypto::{AccountKeyPair, get_key_pair},
    iota_system_state::epoch_start_iota_system_state::EpochStartSystemStateTrait,
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    messages_grpc::TxStatusUpdate,
    object::Object,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionData, TransactionDataAPI},
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
/// consensus adapter is a `UserTransactionV2` carrying an
/// `Attestation::Validator` whose `attestor_index` matches this validator's own
/// position in the consensus committee.
#[tokio::test]
async fn test_submit_single_tx_produces_user_transaction_v2_with_validator_attestation() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
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
            starfish_core::GenericTransactionRef::BlockRef(starfish_core::BlockRef::MIN),
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

    let tx_data = TransactionData::new_transfer(
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

    // Assert the consensus message is UserTransactionV2 with Validator attestation.
    let consensus_tx = rx
        .recv()
        .await
        .expect("consensus message should have been captured");
    let ConsensusTransactionKind::UserTransactionV2(attested) = consensus_tx.kind else {
        panic!("expected UserTransactionV2, got {:?}", consensus_tx.kind);
    };
    let Attestation::Validator { attestor_index, .. } = &attested.attestation else {
        panic!(
            "expected Attestation::Validator, got {:?}",
            attested.attestation
        );
    };

    // The attestor_index must match this validator's position in the consensus
    // committee — mirrors the lookup performed in submit_single_tx.
    let expected_index = epoch_store
        .committee()
        .names()
        .position(|n| n == &authority_state.name)
        .and_then(|i| {
            epoch_store
                .epoch_start_state()
                .get_consensus_committee()
                .to_authority_index(i)
        })
        .expect("authority must be present in the consensus committee");
    assert_eq!(*attestor_index, expected_index);
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
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
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

    let tx_data = TransactionData::new_transfer(
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
