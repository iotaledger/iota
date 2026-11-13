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

/// A second claim of an address while the first is in flight on this
/// validator conflicts on the address's claim lock, a reference that is no
/// object, so it is rejected as a pending claim of the address rather than as
/// a lock conflict on an object.
#[tokio::test]
async fn test_submit_single_tx_rejects_a_second_claim_in_flight() {
    telemetry_subscribers::init_for_testing();

    let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
    let gas_ids = [ObjectId::random(), ObjectId::random()];
    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(gas_ids[0], sender),
            Object::with_id_owner_for_testing(gas_ids[1], sender),
        ])
        .build()
        .await;

    let mut mock = MockConsensusClient::new();
    mock.expect_submit().times(1).returning(|_, _| {
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
    assert!(
        epoch_store
            .protocol_config()
            .enable_claim_account_transaction()
    );
    let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());
    let soft_locks = Arc::new(PreConsensusSoftLocks::new());
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let claims: Vec<_> = gas_ids
        .iter()
        .map(|gas_id| {
            let claim = iota_sdk_types::SmartAccountClaim {
                public_key_scheme: iota_sdk_types::SignatureScheme::Ed25519.to_u8(),
                public_key_raw_bytes: AsRef::<[u8]>::as_ref(&sender_key.public_key()).to_vec(),
            };
            let tx_data = Transaction::new(
                iota_sdk_types::TransactionKind::new_claim_account(
                    iota_sdk_types::ClaimAccountTransaction::new_smart_account(claim),
                ),
                sender,
                authority_state.get_object(gas_id).unwrap().object_ref(),
                iota_types::transaction::TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE * rgp,
                rgp,
            );
            to_sender_signed_transaction(tx_data, &sender_key)
        })
        .collect();
    let first_digest = *claims[0].digest();

    let mut updates = Vec::new();
    for claim in claims {
        updates.push(
            ValidatorService::submit_single_tx(
                &authority_state,
                &consensus_adapter,
                &metrics,
                &epoch_store,
                &soft_locks,
                claim,
            )
            .await
            .0,
        );
    }

    assert!(
        matches!(updates[0], TxStatusUpdate::Submitted),
        "expected the first claim to be submitted, got {:?}",
        updates[0]
    );
    let TxStatusUpdate::Rejected {
        error:
            IotaError::UserInput {
                error:
                    UserInputError::AccountClaimPending {
                        address,
                        pending_transaction,
                    },
            },
    } = &updates[1]
    else {
        panic!(
            "expected the second claim to be rejected as pending, got {:?}",
            updates[1]
        );
    };
    assert_eq!(*address, sender);
    assert_eq!(*pending_transaction, first_digest);
}
