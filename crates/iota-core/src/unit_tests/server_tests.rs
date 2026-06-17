// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::traits::KeyPair;
use iota_network::api::{
    GetCheckpointRequest, GetTxStatusRequest, NotifyCapabilitiesRequest, SubmitTxRequest,
    TxStatusQuery, ValidatorPeer, ValidatorV2,
};
use iota_protocol_config::{Chain, ProtocolConfig};
use iota_sdk_types::ProgrammableTransaction;
// Additional imports for P-COOL tests
use iota_sdk_types::{
    Argument, Command, Identifier, ObjectId, SplitCoins,
    crypto::{Intent, IntentMessage, IntentScope::AuthorityCapabilities},
};
use iota_types::digests::TransactionDigest;
// Additional imports for P-COOL tests
use iota_types::{
    base_types::{AuthorityName, IotaAddress, dbg_addr, dbg_object_id, random_object_ref},
    crypto::{
        AccountKeyPair, AuthorityKeyPair, AuthoritySignature, IotaAuthoritySignature,
        get_authority_key_pair, get_key_pair,
    },
    error::IotaError,
    messages_checkpoint::CheckpointResponse,
    messages_consensus::{AuthorityCapabilitiesV1, SignedAuthorityCapabilitiesV1},
    messages_grpc::{LayoutGenerationOption, TxStatusUpdate},
    object::Object,
    supported_protocol_versions::SupportedProtocolVersions,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionData},
    utils::to_sender_signed_transaction,
};
use tokio_stream::StreamExt;

use super::*;
use crate::{
    authority::{
        AuthorityState,
        authority_test_utils::init_certified_transaction,
        authority_tests::{init_state_with_ids_and_object_basics, init_state_with_object_id},
        test_authority_builder::TestAuthorityBuilder,
    },
    authority_client::{NetworkAuthorityClient, validator::ValidatorAPI},
    authority_server::{
        AuthorityServer, ValidatorService, ValidatorServiceMetrics, make_tonic_request_for_testing,
    },
    checkpoints::CheckpointStore,
    consensus_adapter::{
        ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics,
        MockConsensusClient,
    },
};

// This is the most basic example of how to test the server logic
#[tokio::test]
async fn test_simple_request() {
    let sender = dbg_addr(1);
    let object_id = dbg_object_id(1);
    let authority_state = init_state_with_object_id(sender, object_id).await;

    // The following two fields are only needed for shared objects (not by this
    // bench).
    let server = AuthorityServer::new_for_test(authority_state.clone());

    let server_handle = server.spawn_for_test().await.unwrap();

    let client = NetworkAuthorityClient::connect(
        server_handle.address(),
        authority_state
            .config
            .network_key_pair()
            .public()
            .to_owned(),
    )
    .await
    .unwrap();

    let req =
        ObjectInfoRequest::latest_object_info_request(object_id, LayoutGenerationOption::Generate);

    client.handle_object_info_request(req).await.unwrap();
}

// TODO: Happy path tests for handling AuthorityCapabilities are not covered
//  here as the setup is more  complex and will be handled in end-to-end tests.

