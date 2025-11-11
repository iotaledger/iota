// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use futures::StreamExt;
use iota_config::local_ip_utils;
use iota_grpc_client::{LedgerClient, NodeClient};
use test_cluster::{TestCluster, TestClusterBuilder};

async fn setup_test_cluster_and_client() -> (TestCluster, LedgerClient) {
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

    let client = NodeClient::connect(&format!("http://{grpc_addr}"))
        .await
        .expect("connect gRPC");

    let ledger_service_client = client
        .ledger_service_client()
        .expect("Ledger service client should be available");

    (cluster, ledger_service_client)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_stream_checkpoints() {
    let (_cluster, mut client) = setup_test_cluster_and_client().await;

    // Request all checkpoints using the higher-level GrpcNodeClient API
    let mut stream = client
        .stream_checkpoints(None, None, false)
        .await
        .expect("gRPC call");

    // Only collect the first 20 checkpoints to avoid hanging
    let mut indices = Vec::new();
    let mut count = 0;

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.next().await {
            match res {
                Ok(checkpoint_content) => match checkpoint_content {
                    iota_grpc_client::CheckpointContent::Summary(summary) => match summary {
                        iota_grpc_types::checkpoints::CertifiedCheckpointSummary::V1(
                            v1_summary,
                        ) => {
                            indices.push(v1_summary.data().sequence_number);
                            count += 1;
                            if count >= 20 {
                                break;
                            }
                        }
                    },
                    iota_grpc_client::CheckpointContent::Data(_) => {
                        panic!("Expected summary, got data");
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
async fn test_get_epoch_first_checkpoint_sequence_number() {
    let (cluster, mut client) = setup_test_cluster_and_client().await;

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
        .stream_checkpoints(Some(0), None, false)
        .await
        .expect("gRPC stream");
    let mut all_indices = vec![];
    let mut all_epochs = vec![];

    tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(res) = stream.next().await {
            match res {
                Ok(checkpoint_content) => match checkpoint_content {
                    iota_grpc_client::CheckpointContent::Summary(summary) => match summary {
                        iota_grpc_types::checkpoints::CertifiedCheckpointSummary::V1(
                            v1_summary,
                        ) => {
                            let epoch = v1_summary.data().epoch;
                            all_indices.push(v1_summary.data().sequence_number);
                            all_epochs.push(epoch);
                            if v1_summary.data().sequence_number > 50 {
                                break;
                            }
                        }
                    },
                    iota_grpc_client::CheckpointContent::Data(_) => {
                        panic!("Expected summary, got data");
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_stream_full_checkpoint_data() {
    let (_cluster, mut client) = setup_test_cluster_and_client().await;

    let mut stream = client
        .stream_checkpoints(None, Some(2), true)
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(120), async {
        if let Some(res) = stream.next().await {
            match res {
                Ok(checkpoint_content) => match checkpoint_content {
                    iota_grpc_client::CheckpointContent::Data(checkpoint_data) => {
                        match checkpoint_data {
                            iota_grpc_types::checkpoints::CheckpointData::V1(v1_data) => {
                                assert_eq!(v1_data.checkpoint_summary.sequence_number, 2);
                            }
                        }
                    }
                    iota_grpc_client::CheckpointContent::Summary(_) => {
                        panic!("Expected data, got summary");
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
