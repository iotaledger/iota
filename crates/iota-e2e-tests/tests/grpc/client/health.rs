// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::Error;
use iota_macros::sim_test;

use super::super::utils::setup_grpc_test;

#[sim_test]
async fn get_health() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;

    // Default threshold: should succeed and return checkpoint info.
    let response = client
        .get_health(None)
        .await
        .expect("Health check should succeed with default threshold");
    assert!(
        response.body().executed_checkpoint_height.is_some(),
        "Response should include executed_checkpoint_height"
    );
    assert!(
        response.body().estimated_validator_latency_ms.is_none(),
        "estimated_validator_latency_ms should be None (not yet implemented)"
    );

    // Large threshold: should always pass.
    let response = client
        .get_health(Some(u64::MAX))
        .await
        .expect("Health check should succeed with a large threshold");
    assert!(
        response.body().executed_checkpoint_height.unwrap() >= 1,
        "Executed checkpoint height should be at least 1"
    );

    // Zero threshold: the checkpoint must be from right now, which is
    // virtually impossible — expect UNAVAILABLE.
    let err = client
        .get_health(Some(0))
        .await
        .expect_err("Health check should fail with zero threshold");
    match err {
        Error::Grpc(status) => {
            assert_eq!(
                status.code(),
                tonic::Code::Unavailable,
                "Expected UNAVAILABLE, got: {status}"
            );
        }
        other => panic!("Expected Error::Grpc, got: {other}"),
    }
}