// This test verifies that the authority rejects capability notifications from
// unauthorized authorities (authorities that are not part of non-committee
// validators).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_authority_reject_authority_capabilities() {
    telemetry_subscribers::init_for_testing();

    // Create one sender, one recipient addresses, and 2 gas objects.
    let (_sender, sender_key): (_, AuthorityKeyPair) = get_authority_key_pair();

    let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    protocol_config.set_select_committee_from_eligible_validators_for_testing(true);
    protocol_config.set_track_non_committee_eligible_validators_for_testing(true);
    protocol_config.set_select_committee_supporting_next_epoch_version(true);

    let authority_state = TestAuthorityBuilder::new()
        .with_protocol_config(protocol_config)
        .build()
        .await;

    // Create a validator service around the `authority_state`.
    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    // Create the validator service that will handle capability notifications
    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    // Create an authority capabilities message containing the authority's identity
    // and supported features
    let capabilities = AuthorityCapabilitiesV1::new(
        AuthorityName::new(sender_key.public().pubkey.to_bytes()), // Authority identifier
        Chain::Mainnet,                                            // Target blockchain network
        SupportedProtocolVersions::new_for_testing(1, 10),         // Protocol version range
        vec![],                                                    /* Empty capabilities list
                                                                    * for this test */
    );

    // Sign the capability message with the authority's private key
    // This creates a cryptographic proof that the message came from the claimed
    // authority
    let signature = AuthoritySignature::new_secure(
        &IntentMessage::new(Intent::iota_app(AuthorityCapabilities), &capabilities),
        &authority_state.current_epoch_for_testing(),
        &sender_key,
    );

    // Package the signed capabilities into a request message
    let request1 = HandleCapabilityNotificationRequestV1 {
        message: SignedAuthorityCapabilitiesV1::new_from_data_and_sig(capabilities, signature),
    };

    // Attempt to handle the capability notification and verify it gets rejected
    // The request should be rejected because the signer is not a non-committee
    // validator authorized to send capability notifications
    assert!(
        validator_service
            .handle_capability_notification_v1(make_tonic_request_for_testing(request1))
            .await
            .is_err(),
        "Expected capability notification from unauthorized authority to be rejected"
    );

    // Test with authority_state's own keys - this should also be rejected
    // because the authority should not accept capability notifications from itself
    let authority_capabilities = AuthorityCapabilitiesV1::new(
        authority_state.name, // Use the authority's own name
        Chain::Mainnet,
        SupportedProtocolVersions::new_for_testing(1, 10),
        vec![],
    );

    // Sign with the authority_state's own key pair
    let authority_signature = AuthoritySignature::new_secure(
        &IntentMessage::new(
            Intent::iota_app(AuthorityCapabilities),
            &authority_capabilities,
        ),
        &authority_state.current_epoch_for_testing(),
        &*authority_state.secret,
    );

    let request2 = HandleCapabilityNotificationRequestV1 {
        message: SignedAuthorityCapabilitiesV1::new_from_data_and_sig(
            authority_capabilities,
            authority_signature,
        ),
    };

    // This should also be rejected - committee validators should not accept
    // capability notifications from themselves or other committee members
    assert!(
        validator_service
            .handle_capability_notification_v1(make_tonic_request_for_testing(request2))
            .await
            .is_err(),
        "Expected capability notification from authority itself to be rejected"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_handle_capability_notification_v1_feature_disabled() {
    telemetry_subscribers::init_for_testing();

    let (_sender, sender_key): (_, AuthorityKeyPair) = get_authority_key_pair();

    let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    protocol_config.set_select_committee_from_eligible_validators_for_testing(false);
    protocol_config.set_track_non_committee_eligible_validators_for_testing(false);
    protocol_config.set_select_committee_supporting_next_epoch_version(false);

    let authority_state = TestAuthorityBuilder::new()
        .with_protocol_config(protocol_config)
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let capabilities = AuthorityCapabilitiesV1::new(
        AuthorityName::new(sender_key.public().pubkey.to_bytes()),
        Chain::Mainnet,
        SupportedProtocolVersions::new_for_testing(1, 10),
        vec![],
    );

    let signature = AuthoritySignature::new_secure(
        &IntentMessage::new(Intent::iota_app(AuthorityCapabilities), &capabilities),
        &authority_state.current_epoch_for_testing(),
        &sender_key,
    );

    let request = HandleCapabilityNotificationRequestV1 {
        message: SignedAuthorityCapabilitiesV1::new_from_data_and_sig(capabilities, signature),
    };

    let result = validator_service
        .handle_capability_notification_v1(make_tonic_request_for_testing(request))
        .await;

    assert!(
        result.is_err(),
        "Expected capability notification to be rejected due to feature being disabled"
    );
    let err_kind = IotaError::from(result.unwrap_err());
    assert!(
        matches!(err_kind, IotaError::UnsupportedFeature { .. }),
        "Expected UnsupportedFeature error, but got {err_kind:?}",
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_get_checkpoint_happy_path() {
    telemetry_subscribers::init_for_testing();

    let authority_state = TestAuthorityBuilder::new()
        .insert_genesis_checkpoint()
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state,
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    // Request the genesis checkpoint (sequence 0, certified).
    let proto_request = GetCheckpointRequest {
        sequence_number: Some(0),
        request_content: true,
        certified: true,
    };

    let response = validator_service
        .get_checkpoint(make_tonic_request_for_testing(proto_request))
        .await
        .expect("get_checkpoint should succeed for genesis checkpoint");

    let proto_resp = response.into_inner();

    // The genesis checkpoint must be present.
    assert!(
        proto_resp.checkpoint.is_some(),
        "Expected checkpoint data in response"
    );

    // Verify the proto response can be converted back to the domain type.
    let native: CheckpointResponse = proto_resp
        .try_into()
        .expect("proto → native conversion should succeed");
    assert!(native.checkpoint.is_some());
    assert!(native.contents.is_some());
}

async fn build_shared_object_transaction(
    state: &AuthorityState,
    sender: IotaAddress,
    sender_key: &AccountKeyPair,
    gas_object_id: ObjectId,
    pkg_ref: iota_types::base_types::ObjectRef,
) -> Transaction {
    let rgp = state.reference_gas_price_for_testing().unwrap();
    let gas = state.get_object(&gas_object_id).await.unwrap();
    let tx_data = TransactionData::new_move_call(
        sender,
        pkg_ref.object_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("use_clock"),
        vec![],
        gas.object_ref(),
        vec![CallArg::CLOCK_IMMUTABLE],
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
        rgp,
    )
    .unwrap();
    to_sender_signed_transaction(tx_data, sender_key)
}

// ── ValidatorV2 submit_tx (streaming) tests ──────────────────────────────────

/// Helper: convert a `Vec<Transaction>` into the proto `SubmitTxRequest` and
/// wrap it in a tonic request.
fn make_v2_submit_request(transactions: Vec<Transaction>) -> tonic::Request<SubmitTxRequest> {
    let proto: SubmitTxRequest = transactions.try_into().expect("BCS serialization failed");
    make_tonic_request_for_testing(proto)
}

/// Result from collecting a V2 stream item: either a successfully decoded
/// status or a raw `tonic::Status` error.
enum V2StreamItem {
    Ok(iota_types::digests::TransactionDigest, TxStatusUpdate),
    Err(tonic::Status),
}

/// Collect all items from a V2 streaming response.
async fn collect_v2_stream_raw(
    response: tonic::Response<crate::authority_server::StreamResponse<iota_network::api::TxStatus>>,
) -> Vec<V2StreamItem> {
    use iota_network::api::status_detail::Kind;
    let mut stream = response.into_inner();
    let mut results = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Err(status) => results.push(V2StreamItem::Err(status)),
            Ok(status) => {
                let digest: iota_types::digests::TransactionDigest = status
                    .tx_digest
                    .expect("tx_digest present")
                    .try_into()
                    .expect("digest conversion");
                let detail = status.status.expect("status present");
                let native_result = match detail.kind.expect("kind present") {
                    Kind::Submitted(_) => TxStatusUpdate::Submitted,
                    Kind::Executed(exec) => {
                        let effects_digest = bcs::from_bytes(&exec.effects_digest)
                            .expect("effects_digest deserialization");
                        let details = exec
                            .details
                            .map(|d| bcs::from_bytes(&d).expect("details deserialization"))
                            .map(Box::new);
                        TxStatusUpdate::Executed {
                            effects_digest,
                            details,
                        }
                    }
                    Kind::Rejected(rej) => {
                        let error: IotaError =
                            bcs::from_bytes(&rej.error).expect("error deserialization");
                        TxStatusUpdate::Rejected { error }
                    }
                    Kind::Expired(exp) => TxStatusUpdate::Expired { epoch: exp.epoch },
                };
                results.push(V2StreamItem::Ok(digest, native_result));
            }
        }
    }
    results
}

/// Convenience wrapper: collect all items and panic on stream-level errors.
async fn collect_v2_stream(
    response: tonic::Response<crate::authority_server::StreamResponse<iota_network::api::TxStatus>>,
) -> Vec<(iota_types::digests::TransactionDigest, TxStatusUpdate)> {
    collect_v2_stream_raw(response)
        .await
        .into_iter()
        .map(|item| match item {
            V2StreamItem::Ok(digest, result) => (digest, result),
            V2StreamItem::Err(status) => panic!("unexpected stream error: {status}"),
        })
        .collect()
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_success() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
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

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();
    let recipient = dbg_addr(2);

    let tx_data = TransactionData::new_transfer(
        recipient,
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);
    let expected_digest = *tx.digest();

    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx]))
        .await
        .expect("submit_tx should succeed");

    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 1);
    let (digest, result) = &results[0];
    assert_eq!(*digest, expected_digest);
    assert!(
        matches!(result, TxStatusUpdate::Submitted),
        "Expected Submitted, got {result:?}"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_invalid_signature() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, _sender_key): (_, AccountKeyPair) = get_key_pair();
    let (_wrong_sender, wrong_key): (_, AccountKeyPair) = get_key_pair();
    let object_id = ObjectId::random();
    let gas_id = ObjectId::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ])
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();
    let recipient = dbg_addr(2);

    let tx_data = TransactionData::new_transfer(
        recipient,
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &wrong_key);

    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx]))
        .await
        .expect("submit_tx stream should open successfully");

    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 1);
    match &results[0].1 {
        TxStatusUpdate::Rejected { error } => {
            let msg = format!("{error:?}").to_lowercase();
            assert!(
                msg.contains("signature"),
                "Error should mention signature, got: {msg}",
            );
        }
        other => panic!("Expected Rejected, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_feature_flag_disabled() {
    telemetry_subscribers::init_for_testing();

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

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();
    let recipient = dbg_addr(2);

    let tx_data = TransactionData::new_transfer(
        recipient,
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let result = validator_service
        .submit_tx(make_v2_submit_request(vec![tx]))
        .await;

    match result {
        Err(err) => assert!(
            err.message()
                .contains("P-COOL flow is not enabled in this protocol version"),
        ),
        Ok(_) => panic!("Expected error when P-COOL is disabled"),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_already_executed() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
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

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();

    let tx_data = TransactionData::new_transfer(
        dbg_addr(2),
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    // Execute the transaction first.
    let cert = init_certified_transaction(tx.clone(), &authority_state);
    let (effects, _) = authority_state.execute_for_test(&cert);

    // Re-submit via V2 streaming endpoint.
    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx]))
        .await
        .expect("submit_tx should succeed");

    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 1);
    match &results[0].1 {
        TxStatusUpdate::Executed { effects_digest, .. } => {
            assert_eq!(effects_digest, effects.digest());
        }
        other => panic!("Expected Executed, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_multiple_transactions() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id1 = ObjectId::random();
    let gas_id2 = ObjectId::random();

    let (authority_state, pkg_ref) =
        init_state_with_ids_and_object_basics(vec![(sender, gas_id1), (sender, gas_id2)]).await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let tx1 =
        build_shared_object_transaction(&authority_state, sender, &sender_key, gas_id1, pkg_ref)
            .await;
    let tx2 =
        build_shared_object_transaction(&authority_state, sender, &sender_key, gas_id2, pkg_ref)
            .await;

    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx1, tx2]))
        .await
        .expect("submit_tx should succeed");

    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 2, "Expected one result per transaction");
    for (i, (_digest, result)) in results.iter().enumerate() {
        assert!(
            matches!(result, TxStatusUpdate::Submitted),
            "Expected Submitted for tx {i}, got {result:?}"
        );
    }
}

