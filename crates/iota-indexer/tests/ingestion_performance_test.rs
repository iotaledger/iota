// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use iota_config::local_ip_utils;
use iota_grpc_api::client::{CheckpointClient, NodeClient};
use iota_indexer::{
    store::indexer_store::IndexerStore,
    test_utils::{IndexerTypeConfig, start_test_indexer, start_test_indexer_grpc},
};
use test_cluster::{TestCluster, TestClusterBuilder};
use tokio_util::sync::CancellationToken;
use tracing::info;

const START_CP: u64 = 0;
const MAX_CP: u64 = 19;

fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();
    });
}

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
async fn test_checkpoint_sync_performance_rest() {
    init_tracing();

    // Start a test cluster
    let cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .build()
        .await;

    // Wait for a range of checkpoints to be available
    cluster.wait_for_checkpoint(MAX_CP, None).await;

    // Prepare DB and indexer
    let db_url = "postgres://postgres:postgrespw@localhost:5432/iota_indexer_perf_rest".to_string();
    let rpc_url = cluster.rpc_url().to_string();
    let (store, handle, _cancel) = start_test_indexer(
        db_url.clone(),
        true,
        None,
        rpc_url,
        IndexerTypeConfig::writer_mode(None, None),
        None,
    )
    .await;

    // Wait for the indexer to process up to MAX_CP
    let t0 = Instant::now();
    tokio::time::timeout(Duration::from_secs(3600), async {
        loop {
            if let Ok((_min_cp, max_cp)) = store.get_available_checkpoint_range().await {
                if max_cp >= MAX_CP {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Indexer did not process checkpoints in time");
    let elapsed = t0.elapsed();
    info!("[REST] Synced checkpoints {START_CP}-{MAX_CP} in {elapsed:?}");

    // Clean up
    handle.abort();
    let _ = handle.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_checkpoint_sync_performance_grpc() {
    init_tracing();

    // Start a test cluster with gRPC enabled
    let (cluster, _checkpoint_client, grpc_addr) = setup_test_cluster_and_client().await;

    // Wait for a range of checkpoints to be available
    cluster.wait_for_checkpoint(MAX_CP, None).await;

    // Prepare DB and indexer
    let db_url = "postgres://postgres:postgrespw@localhost:5432/iota_indexer_perf_grpc".to_string();
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

    // Wait for the indexer to process up to MAX_CP
    let t0 = Instant::now();
    tokio::time::timeout(Duration::from_secs(3600), async {
        loop {
            if let Ok((_min_cp, max_cp)) = store.get_available_checkpoint_range().await {
                if max_cp >= MAX_CP {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Indexer did not process checkpoints in time");
    let elapsed = t0.elapsed();
    info!("[gRPC] Synced checkpoints {START_CP}-{MAX_CP} in {elapsed:?}");

    // Clean up
    cancel.cancel();
    let _ = handle.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_checkpoint_sync_performance_file() {
    init_tracing();

    use tempfile::tempdir;

    // Start a test cluster with gRPC enabled (needed for checkpoint export)
    let (cluster, mut checkpoint_client, _grpc_addr) = setup_test_cluster_and_client().await;

    // Wait for a range of checkpoints to be available
    cluster.wait_for_checkpoint(MAX_CP, None).await;

    // Export checkpoints to a temp directory
    let temp_dir = tempdir().unwrap();
    let checkpoint_dir = temp_dir.path().to_path_buf();

    // --- Generate and export checkpoints via gRPC FIRST ---
    {
        use futures::StreamExt;
        use iota_grpc_api::CheckpointContent;
        use iota_storage::blob::{Blob, BlobEncoding};

        let mut stream = checkpoint_client
            .stream_checkpoints(Some(START_CP), Some(MAX_CP), true)
            .await
            .expect("gRPC stream");

        let mut sequence_number = START_CP;
        while let Some(Ok(checkpoint_content)) = stream.next().await {
            // Handle the CheckpointContent
            let checkpoint_data = match checkpoint_content {
                CheckpointContent::Data(grpc_data) => {
                    // Convert from gRPC CheckpointData to iota_types CheckpointData
                    match grpc_data {
                        iota_grpc_types::CheckpointData::V1(v1_data) => {
                            iota_types::full_checkpoint_content::CheckpointData {
                                checkpoint_summary: v1_data.checkpoint_summary,
                                checkpoint_contents: v1_data.checkpoint_contents,
                                transactions: v1_data.transactions,
                            }
                        }
                    }
                }
                CheckpointContent::Summary(_) => {
                    panic!("Expected data, got summary")
                }
            };

            // Re-encode using Blob format (which is what the file reader expects)
            let blob = Blob::encode(&checkpoint_data, BlobEncoding::Bcs)
                .expect("encode checkpoint as blob");

            let file_path = checkpoint_dir.join(format!("{}.chk", sequence_number));
            std::fs::write(file_path, blob.to_bytes()).expect("write checkpoint file");
            sequence_number += 1;
        }
    }
    // --- End export ---

    // Now start the indexer with the populated checkpoint directory
    let db_url = "postgres://postgres:postgrespw@localhost:5432/iota_indexer_perf_file".to_string();
    let (store, handle, _cancel) = iota_indexer::test_utils::start_test_indexer(
        db_url.clone(),
        true,
        None,
        cluster.rpc_url().to_string(),
        IndexerTypeConfig::writer_mode(None, None),
        Some(checkpoint_dir.clone()),
    )
    .await;

    // Wait for the indexer to process up to MAX_CP
    let t0 = Instant::now();
    tokio::time::timeout(Duration::from_secs(3600), async {
        loop {
            if let Ok((_min_cp, max_cp)) = store.get_available_checkpoint_range().await {
                if max_cp >= MAX_CP {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Indexer did not process checkpoints in time");
    let elapsed = t0.elapsed();
    info!("[File] Synced checkpoints {START_CP}-{MAX_CP} in {elapsed:?}");

    // Clean up
    handle.abort();
    let _ = handle.await;
}
