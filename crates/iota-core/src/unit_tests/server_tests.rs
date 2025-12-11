// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::{Chain, ProtocolConfig};
use iota_sdk_types::crypto::{Intent, IntentMessage, IntentScope::AuthorityCapabilities};
use iota_types::{
    base_types::{AuthorityName, dbg_addr, dbg_object_id},
    crypto::{
        AuthorityKeyPair, AuthoritySignature, IotaAuthoritySignature, get_authority_key_pair,
    },
    error::IotaError,
    messages_consensus::{AuthorityCapabilitiesV1, SignedAuthorityCapabilitiesV1},
    messages_grpc::LayoutGenerationOption,
    supported_protocol_versions::SupportedProtocolVersions,
};

use super::*;
use crate::{
    authority::{
        authority_tests::init_state_with_object_id, test_authority_builder::TestAuthorityBuilder,
    },
    authority_client::{AuthorityAPI, NetworkAuthorityClient},
    consensus_adapter::MockConsensusClient,
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
