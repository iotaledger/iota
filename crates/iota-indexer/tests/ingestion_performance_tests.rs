// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Instant;

use iota_indexer::{
    store::indexer_store::IndexerStore,
    test_utils::{IndexerTypeConfig, start_test_indexer, start_test_indexer_grpc},
};
use test_cluster::TestClusterBuilder;
use tokio_util::sync::CancellationToken;

const START_CP: u64 = 0;
const MAX_CP: u64 = 20;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_checkpoint_sync_performance_rest() {
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
    let (store, handle) = start_test_indexer(
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
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if let Ok((_min_cp, max_cp)) = store.get_available_checkpoint_range().await {
                if max_cp >= MAX_CP {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Indexer did not process checkpoints in time");
    let elapsed = t0.elapsed();
    println!(
        "[REST] Synced checkpoints {}-{} in {:?}",
        START_CP, MAX_CP, elapsed
    );

    // Clean up
    drop(handle);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_checkpoint_sync_performance_grpc() {
    // Start a test cluster with gRPC enabled
    let grpc_port = 50052u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);
    let cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .with_num_validators(1)
        .build()
        .await;

    // Wait for a range of checkpoints to be available
    cluster.wait_for_checkpoint(MAX_CP, None).await;

    // Prepare DB and indexer
    let db_url = "postgres://postgres:postgrespw@localhost:5432/iota_indexer_perf_grpc".to_string();
    let cancel = CancellationToken::new();
    let (store, handle) = start_test_indexer_grpc(
        db_url.clone(),
        true,
        None,
        format!("http://{}", grpc_addr),
        None,
        cancel.clone(),
    )
    .await;

    // Wait for the indexer to process up to MAX_CP
    let t0 = Instant::now();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if let Ok((_min_cp, max_cp)) = store.get_available_checkpoint_range().await {
                if max_cp >= MAX_CP {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Indexer did not process checkpoints in time");
    let elapsed = t0.elapsed();
    println!(
        "[gRPC] Synced checkpoints {}-{} in {:?}",
        START_CP, MAX_CP, elapsed
    );

    // Clean up
    drop(handle);
    // cancel.cancel();
    // let _ = handle.await;
}
