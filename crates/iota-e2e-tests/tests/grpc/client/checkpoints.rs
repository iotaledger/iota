// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;

use super::{super::utils::setup_grpc_test, common::assert_grpc_not_found};

#[sim_test]
async fn get_checkpoint_scenarios() {
    let (_test_cluster, client) = setup_grpc_test(Some(2), None).await;

    // Test: get latest checkpoint
    let latest = client
        .get_checkpoint_latest(None, None, None)
        .await
        .expect("Failed to get latest checkpoint");
    assert!(
        latest.body().sequence_number() >= 1,
        "Latest checkpoint sequence number should be at least 1"
    );

    // Test: get genesis checkpoint (sequence 0)
    let genesis = client
        .get_checkpoint_by_sequence_number(0, None, None, None)
        .await
        .expect("Failed to get genesis checkpoint");
    assert_eq!(
        genesis.body().sequence_number(),
        0,
        "Genesis checkpoint should have sequence number 0"
    );

    // Test: get checkpoint by sequence number
    let checkpoint_1 = client
        .get_checkpoint_by_sequence_number(1, None, None, None)
        .await
        .expect("Failed to get checkpoint by sequence number");
    assert_eq!(
        checkpoint_1.body().sequence_number(),
        1,
        "Checkpoint sequence number should match requested"
    );

    // Test: nonexistent checkpoint returns not-found error
    let result = client
        .get_checkpoint_by_sequence_number(999_999_999, None, None, None)
        .await;
    assert_grpc_not_found(result);

    // Test: future checkpoint returns not-found error
    let future_sequence = latest.body().sequence_number() + 100;
    let result = client
        .get_checkpoint_by_sequence_number(future_sequence, None, None, None)
        .await;
    assert_grpc_not_found(result);
}
