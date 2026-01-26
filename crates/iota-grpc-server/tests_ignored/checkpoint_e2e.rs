// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::time::Duration;

use futures::StreamExt;
use iota_config::local_ip_utils;
use iota_grpc_client::{LedgerClient, NodeClient};
use test_cluster::{TestCluster, TestClusterBuilder};

async fn setup_test_cluster_and_client(
    client_max_message_size_bytes: Option<u32>,
) -> (TestCluster, LedgerClient) {
    let localhost = local_ip_utils::localhost_for_testing();
    let grpc_port = local_ip_utils::get_available_port(&localhost);
    let grpc_addr = format!("{localhost}:{grpc_port}");

    // Start a test cluster with gRPC enabled and pruning disabled
    let cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.parse().expect("Invalid gRPC address"))
        .disable_fullnode_pruning()
        .with_num_validators(1)
        .build()
        .await;

    let client = NodeClient::connect(
        &format!("http://{grpc_addr}"),
        client_max_message_size_bytes,
    )
    .await
    .expect("connect gRPC");

    let ledger_service_client = client
        .ledger_service_client()
        .expect("Ledger service client should be available");

    (cluster, ledger_service_client)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_checkpoint_summary() {
    let (cluster, mut client) = setup_test_cluster_and_client(None).await;

    // Wait for 2 new checkpoint to be available
    cluster.wait_for_checkpoint(2, None).await;

    // Test getting checkpoint summary for sequence number 0
    let summary = client.get_checkpoint_summary(0).await.expect("gRPC call");

    // Verify the summary structure
    let digest_0 = match &summary {
        iota_grpc_types::checkpoints::CertifiedCheckpointSummary::V1(v1_summary) => {
            assert_eq!(v1_summary.data().epoch, 0);
            assert_eq!(v1_summary.data().sequence_number, 0);
            // Verify digest is not all zeros
            assert_ne!(v1_summary.data().content_digest.inner(), &[0u8; 32]);
            v1_summary.data().content_digest
        }
    };

    // Test getting another checkpoint summary
    let summary_1 = client.get_checkpoint_summary(1).await.expect("gRPC call");

    let digest_1 = match &summary_1 {
        iota_grpc_types::checkpoints::CertifiedCheckpointSummary::V1(v1_summary_1) => {
            assert_eq!(v1_summary_1.data().epoch, 0);
            assert_eq!(v1_summary_1.data().sequence_number, 1);
            v1_summary_1.data().content_digest
        }
    };

    // Different checkpoints should have different digests
    assert_ne!(digest_0, digest_1);

    // Test getting checkpoint summary for a non-existent sequence number
    match client.get_checkpoint_summary(999999).await {
        Ok(_) => {
            panic!("Unexpectedly found checkpoint summary for non-existent sequence number");
        }
        Err(status) => {
            assert!(status.code() == tonic::Code::NotFound);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_checkpoint_data() {
    let (cluster, mut client) = setup_test_cluster_and_client(None).await;

    // Wait for 2 new checkpoint to be available
    cluster.wait_for_checkpoint(2, None).await;

    // Test getting checkpoint data for sequence number 0
    let checkpoint_data = client.get_checkpoint_data(0).await.expect("gRPC call");

    // Verify the checkpoint data structure
    let digest_0 = match &checkpoint_data {
        iota_grpc_types::checkpoints::CheckpointData::V1(v1_data) => {
            assert_eq!(v1_data.checkpoint_summary.sequence_number, 0);
            assert_eq!(v1_data.checkpoint_summary.epoch, 0);
            assert!(!v1_data.transactions.is_empty());
            assert!(v1_data.checkpoint_contents.size() > 0);

            v1_data.checkpoint_summary.content_digest
        }
    };

    // Test getting another checkpoint
    let checkpoint_data_1 = client.get_checkpoint_data(1).await.expect("gRPC call");

    let digest_1 = match &checkpoint_data_1 {
        iota_grpc_types::checkpoints::CheckpointData::V1(v1_data_1) => {
            assert_eq!(v1_data_1.checkpoint_summary.sequence_number, 1);
            assert_eq!(v1_data_1.checkpoint_summary.epoch, 0);

            v1_data_1.checkpoint_summary.content_digest
        }
    };

    // Verify they are different checkpoints
    assert_ne!(digest_0, digest_1);

    // Test getting checkpoint data for a non-existent sequence number
    match client.get_checkpoint_data(999999).await {
        Ok(_) => {
            panic!("Unexpectedly found checkpoint data for non-existent sequence number");
        }
        Err(status) => {
            assert!(status.code() == tonic::Code::NotFound);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_stream_checkpoint_summaries() {
    let (_cluster, mut client) = setup_test_cluster_and_client(None).await;

    // Request checkpoint summaries using the higher-level GrpcNodeClient API
    let mut stream = client
        .stream_checkpoint_summaries(None, None)
        .await
        .expect("gRPC call");

    // Only collect the first 20 checkpoints to avoid hanging
    let mut indices = Vec::new();
    let mut count = 0;

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.next().await {
            match res {
                Ok(summary) => match summary {
                    iota_grpc_types::checkpoints::CertifiedCheckpointSummary::V1(v1_summary) => {
                        indices.push(v1_summary.data().sequence_number);
                        count += 1;
                        if count >= 20 {
                            break;
                        }
                    }
                },
                Err(e) => {
                    panic!("Error streaming checkpoint: {e:?}");
                }
            }
        }
    })
    .await
    .expect("waiting for checkpoints timed out");

    // There should be at least 20 checkpoints
    assert!(indices.len() >= 20, "Should stream at least 20 checkpoints");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stream_checkpoint_data() {
    let (_cluster, mut client) = setup_test_cluster_and_client(None).await;

    let mut stream = Box::pin(client.stream_checkpoints(None, Some(2)).await.unwrap());

    tokio::time::timeout(Duration::from_secs(120), async {
        if let Some(res) = stream.next().await {
            match res {
                Ok(checkpoint_data) => match checkpoint_data {
                    iota_grpc_types::checkpoints::CheckpointData::V1(v1_data) => {
                        assert_eq!(v1_data.checkpoint_summary.sequence_number, 2);
                    }
                },
                Err(e) => {
                    panic!("Stream error: {e:?}");
                }
            }
        } else {
            panic!("No checkpoint data returned");
        }
    })
    .await
    .expect("waiting for checkpoint data timed out");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_epoch_first_checkpoint_sequence_number() {
    let (cluster, mut client) = setup_test_cluster_and_client(None).await;

    let sender = cluster.get_address_0();
    let receiver = cluster.get_address_1();

    // Wait for 2 new checkpoint to be available
    cluster.wait_for_checkpoint(2, None).await;

    // Advance to a new epoch
    cluster.force_new_epoch().await;
    cluster.transfer_iota_must_exceed(sender, receiver, 1).await;

    // Wait for 3 new checkpoints in the new epoch
    cluster.wait_for_checkpoint(3, None).await;
    cluster.force_new_epoch().await;
    cluster.transfer_iota_must_exceed(sender, receiver, 1).await;

    // List all checkpoints and their epochs using the gRPC stream
    let mut stream = client
        .stream_checkpoint_summaries(Some(0), None)
        .await
        .expect("gRPC stream");
    let mut all_indices = vec![];
    let mut all_epochs = vec![];

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.next().await {
            match res {
                Ok(summary) => match summary {
                    iota_grpc_types::checkpoints::CertifiedCheckpointSummary::V1(v1_summary) => {
                        let epoch = v1_summary.data().epoch;
                        all_indices.push(v1_summary.data().sequence_number);
                        all_epochs.push(epoch);
                        if v1_summary.data().sequence_number > 50 {
                            break;
                        }
                    }
                },
                Err(e) => {
                    panic!("gRPC stream error: {e:?}");
                }
            }
        }
    })
    .await
    .expect("waiting for checkpoints timed out");

    // Query for the first checkpoint of epoch 0 (should be 0)
    let first_0 = client
        .get_epoch_first_checkpoint_sequence_number(0)
        .await
        .expect("gRPC call");
    assert_eq!(first_0, 0, "First checkpoint of epoch 0 should be 0");

    // Query for the first checkpoint of epoch 1 (should be >= 2)
    let first_1 = client
        .get_epoch_first_checkpoint_sequence_number(1)
        .await
        .expect("gRPC call");
    assert!(
        first_1 >= 2,
        "First checkpoint of epoch 1 should be >= 2, got {first_1}"
    );
}
