// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_crypto::{ed25519::Ed25519PrivateKey, simple::SimpleKeypair};
use iota_sdk_types::{Address, ObjectId, Transaction};
use iota_types::{
    attestation::{Attestation, AttestationData, AttestedTransaction},
    base_types::dbg_addr,
    crypto::{AccountPrivateKey, get_key_pair, get_key_pair_from_rng},
    error::IotaError,
    iota_system_state::attestor_registry::{EpochStartAttestorInfoV1, attestor_pubkey_bytes},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKind},
    messages_grpc::TxStatusUpdate,
    object::Object,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
    utils::to_sender_signed_transaction,
};
use rand::{SeedableRng, rngs::StdRng};
use tokio::sync::mpsc;

use super::ValidatorService;
use crate::{
    authority::{AuthorityState, test_authority_builder::TestAuthorityBuilder},
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

// --- External attestation ingress ---

fn enable_external_attestation() -> OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_enable_external_attestation_for_testing(true);
        config
    })
}

/// A test attestor from `seed`: its signing key and epoch-start entry.
fn test_attestor(seed: u8) -> (SimpleKeypair, EpochStartAttestorInfoV1) {
    let keypair = SimpleKeypair::from(
        get_key_pair_from_rng::<Ed25519PrivateKey, _>(&mut StdRng::from_seed([seed; 32])).1,
    );
    let entry = EpochStartAttestorInfoV1 {
        attestor_address: Address::random(),
        attestor_pubkey: attestor_pubkey_bytes(&keypair),
    };
    (keypair, entry)
}

/// An authority holding a transfer object and a gas coin for a fresh sender,
/// with `attestors` as its epoch-start attestor set.
async fn init_state_with_attestors(
    attestors: Vec<EpochStartAttestorInfoV1>,
) -> (
    Arc<AuthorityState>,
    Address,
    AccountPrivateKey,
    ObjectId,
    ObjectId,
) {
    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ])
        .with_epoch_start_attestors(attestors)
        .build()
        .await;
    (authority_state, sender, sender_key, object_id, gas_id)
}

fn consensus_adapter_with(
    authority_state: &Arc<AuthorityState>,
    mock: MockConsensusClient,
) -> Arc<ConsensusAdapter> {
    Arc::new(ConsensusAdapter::new(
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
    ))
}

/// A signed transfer of `object_id` paid with `gas_id`, explicitly attested by
/// `attestor_address` with `keypair`, claiming the protocol floor in units.
fn attested_transfer(
    authority_state: &Arc<AuthorityState>,
    sender: Address,
    sender_key: &AccountPrivateKey,
    object_id: ObjectId,
    gas_id: ObjectId,
    attestor_address: Address,
    keypair: &SimpleKeypair,
) -> AttestedTransaction {
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
    let tx = to_sender_signed_transaction(tx_data, sender_key);
    let protocol_config = authority_state
        .load_epoch_store_one_call_per_task()
        .protocol_config()
        .clone();
    let payload = AttestationData::V1 {
        computation_units: protocol_config
            .base_tx_cost_fixed()
            .min(protocol_config.gas_rounding_step()),
        object_versions: vec![],
    };
    let attestation = Attestation::new_explicit(tx.digest(), payload, attestor_address, keypair);
    AttestedTransaction::new(tx, attestation)
}

