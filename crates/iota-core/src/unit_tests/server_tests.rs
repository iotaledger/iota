// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::traits::KeyPair;
use iota_protocol_config::{Chain, ProtocolConfig};
use iota_sdk_types::crypto::{Intent, IntentMessage, IntentScope::AuthorityCapabilities};
use iota_types::{
    base_types::{AuthorityName, dbg_addr, dbg_object_id},
    crypto::{
        AuthorityKeyPair, AuthoritySignature, IotaAuthoritySignature, get_authority_key_pair,
    },
    error::IotaError,
    messages_consensus::{AuthorityCapabilitiesV1, SignedAuthorityCapabilitiesV1},
    messages_grpc::{
        LayoutGenerationOption, RawSubmitTransactionsRequest, RawSubmitTransactionsResponse,
        SubmitTransactionsRequest, SubmitTransactionsResponse,
    },
    supported_protocol_versions::SupportedProtocolVersions,
};
// Additional imports for white flag tests
use iota_types::{
    base_types::{Identifier, IotaAddress, ObjectID, random_object_ref},
    crypto::{AccountKeyPair, get_key_pair},
    messages_grpc::SubmitTransactionResult,
    object::Object,
    transaction::{
        CallArg, Command, ProgrammableTransaction, TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionData,
    },
    utils::to_sender_signed_transaction,
};

use super::*;
use crate::{
    authority::{
        AuthorityState,
        authority_test_utils::init_certified_transaction,
        authority_tests::{init_state_with_ids_and_object_basics, init_state_with_object_id},
        test_authority_builder::TestAuthorityBuilder,
    },
    authority_client::{NetworkAuthorityClient, validator::ValidatorAPI},
    authority_server::{AuthorityServer, ValidatorServiceMetrics, make_tonic_request_for_testing},
    checkpoints::CheckpointStore,
    consensus_adapter::{
        ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics,
        MockConsensusClient,
    },
};

/// Helper to make a tonic request with a native SubmitTransactionsRequest
/// converted to Raw* for the protobuf wire format.
fn make_raw_submit_request(
    request: SubmitTransactionsRequest,
) -> tonic::Request<RawSubmitTransactionsRequest> {
    let raw: RawSubmitTransactionsRequest = request.try_into().expect("BCS serialization failed");
    make_tonic_request_for_testing(raw)
}

/// Helper to convert a Raw submit response back to native types for assertion.
fn into_native_submit_response(
    result: Result<(tonic::Response<RawSubmitTransactionsResponse>, Weight), tonic::Status>,
) -> Result<(tonic::Response<SubmitTransactionsResponse>, Weight), tonic::Status> {
    let (response, weight) = result?;
    let native: SubmitTransactionsResponse = response
        .into_inner()
        .try_into()
        .map_err(|e: IotaError| tonic::Status::internal(e.to_string()))?;
    Ok((tonic::Response::new(native), weight))
}

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
        Some(
            authority_state
                .config
                .network_key_pair()
                .public()
                .to_owned(),
        ),
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

