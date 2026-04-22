// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use futures::StreamExt;
use iota_macros::sim_test;
use tokio::time::timeout;

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

    // Test: get checkpoint by digest round-trips via the default mask
    let genesis_digest = genesis
        .body()
        .summary()
        .expect("genesis should have summary")
        .digest()
        .expect("genesis summary should have a digest");
    let by_digest = client
        .get_checkpoint_by_digest(genesis_digest, None, None, None)
        .await
        .expect("Failed to get checkpoint by digest");
    assert_eq!(
        by_digest.body().sequence_number(),
        0,
        "by-digest lookup should return the genesis checkpoint"
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

#[sim_test]
async fn stream_checkpoints_live() {
    // Live-streaming sanity check: open the stream before checkpoint 2 exists
    // and verify the broadcaster delivers it within the 120s window. This is
    // the one test that exercises the live (non-historical) streaming path —
    // every other checkpoint test either replays historical checkpoints or
    // pre-waits for the range to be produced.
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut stream = client
        .stream_checkpoints(Some(2), Some(2), None, None, None)
        .await
        .expect("Failed to open checkpoint stream");

    let first = timeout(Duration::from_secs(120), stream.body_mut().next())
        .await
        .expect("waiting for live checkpoint timed out")
        .expect("stream ended without a checkpoint")
        .expect("stream error");
    assert_eq!(first.sequence_number, 2, "expected checkpoint 2");
}
