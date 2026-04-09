// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::ResponseExt;
use iota_macros::sim_test;

use super::super::utils::setup_grpc_test;

#[sim_test]
async fn metadata_envelope_headers() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;

    // Use get_service_info to get a MetadataEnvelope and verify metadata headers.
    let response = client
        .get_service_info(None)
        .await
        .expect("get_service_info should succeed");

    // The server should populate chain metadata in response headers.
    assert!(response.chain().is_some(), "chain header should be present");
    assert!(
        response.chain_id().is_some(),
        "chain_id header should be present"
    );
    assert!(response.epoch().is_some(), "epoch header should be present");
    assert!(
        response.checkpoint_height().is_some(),
        "checkpoint_height header should be present"
    );
    assert!(
        response.timestamp_ms().is_some(),
        "timestamp_ms header should be present"
    );
    assert!(
        response.timestamp().is_some(),
        "timestamp header should be present"
    );
    assert!(
        response.lowest_available_checkpoint().is_some(),
        "lowest_available_checkpoint header should be present"
    );
    assert!(
        response.lowest_available_checkpoint_objects().is_some(),
        "lowest_available_checkpoint_objects header should be present"
    );
    assert!(
        response.server_version().is_some(),
        "server_version header should be present"
    );

    // Verify that the body is also accessible through body().
    assert!(
        response.body().chain_id.is_some(),
        "service info body should contain chain_id"
    );

    // Both body and header should report a chain_id.
    assert!(
        response.chain_id().is_some(),
        "header chain_id should be present alongside body chain_id"
    );

    // Also verify with get_health — a different endpoint should also carry
    // metadata headers.
    let health = client
        .get_health(None)
        .await
        .expect("get_health should succeed");

    assert!(
        health.epoch().is_some(),
        "health response should carry epoch header"
    );
    assert!(
        health.checkpoint_height().is_some(),
        "health response should carry checkpoint_height header"
    );
    assert!(
        health.chain().is_some(),
        "health response should carry chain header"
    );
}