/// An explicit attestation from an attestor of this epoch's set reaches
/// consensus as a `UserTransactionV2` without a dry-run on the validator.
#[tokio::test]
async fn test_submit_single_externally_attested_tx_reaches_consensus() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_external_attestation();
    let (keypair, attestor) = test_attestor(7);
    let (authority_state, sender, sender_key, object_id, gas_id) =
        init_state_with_attestors(vec![attestor.clone()]).await;

    let (captured_tx, mut rx) = mpsc::channel::<ConsensusTransaction>(1);
    let mut mock = MockConsensusClient::new();
    mock.expect_submit().returning(move |transactions, _| {
        let _ = captured_tx.try_send(transactions[0].clone());
        Ok(with_block_status(starfish_core::BlockStatus::Sequenced(
            starfish_core::GenericTransactionRef::BlockRef(starfish_core::BlockRef::MIN),
        )))
    });
    let consensus_adapter = consensus_adapter_with(&authority_state, mock);
    let epoch_store = authority_state.load_epoch_store_one_call_per_task();

    let attested = attested_transfer(
        &authority_state,
        sender,
        &sender_key,
        object_id,
        gas_id,
        attestor.attestor_address,
        &keypair,
    );
    let (update, _weight) = ValidatorService::submit_single_externally_attested_tx(
        &authority_state,
        &consensus_adapter,
        &Arc::new(ValidatorServiceMetrics::new_for_tests()),
        &epoch_store,
        &Arc::new(PreConsensusSoftLocks::new()),
        attested,
    );
    assert!(
        matches!(update, TxStatusUpdate::Submitted),
        "expected Submitted, got {update:?}",
    );

    let consensus_tx = rx
        .recv()
        .await
        .expect("consensus message should have been captured");
    let ConsensusTransactionKind::UserTransactionV2(forwarded) = consensus_tx.kind else {
        panic!("expected UserTransactionV2, got {:?}", consensus_tx.kind);
    };
    assert!(
        matches!(
            forwarded.attestation,
            Attestation::Explicit { attestor_address, .. } if attestor_address == attestor.attestor_address
        ),
        "expected the explicit attestation to be forwarded, got {:?}",
        forwarded.attestation
    );
}

/// Explicit attestations naming an attestor outside this epoch's set, or
/// signed with a key other than the registered one, are rejected before
/// reaching consensus.
#[tokio::test]
async fn test_submit_single_externally_attested_tx_rejects_unknown_attestor_and_wrong_key() {
    telemetry_subscribers::init_for_testing();
    let _guard = enable_external_attestation();
    let (keypair, attestor) = test_attestor(7);
    let (other_keypair, _) = test_attestor(8);
    let (authority_state, sender, sender_key, object_id, gas_id) =
        init_state_with_attestors(vec![attestor.clone()]).await;

    let mut mock = MockConsensusClient::new();
    mock.expect_submit().never();
    let consensus_adapter = consensus_adapter_with(&authority_state, mock);
    let epoch_store = authority_state.load_epoch_store_one_call_per_task();
    let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());
    let soft_locks = Arc::new(PreConsensusSoftLocks::new());

    let unknown_attestor = attested_transfer(
        &authority_state,
        sender,
        &sender_key,
        object_id,
        gas_id,
        Address::random(),
        &keypair,
    );
    let (update, _) = ValidatorService::submit_single_externally_attested_tx(
        &authority_state,
        &consensus_adapter,
        &metrics,
        &epoch_store,
        &soft_locks,
        unknown_attestor,
    );
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::ExplicitAttestationUnknownAttestor { .. }
            }
        ),
        "expected ExplicitAttestationUnknownAttestor, got {update:?}",
    );

    let wrong_key = attested_transfer(
        &authority_state,
        sender,
        &sender_key,
        object_id,
        gas_id,
        attestor.attestor_address,
        &other_keypair,
    );
    let (update, _) = ValidatorService::submit_single_externally_attested_tx(
        &authority_state,
        &consensus_adapter,
        &metrics,
        &epoch_store,
        &soft_locks,
        wrong_key,
    );
    assert!(
        matches!(
            update,
            TxStatusUpdate::Rejected {
                error: IotaError::ExplicitAttestationKeyMismatch { .. }
            }
        ),
        "expected ExplicitAttestationKeyMismatch, got {update:?}",
    );
}

/// With external attestation disabled the endpoint refuses attested
/// submissions before looking at any transaction.
#[tokio::test]
async fn test_submit_externally_attested_tx_impl_rejects_when_disabled() {
    telemetry_subscribers::init_for_testing();
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config
    });
    let (keypair, attestor) = test_attestor(7);
    let (authority_state, sender, sender_key, object_id, gas_id) =
        init_state_with_attestors(vec![attestor.clone()]).await;

    let mut mock = MockConsensusClient::new();
    mock.expect_submit().never();
    let service = ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter_with(&authority_state, mock),
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    );

    let attested = attested_transfer(
        &authority_state,
        sender,
        &sender_key,
        object_id,
        gas_id,
        attestor.attestor_address,
        &keypair,
    );
    assert!(
        service
            .submit_externally_attested_tx_impl(vec![attested])
            .await
            .is_err(),
        "expected the endpoint to refuse attested submissions",
    );
}
