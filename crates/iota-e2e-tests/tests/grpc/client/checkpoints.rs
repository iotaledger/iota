// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;

use super::common::{assert_not_found_error, setup_grpc_test};

#[sim_test]
async fn get_latest_checkpoint() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let checkpoint = client
        .get_latest_checkpoint()
        .await
        .expect("Failed to get latest checkpoint");

    assert!(
        checkpoint.checkpoint.sequence_number >= 1,
        "Latest checkpoint sequence number should be at least 1"
    );
}

#[sim_test]
async fn get_checkpoint_by_sequence() {
    let (_test_cluster, client) = setup_grpc_test(2).await;

    let checkpoint = client
        .get_checkpoint(1)
        .await
        .expect("Failed to get checkpoint by sequence number");

    assert_eq!(
        checkpoint.checkpoint.sequence_number, 1,
        "Checkpoint sequence number should match requested"
    );
}

#[sim_test]
async fn get_checkpoint_sequence_zero() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let checkpoint = client
        .get_checkpoint(0)
        .await
        .expect("Failed to get genesis checkpoint");

    assert_eq!(
        checkpoint.checkpoint.sequence_number, 0,
        "Genesis checkpoint should have sequence number 0"
    );
}

#[sim_test]
async fn get_checkpoint_nonexistent_sequence() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let result = client.get_checkpoint(999_999_999).await;
    assert_not_found_error(result);
}

#[sim_test]
async fn get_checkpoint_future_sequence() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let latest = client
        .get_latest_checkpoint()
        .await
        .expect("Failed to get latest checkpoint");

    let future_sequence = latest.checkpoint.sequence_number + 100;
    let result = client.get_checkpoint(future_sequence).await;

    assert!(
        result.is_err(),
        "Fetching future checkpoint should return an error"
    );
}
