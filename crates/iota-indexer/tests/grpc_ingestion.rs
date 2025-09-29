// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_config::local_ip_utils;
use iota_grpc_api::client::{CheckpointClient, NodeClient};
use iota_indexer::{store::indexer_store::IndexerStore, test_utils::start_test_indexer_grpc};
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio_util::sync::CancellationToken;

async fn setup_test_cluster_and_client() -> (TestCluster, CheckpointClient, String) {
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

    let node_client = NodeClient::connect(&format!("http://{grpc_addr}"))
        .await
        .expect("connect gRPC");

    let checkpoint_client = node_client
        .checkpoint_client()
        .expect("Checkpoint client should be available");

    (cluster, checkpoint_client, grpc_addr)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_grpc_checkpoint_ingestion() {
    // Start a test cluster with gRPC enabled
    let (cluster, _checkpoint_client, grpc_addr) = setup_test_cluster_and_client().await;

    // Wait for checkpoint 3 to be available
    cluster.wait_for_checkpoint(3, None).await;

    // Prepare DB and indexer
    let db_url = "postgres://postgres:postgrespw@localhost:5432/iota_indexer_test_grpc".to_string();
    let cancel = CancellationToken::new();
    let (store, handle) = start_test_indexer_grpc(
        db_url.clone(),
        true,
        None,
        format!("http://{grpc_addr}"),
        None,
        cancel.clone(),
    )
    .await;

    // Wait for checkpoints 0, 1, and 2 to exist in the store
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok((min_cp, max_cp)) = store.get_available_checkpoint_range().await {
                if min_cp == 0 && max_cp >= 3 {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Checkpoints 0, 1, 2 did not appear in time");

    // Wait for the indexer to process at least 6 checkpoint
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut count = 0;
        loop {
            let latest_cp = store.get_latest_checkpoint_sequence_number().await.unwrap();
            if let Some(_seq) = latest_cp {
                count += 1;
                if count > 6 {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("Indexer did not process a checkpoint in time");

    // Clean up
    cancel.cancel();
    let _ = handle.await;
}