// White Flag Flow Tests

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_transaction_v1_feature_flag_disabled() {
    telemetry_subscribers::init_for_testing();

    // Create authority with default config (white flag disabled)
    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ])
        .build()
        .await;

    // Create validator service with mock consensus
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

    // Create a valid transaction
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();
    let recipient = dbg_addr(2);

    let tx_data = TransactionData::new_transfer(
        recipient,
        object.compute_object_reference(),
        sender,
        gas.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    // Call submit_transaction
    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(
            SubmitTransactionsRequest::new_transaction(tx),
        ))
        .await;

    // Should return Err as the feature is not supported
    assert!(result.is_err(), "Expected an error but got Ok");
    let err = result.unwrap_err();
    assert_eq!(err.code(), tonic::Code::Internal);
    assert!(
        err.message()
            .contains("White flag flow is not enabled in this protocol version")
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_transaction_invalid_signature() {
    telemetry_subscribers::init_for_testing();

    // Enable white flag flow
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    // Create authority
    let (sender, _sender_key): (_, AccountKeyPair) = get_key_pair();
    let (_wrong_sender, wrong_key): (_, AccountKeyPair) = get_key_pair();
    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ])
        .build()
        .await;

    // Create validator service with mock consensus
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

    // Create transaction with wrong signature
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();
    let recipient = dbg_addr(2);

    let tx_data = TransactionData::new_transfer(
        recipient,
        object.compute_object_reference(),
        sender,
        gas.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    // Sign with wrong key
    let tx = to_sender_signed_transaction(tx_data, &wrong_key);

    // Call submit_transaction
    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(
            SubmitTransactionsRequest::new_transaction(tx),
        ))
        .await;

    // Signature errors are reported per-transaction as Rejected results.
    let result = into_native_submit_response(result);
    assert!(result.is_ok(), "Batch response should be Ok");
    let response = result.unwrap().0.into_inner();
    assert_eq!(response.results.len(), 1);
    match &response.results[0] {
        SubmitTransactionResult::Rejected { error } => {
            let msg = format!("{:?}", error).to_lowercase();
            assert!(
                msg.contains("signature"),
                "Error should mention signature, got: {msg}",
            );
        }
        other => panic!("Expected Rejected result, got {:?}", other),
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_transaction_success() {
    telemetry_subscribers::init_for_testing();

    // Enable white flag flow
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    // Create authority
    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

    let authority_state = TestAuthorityBuilder::new()
        .with_starting_objects(&[
            Object::with_id_owner_for_testing(object_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ])
        .build()
        .await;

    // Create validator service with mock consensus
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

    // Create transaction
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let object = authority_state.get_object(&object_id).await.unwrap();
    let gas = authority_state.get_object(&gas_id).await.unwrap();
    let recipient = dbg_addr(2);

    let tx_data = TransactionData::new_transfer(
        recipient,
        object.compute_object_reference(),
        sender,
        gas.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    // Call submit_transaction
    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(
            SubmitTransactionsRequest::new_transaction(tx),
        ))
        .await;

    // Should succeed with Submitted result
    let result = into_native_submit_response(result);
    assert!(result.is_ok(), "Transaction submission should succeed");
    let response = result.unwrap().0.into_inner();
    match &response.results[0] {
        SubmitTransactionResult::Submitted => {
            // Success - transaction was submitted to consensus
        }
        other => panic!("Expected Submitted result, got {:?}", other),
    }
}

// NOTE: Fullnode test removed as TestAuthorityBuilder doesn't expose
// a simple way to build a fullnode. The fullnode rejection logic is tested
// in integration tests.

// ── Helper ────────────────────────────────────────────────────────────────────

/// Builds a `use_clock` transaction (shared-object) signed by `sender_key`.
/// Soft-bundle validity check requires every transaction to contain at least
/// one shared object, and the clock satisfies that requirement.
async fn build_shared_object_transaction(
    state: &AuthorityState,
    sender: IotaAddress,
    sender_key: &AccountKeyPair,
    gas_object_id: ObjectID,
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
        gas.compute_object_reference(),
        vec![CallArg::CLOCK_IMMUTABLE],
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp,
        rgp,
    )
    .unwrap();
    to_sender_signed_transaction(tx_data, sender_key)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// A PTB with an empty `SplitCoins` args list is structurally invalid and
/// fails `validity_check` before signature verification or deny checks.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_transaction_invalid_transaction() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id = ObjectID::random();

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

    // Build a PTB with a SplitCoins command that has an empty amounts list —
    // this is caught by validity_check as UserInputError::EmptyCommandInput.
    let pt = ProgrammableTransaction {
        inputs: vec![],
        commands: vec![Command::SplitCoins(
            iota_types::transaction::Argument::Gas,
            vec![], // empty — invalid
        )],
    };
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas.compute_object_reference()],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(
            SubmitTransactionsRequest::new_transaction(tx),
        ))
        .await;

    let (response, _weight) = into_native_submit_response(result)
        .expect("batch call should succeed even when individual txs fail");
    let results = response.into_inner().results;
    assert_eq!(results.len(), 1);
    assert!(
        matches!(&results[0], SubmitTransactionResult::Rejected { .. }),
        "Expected Rejected for invalid transaction, got {:?}",
        results[0]
    );
}

/// Re-submitting an already-executed transaction returns
/// `SubmitTransactionResult::Executed` with a populated `details` field.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_transaction_already_executed() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let object_id = ObjectID::random();
    let gas_id = ObjectID::random();

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
        object.compute_object_reference(),
        sender,
        gas.compute_object_reference(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    // Execute the transaction directly (bypasses handle_transaction, which is
    // disabled when white-flag flow is enabled).
    // TODO: prepare helper methods to avoid creating certified transactions, but
    // rather executing UserTransactions directly in tests.
    let cert = init_certified_transaction(tx.clone(), &authority_state);
    let (effects, _) = authority_state.execute_for_test(&cert);

    // Re-submit the same transaction via the white-flag endpoint.
    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(
            SubmitTransactionsRequest::new_transaction(tx),
        ))
        .await;

    let result = into_native_submit_response(result);
    assert!(result.is_ok(), "Expected Ok for already-executed tx");
    let response = result.unwrap().0.into_inner();
    assert_eq!(response.results.len(), 1, "Expected exactly one result");
    match &response.results[0] {
        SubmitTransactionResult::Executed { effects_digest, .. } => {
            assert_eq!(effects_digest, effects.digest());
        }
        other => panic!("Expected Executed result, got {:?}", other),
    }
}