/// V2 mirror of `test_submit_transaction_invalid_transaction`: a PTB with an
/// empty `SplitCoins` args list is structurally invalid and fails
/// `validity_check`, producing a `Rejected` stream item.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_invalid_transaction() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id = ObjectId::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[Object::with_id_owner_for_testing(gas_id, sender)])
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();

    let pt = ProgrammableTransaction {
        inputs: vec![],
        commands: vec![Command::SplitCoins(SplitCoins {
            coin: Argument::Gas,
            amounts: vec![], // empty — invalid
        })],
    };
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx]))
        .await
        .expect("stream should open successfully");

    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 1);
    assert!(
        matches!(&results[0].1, TxStatusUpdate::Rejected { .. }),
        "Expected Rejected for invalid transaction, got {:?}",
        results[0].1
    );
}

/// V2 mirror of `test_submit_transaction_gas_object_validation`: a transaction
/// referencing a non-existent gas object fails during validation checks.
/// In V2, per-transaction errors from `handle_transaction_validation_checks`
/// are surfaced as `TxStatusUpdate::Rejected` stream items.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_gas_object_validation() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let object_id = ObjectId::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[Object::with_id_owner_for_testing(object_id, sender)])
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();

    let tx_data = TransactionData::new_transfer(
        dbg_addr(2),
        object.object_ref(),
        sender,
        random_object_ref(), // non-existent gas object
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx]))
        .await
        .expect("stream should open successfully");

    // Per-transaction validation errors are surfaced as `Rejected` stream items.
    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 1);
    assert!(
        matches!(&results[0].1, TxStatusUpdate::Rejected { .. }),
        "Expected Rejected for non-existent gas object, got {:?}",
        results[0].1
    );
}

