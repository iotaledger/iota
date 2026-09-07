// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ObjectId, Transaction};
use iota_types::{
    attestation::Attestation,
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_key_pair},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    messages_grpc::TxStatusUpdate,
    object::Object,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
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
        .authority_index(&authority_state.name)
        .map(|i| i as u8)
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

/// Builds an authority with a transfer to attest, submits it through
/// `submit_single_tx`, and returns the attested payload captured on its way
/// to consensus. Protocol overrides must already be installed by the caller
/// (the returned guard must outlive the submission).
async fn submit_transfer_and_capture_payload() -> iota_types::attestation::AttestationData {
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

    let consensus_tx = rx
        .recv()
        .await
        .expect("consensus message should have been captured");
    let ConsensusTransactionKind::UserTransactionV2(attested) = consensus_tx.kind else {
        panic!("expected UserTransactionV2, got {:?}", consensus_tx.kind);
    };
    attested.attestation.payload().clone()
}

/// A minimal coefficient table that can price a plain transfer: nonzero
/// fixed overhead plus per-byte costs for inputs and writes; no native
/// functions (a transfer calls none).
fn transfer_pricing_table() -> iota_protocol_config::GasVectorCoefficientsV1 {
    iota_protocol_config::GasVectorCoefficientsV1 {
        input_object_bytes_fs: 5_000_000, // 5 ns per input byte
        written_bytes_fs: 5_000_000,      // 5 ns per written byte
        moved_bytes_per_read_op: 5_000,
        fixed_overhead_fs: 20_000_000_000, // 20 µs
        safety_multiplier_bps: 15_000,     // ×1.5
        ..Default::default()
    }
}

/// With the `attestation_gas_vector` flag on and the coefficient table plus
/// the memory-bandwidth ceiling in the config, the attestor prices its
/// dry-run's resource profile and attests `AttestationData::V2` — with a
/// nonzero cpu_time, moved bytes covering the transfer's input reads, the
/// written bytes, and a declared rate within the ceiling (the producer's
/// bandwidth floor guarantees it).
#[tokio::test]
async fn test_attestation_carries_gas_vector_when_table_present() {
    telemetry_subscribers::init_for_testing();
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_attestation_gas_vector_for_testing(true);
        config.set_gas_vector_coefficients_for_testing(transfer_pricing_table());
        config.set_memory_bandwidth_bytes_per_sec_for_testing(1_000_000_000);
        config
    });

    let payload = submit_transfer_and_capture_payload().await;
    let iota_types::attestation::AttestationData::V2 {
        cpu_time,
        moved_bytes,
        write_bytes,
        object_versions,
    } = payload
    else {
        panic!("expected AttestationData::V2, got {payload:?}");
    };
    // 20 µs fixed overhead × 1.5 safety multiplier is the prediction's floor.
    assert!(cpu_time >= 30_000, "cpu_time {cpu_time} below c0 × m");
    // The transfer reads at least its object and gas coin: two read
    // operations at 5_000 equivalent bytes each, plus their payload bytes.
    assert!(moved_bytes >= 10_000, "moved_bytes {moved_bytes} too small");
    assert!(write_bytes > 0, "a transfer writes its mutated objects");
    assert!(
        iota_types::gas_model::gas_vector::cpu_time_covers_moved_bytes(
            cpu_time,
            moved_bytes,
            1_000_000_000
        ),
        "declared rate must satisfy the bandwidth rule the validators check"
    );
    // The evidence base is unchanged from V1: a plain transfer's inputs are
    // all pinned by the transaction itself (owned object + gas coin), which
    // the attested versions deliberately exclude — so the list is empty.
    assert!(object_versions.is_empty());
}

/// With the flag on but no coefficient table in the config, the producer
/// stays on V1 — a gas vector cannot be priced without the constants.
#[tokio::test]
async fn test_attestation_stays_v1_without_coefficient_table() {
    telemetry_subscribers::init_for_testing();
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_attestation_gas_vector_for_testing(true);
        config
    });

    let payload = submit_transfer_and_capture_payload().await;
    assert!(
        matches!(payload, iota_types::attestation::AttestationData::V1 { .. }),
        "expected V1 without a coefficient table, got {payload:?}"
    );
}