/// A transaction with a random (non-existent) gas object fails during deny
/// checks. IOTA maps deny-check errors to `tonic::Status` via
/// `.map_err(tonic::Status::from)?`, producing a hard `Err` rather than a
/// `Rejected` variant.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_transaction_gas_object_validation() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let object_id = ObjectID::random();

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

    // Use a random object ref that doesn't exist in the store as gas payment.
    let tx_data = TransactionData::new_transfer(
        dbg_addr(2),
        object.compute_object_reference(),
        sender,
        random_object_ref(), // non-existent gas object
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(
            SubmitTransactionsRequest::new_transaction(tx),
        ))
        .await;

    // Non-existent gas object errors are reported per-transaction as Rejected
    // results. TODO: check for a specific error kind once we have better error
    // handling in place for the white-flag flow.
    let result = into_native_submit_response(result);
    assert!(result.is_ok(), "Batch response should be Ok");
    let response = result.unwrap().0.into_inner();
    assert_eq!(response.results.len(), 1);
    match &response.results[0] {
        SubmitTransactionResult::Rejected { .. } => {}
        other => panic!("Expected Rejected result, got {:?}", other),
    }
}

/// Soft-bundle happy path: two `use_clock` (shared-object) transactions
/// submitted together are accepted and return `Submitted`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_soft_bundle_transactions() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id1 = ObjectID::random();
    let gas_id2 = ObjectID::random();

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

    // len > 1 triggers the soft-bundle path.
    let request = SubmitTransactionsRequest {
        transactions: vec![tx1, tx2],
    };

    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(request))
        .await;

    let result = into_native_submit_response(result);
    assert!(result.is_ok(), "Batch submission should succeed");
    let response = result.unwrap().0.into_inner();
    assert_eq!(
        response.results.len(),
        2,
        "Expected one result per transaction"
    );
    for (i, result) in response.results.iter().enumerate() {
        match result {
            SubmitTransactionResult::Submitted => {}
            other => panic!("Expected Submitted for tx {i}, got {:?}", other),
        }
    }
}

/// In white-flag mode, transactions with different gas prices are processed
/// independently — there is no bundle-level gas price matching. Each
/// transaction is submitted on its own regardless of gas price differences.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_transactions_different_gas_prices_accepted() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id1 = ObjectID::random();
    let gas_id2 = ObjectID::random();

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

    // tx1 at base rgp, tx2 at 2× rgp — both should be accepted independently.
    let rgp = authority_state.reference_gas_price_for_testing().unwrap();
    let gas1 = authority_state.get_object(&gas_id1).await.unwrap();
    let gas2 = authority_state.get_object(&gas_id2).await.unwrap();

    let tx_data1 = TransactionData::new_move_call(
        sender,
        pkg_ref.object_id,
        Identifier::from_static("object_basics"),
        Identifier::from_static("use_clock"),
        vec![],
        gas1.compute_object_reference(),
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
        gas2.compute_object_reference(),
        vec![CallArg::CLOCK_IMMUTABLE],
        TEST_ONLY_GAS_UNIT_FOR_OBJECT_BASICS * rgp * 2,
        rgp * 2, // different price — accepted in white-flag mode
    )
    .unwrap();
    let tx2 = to_sender_signed_transaction(tx_data2, &sender_key);

    let request = SubmitTransactionsRequest {
        transactions: vec![tx1, tx2],
    };

    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(request))
        .await;

    // With soft-bundle checks removed, each transaction is processed independently.
    // Different gas prices no longer cause a batch-level rejection.
    let result = into_native_submit_response(result);
    assert!(result.is_ok(), "Batch response should be Ok");
    let response = result.unwrap().0.into_inner();
    assert_eq!(
        response.results.len(),
        2,
        "Expected one result per transaction"
    );
    for (i, result) in response.results.iter().enumerate() {
        match result {
            SubmitTransactionResult::Submitted => {}
            other => panic!("Expected Submitted for tx {i}, got {:?}", other),
        }
    }
}

/// A transaction whose serialized size exceeds `max_tx_size_bytes` (128 KiB)
/// is rejected by `validity_check` before any signature or deny-check logic.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn test_submit_oversized_transaction() {
    telemetry_subscribers::init_for_testing();

    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_white_flag_flow_for_testing(true);
        config
    });

    let (sender, sender_key): (_, AccountKeyPair) = get_key_pair();
    let gas_id = ObjectID::random();

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
    // Each pure arg is 14 KiB (below the 16 KiB per-arg limit), so the individual
    // arg check doesn't trigger first — only the overall size limit does.
    let inputs: Vec<_> = (0u8..10)
        .map(|i| CallArg::Pure(vec![i; 14 * 1024]))
        .collect();
    let pt = ProgrammableTransaction {
        inputs,
        commands: vec![],
    };
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas.compute_object_reference()],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);

    let result = validator_service
        .handle_submit_transactions_impl(make_raw_submit_request(
            SubmitTransactionsRequest::new_transaction(tx),
        ))
        .await;

    let (response, _weight) = into_native_submit_response(result)
        .expect("batch call should succeed even when individual txs fail");
    let results = response.into_inner().results;
    assert_eq!(results.len(), 1);
    assert!(
        matches!(&results[0], SubmitTransactionResult::Rejected { .. }),
        "Expected Rejected for oversized transaction, got {:?}",
        results[0]
    );
}