/// V2 mirror of `test_submit_transactions_different_gas_prices_accepted`:
/// transactions with different gas prices are processed independently in
/// P-COOL mode.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_different_gas_prices_accepted() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id1 = ObjectId::random();
    let gas_id2 = ObjectId::random();

    let (authority_state, pkg_ref) =
        init_state_with_ids_and_object_basics(vec![(sender, gas_id1), (sender, gas_id2)]).await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let gas1 = authority_state.get_object(&gas_id1).await.unwrap();
    let gas2 = authority_state.get_object(&gas_id2).await.unwrap();

    let tx_data1 = TransactionData::new_move_call(
        sender,
        pkg_ref.object_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("use_clock"),
        vec![],
        gas1.object_ref(),
        vec![CallArg::CLOCK_IMMUTABLE],
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
        rgp, // base price
    )
    .unwrap();
    let tx1 = to_sender_signed_transaction(tx_data1, &sender_key);

    let tx_data2 = TransactionData::new_move_call(
        sender,
        pkg_ref.object_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("use_clock"),
        vec![],
        gas2.object_ref(),
        vec![CallArg::CLOCK_IMMUTABLE],
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp * 2,
        rgp * 2, // different price
    )
    .unwrap();
    let tx2 = to_sender_signed_transaction(tx_data2, &sender_key);

    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx1, tx2]))
        .await
        .expect("submit_tx should succeed");

    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 2, "Expected one result per transaction");
    for (i, (_digest, result)) in results.iter().enumerate() {
        assert!(
            matches!(result, TxStatusUpdate::Submitted),
            "Expected Submitted for tx {i}, got {result:?}"
        );
    }
}

/// V2 mirror of `test_submit_oversized_transaction`: a transaction exceeding
/// `max_tx_size_bytes` (128 KiB) is rejected by `validity_check`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_submit_tx_oversized_transaction() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id = ObjectId::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[Object::with_id_owner_for_testing(gas_id, sender)])
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();

    // Build a PTB whose inputs alone total ~140 KiB > max_tx_size_bytes (128 KiB).
    let inputs: Vec<_> = (0u8..10)
        .map(|i| CallArg::Pure(vec![i; 14 * 1024]))
        .collect();
    let pt = ProgrammableTransaction {
        inputs,
        commands: vec![],
    };
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas.object_ref()],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let response = validator_service
        .submit_tx(make_v2_submit_request(vec![tx]))
        .await
        .expect("stream should open successfully");

    let results = collect_v2_stream(response).await;
    assert_eq!(results.len(), 1);
    assert!(
        matches!(&results[0].1, TxStatusUpdate::Rejected { .. }),
        "Expected Rejected for oversized transaction, got {:?}",
        results[0].1
    );
}

// ── ValidatorV2 get_tx_status (streaming) tests ──────────────────────────────

/// Helper: build a proto `GetTxStatusRequest` from digest/include_details
/// pairs.
fn make_v2_get_tx_status_request(
    queries: Vec<(TransactionDigest, bool)>,
) -> tonic::Request<GetTxStatusRequest> {
    let proto = GetTxStatusRequest {
        queries: queries
            .into_iter()
            .map(|(digest, include_details)| {
                let tx_digest: iota_network::api::TxDigest = digest.try_into().unwrap();
                TxStatusQuery {
                    tx_digest: Some(tx_digest),
                    include_details,
                }
            })
            .collect(),
    };
    make_tonic_request_for_testing(proto)
}

/// Collect all items from a get_tx_status streaming response into
/// `(TransactionDigest, TxStatusUpdate)` pairs.
async fn collect_v2_status_stream(
    response: tonic::Response<crate::authority_server::StreamResponse<iota_network::api::TxStatus>>,
) -> Vec<(iota_types::digests::TransactionDigest, TxStatusUpdate)> {
    use iota_network::api::status_detail::Kind;
    let mut stream = response.into_inner();
    let mut results = Vec::new();
    while let Some(item) = stream.next().await {
        let status = item.expect("stream item should be Ok");
        let digest: iota_types::digests::TransactionDigest = status
            .tx_digest
            .expect("tx_digest present")
            .try_into()
            .expect("digest conversion");
        let detail = status.status.expect("status present");
        let update = match detail.kind.expect("kind present") {
            Kind::Submitted(_) => TxStatusUpdate::Submitted,
            Kind::Executed(exec) => {
                let effects_digest =
                    bcs::from_bytes(&exec.effects_digest).expect("effects_digest deserialization");
                let details = exec
                    .details
                    .map(|d| Box::new(bcs::from_bytes(&d).expect("details deserialization")));
                TxStatusUpdate::Executed {
                    effects_digest,
                    details,
                }
            }
            Kind::Rejected(rej) => {
                let error = bcs::from_bytes(&rej.error).expect("error deserialization");
                TxStatusUpdate::Rejected { error }
            }
            Kind::Expired(exp) => TxStatusUpdate::Expired { epoch: exp.epoch },
        };
        results.push((digest, update));
    }
    results
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_get_tx_status_already_executed() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
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

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();

    let tx_data = TransactionData::new_transfer(
        dbg_addr(2),
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);
    let tx_digest = *tx.digest();

    // Execute the transaction first.
    let cert = init_certified_transaction(tx, &authority_state);
    let (effects, _) = authority_state.execute_for_test(&cert);

    // Query status — should return Executed immediately.
    let response = validator_service
        .get_tx_status(make_v2_get_tx_status_request(vec![(tx_digest, false)]))
        .await
        .expect("get_tx_status should succeed");

    let results = collect_v2_status_stream(response).await;
    assert_eq!(results.len(), 1);
    let (digest, update) = &results[0];
    assert_eq!(*digest, tx_digest);
    match update {
        TxStatusUpdate::Executed {
            effects_digest,
            details,
        } => {
            assert_eq!(effects_digest, effects.digest());
            assert!(
                details.is_none(),
                "details should be None when not requested"
            );
        }
        other => panic!("Expected Executed, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_get_tx_status_already_executed_with_details() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
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

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();

    let tx_data = TransactionData::new_transfer(
        dbg_addr(2),
        object.object_ref(),
        sender,
        gas.object_ref(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);
    let tx_digest = *tx.digest();

    // Execute first.
    let cert = init_certified_transaction(tx, &authority_state);
    authority_state.execute_for_test(&cert);

    // Query with include_details = true.
    let response = validator_service
        .get_tx_status(make_v2_get_tx_status_request(vec![(tx_digest, true)]))
        .await
        .expect("get_tx_status should succeed");

    let results = collect_v2_status_stream(response).await;
    assert_eq!(results.len(), 1);
    match &results[0].1 {
        TxStatusUpdate::Executed { details, .. } => {
            assert!(
                details.is_some(),
                "details should be present when requested"
            );
        }
        other => panic!("Expected Executed, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_get_tx_status_multiple_queries() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let object_id1 = ObjectId::random();
    let gas_id1 = ObjectId::random();
    let object_id2 = ObjectId::random();
    let gas_id2 = ObjectId::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id1, sender),
            Object::with_id_owner_for_testing(gas_id1, sender),
            Object::with_id_owner_for_testing(object_id2, sender),
            Object::with_id_owner_for_testing(gas_id2, sender),
        ])
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let obj1 = authority_state.get_object(&object_id1).await.unwrap();
    let gas1 = authority_state.get_object(&gas_id1).await.unwrap();
    let obj2 = authority_state.get_object(&object_id2).await.unwrap();
    let gas2 = authority_state.get_object(&gas_id2).await.unwrap();

    // Build and execute two transactions.
    let tx1 = to_sender_signed_transaction(
        TransactionData::new_transfer(
            dbg_addr(2),
            obj1.object_ref(),
            sender,
            gas1.object_ref(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        ),
        &sender_key,
    );
    let tx2 = to_sender_signed_transaction(
        TransactionData::new_transfer(
            dbg_addr(3),
            obj2.object_ref(),
            sender,
            gas2.object_ref(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        ),
        &sender_key,
    );
    let digest1 = *tx1.digest();
    let digest2 = *tx2.digest();

    let cert1 = init_certified_transaction(tx1, &authority_state);
    let cert2 = init_certified_transaction(tx2, &authority_state);
    authority_state.execute_for_test(&cert1);
    authority_state.execute_for_test(&cert2);

    // Query both with different include_details settings.
    let response = validator_service
        .get_tx_status(make_v2_get_tx_status_request(vec![
            (digest1, true),
            (digest2, false),
        ]))
        .await
        .expect("get_tx_status should succeed");

    let results = collect_v2_status_stream(response).await;
    assert_eq!(results.len(), 2);

    // Results may arrive in any order due to concurrent spawns.
    for (digest, update) in &results {
        match update {
            TxStatusUpdate::Executed { details, .. } => {
                if *digest == digest1 {
                    assert!(details.is_some(), "digest1 requested details");
                } else {
                    assert!(details.is_none(), "digest2 did not request details");
                }
            }
            other => panic!("Expected Executed, got {other:?}"),
        }
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_get_tx_status_too_many_queries() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let authority_state = TestAuthorityBuilder::new().build().await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state,
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    // Build 33 queries (exceeds MAX_QUERIES_PER_GET_TX_STATUS = 32).
    let queries: Vec<_> = (0..33)
        .map(|_| (iota_types::digests::TransactionDigest::random(), false))
        .collect();

    let result = validator_service
        .get_tx_status(make_v2_get_tx_status_request(queries))
        .await;

    match result {
        Err(status) => assert_eq!(status.code(), tonic::Code::InvalidArgument),
        Ok(_) => panic!("should reject oversized request"),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_get_tx_status_empty_queries_ping() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let authority_state = TestAuthorityBuilder::new().build().await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state,
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let response = validator_service
        .get_tx_status(make_v2_get_tx_status_request(vec![]))
        .await
        .expect("empty request should succeed (ping)");

    let results = collect_v2_status_stream(response).await;
    assert!(results.is_empty(), "ping should return empty stream");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_get_tx_status_dropped_digest_rejected() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let authority_state = TestAuthorityBuilder::new().build().await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let dropped_digest = iota_types::digests::TransactionDigest::random();
    let dropped_error = IotaError::TransactionExpired;

    // Simulate white-flag dropping the transaction.
    let epoch_store = authority_state.load_epoch_store_one_call_per_task();
    epoch_store.insert_dropped_digests_for_testing(&[(dropped_digest, dropped_error.clone())]);

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state,
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let response = validator_service
        .get_tx_status(make_v2_get_tx_status_request(vec![(dropped_digest, false)]))
        .await
        .expect("get_tx_status should succeed");

    let results = collect_v2_status_stream(response).await;
    assert_eq!(results.len(), 1);
    let (digest, update) = &results[0];
    assert_eq!(*digest, dropped_digest);
    assert!(
        matches!(update, TxStatusUpdate::Rejected { .. }),
        "Expected Rejected for dropped digest, got {update:?}"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_get_tx_status_unknown_digest_expires() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config
    });

    let authority_state = TestAuthorityBuilder::new().build().await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state,
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let unknown_digest = iota_types::digests::TransactionDigest::random();

    let response = validator_service
        .get_tx_status(make_v2_get_tx_status_request(vec![(unknown_digest, false)]))
        .await
        .expect("get_tx_status should succeed");

    // With paused time, the 30s timeout fires immediately.
    let results = collect_v2_status_stream(response).await;
    assert_eq!(results.len(), 1);
    let (digest, update) = &results[0];
    assert_eq!(*digest, unknown_digest);
    assert!(
        matches!(update, TxStatusUpdate::Expired { .. }),
        "Expected Expired for unknown digest, got {update:?}"
    );
}

// ── ValidatorV2 notify_capabilities tests
// ─────────────────────────────────────

/// Helper: build a proto `NotifyCapabilitiesRequest` from a domain request.
fn make_v2_notify_capabilities_request(
    domain: HandleCapabilityNotificationRequestV1,
) -> tonic::Request<NotifyCapabilitiesRequest> {
    let proto: NotifyCapabilitiesRequest = domain.try_into().unwrap();
    make_tonic_request_for_testing(proto)
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_notify_capabilities_reject_unauthorized() {
    telemetry_subscribers::init_for_testing();

    let (_sender, sender_key): (_, AuthorityKeyPair) = get_authority_key_pair();

    let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    protocol_config.set_select_committee_from_eligible_validators_for_testing(true);
    protocol_config.set_track_non_committee_eligible_validators_for_testing(true);
    protocol_config.set_select_committee_supporting_next_epoch_version(true);

    let authority_state = TestAuthorityBuilder::new()
        .with_protocol_config(protocol_config)
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    // Request from an unknown authority — should be rejected.
    let capabilities = AuthorityCapabilitiesV1::new(
        AuthorityName::new(sender_key.public().pubkey.to_bytes()),
        Chain::Mainnet,
        SupportedProtocolVersions::new_for_testing(1, 10),
        vec![],
    );
    let signature = AuthoritySignature::new_secure(
        &IntentMessage::new(Intent::iota_app(AuthorityCapabilities), &capabilities),
        &authority_state.current_epoch_for_testing(),
        &sender_key,
    );
    let request1 = HandleCapabilityNotificationRequestV1 {
        message: SignedAuthorityCapabilitiesV1::new_from_data_and_sig(capabilities, signature),
    };

    assert!(
        validator_service
            .notify_capabilities(make_v2_notify_capabilities_request(request1))
            .await
            .is_err(),
        "Expected capability notification from unauthorized authority to be rejected"
    );

    // Request from the committee member itself — also rejected.
    let authority_capabilities = AuthorityCapabilitiesV1::new(
        authority_state.name,
        Chain::Mainnet,
        SupportedProtocolVersions::new_for_testing(1, 10),
        vec![],
    );
    let authority_signature = AuthoritySignature::new_secure(
        &IntentMessage::new(
            Intent::iota_app(AuthorityCapabilities),
            &authority_capabilities,
        ),
        &authority_state.current_epoch_for_testing(),
        &*authority_state.secret,
    );
    let request2 = HandleCapabilityNotificationRequestV1 {
        message: SignedAuthorityCapabilitiesV1::new_from_data_and_sig(
            authority_capabilities,
            authority_signature,
        ),
    };

    assert!(
        validator_service
            .notify_capabilities(make_v2_notify_capabilities_request(request2))
            .await
            .is_err(),
        "Expected capability notification from committee member to be rejected"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_v2_notify_capabilities_feature_disabled() {
    telemetry_subscribers::init_for_testing();

    let (_sender, sender_key): (_, AuthorityKeyPair) = get_authority_key_pair();

    let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    protocol_config.set_select_committee_from_eligible_validators_for_testing(false);
    protocol_config.set_track_non_committee_eligible_validators_for_testing(false);
    protocol_config.set_select_committee_supporting_next_epoch_version(false);

    let authority_state = TestAuthorityBuilder::new()
        .with_protocol_config(protocol_config)
        .build()
        .await;

    let consensus_adapter = Arc::new(ConsensusAdapter::new(
        Arc::new(MockConsensusClient::new()),
        CheckpointStore::new_for_tests(),
        authority_state.name,
        Arc::new(ConnectionMonitorStatusForTests {}),
        100_000,
        100_000,
        None,
        None,
        ConsensusAdapterMetrics::new_test(),
    ));

    let validator_service = Arc::new(ValidatorService::new_for_tests(
        authority_state.clone(),
        consensus_adapter,
        Arc::new(ValidatorServiceMetrics::new_for_tests()),
    ));

    let capabilities = AuthorityCapabilitiesV1::new(
        AuthorityName::new(sender_key.public().pubkey.to_bytes()),
        Chain::Mainnet,
        SupportedProtocolVersions::new_for_testing(1, 10),
        vec![],
    );
    let signature = AuthoritySignature::new_secure(
        &IntentMessage::new(Intent::iota_app(AuthorityCapabilities), &capabilities),
        &authority_state.current_epoch_for_testing(),
        &sender_key,
    );
    let request = HandleCapabilityNotificationRequestV1 {
        message: SignedAuthorityCapabilitiesV1::new_from_data_and_sig(capabilities, signature),
    };

    let result = validator_service
        .notify_capabilities(make_v2_notify_capabilities_request(request))
        .await;

    assert!(
        result.is_err(),
        "Expected capability notification to be rejected due to feature being disabled"
    );
    let err_kind = IotaError::from(result.unwrap_err());
    assert!(
        matches!(err_kind, IotaError::UnsupportedFeature { .. }),
        "Expected UnsupportedFeature error, but got {err_kind:?}",
    );
}
