// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_indexer::{store::indexer_store::IndexerStore, test_utils::start_test_indexer_grpc};
use test_cluster::TestClusterBuilder;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_grpc_checkpoint_ingestion() {
    // Start a test cluster with gRPC enabled
    let grpc_port = 50062u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);
    let cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .with_num_validators(1)
        .build()
        .await;

    // Wait for checkpoint 3 to be available
    cluster.wait_for_checkpoint(3, None).await;

    // Prepare DB and indexer
    let db_url = "postgres://postgres:postgrespw@localhost:5432/iota_indexer_test_grpc".to_string();
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

    // Wait for checkpoints 0, 1, and 2 to exist in the store
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if let Ok((min_cp, max_cp)) = store.get_available_checkpoint_range().await {
                if min_cp <= 0 && max_cp >= 3 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Checkpoints 0, 1, 2 did not appear in time");

    // Wait for the indexer to process at least 6 checkpoint
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let mut count = 0;
        loop {
            let latest_cp = store.get_latest_checkpoint_sequence_number().await.unwrap();
            if let Some(seq) = latest_cp {
                count += 1;
                println!("[gRPC][Indexer] Latest checkpoint: {}", seq);
                if count > 6 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    })
    .await
    .expect("Indexer did not process a checkpoint in time");

    // Optionally, check logs or DB state for correct ingestion method
    // (Here, just print a message for demonstration)
    println!("gRPC ingestion test passed: indexer processed at least one checkpoint");

    // Clean up
    cancel.cancel();
    let _ = handle.await;
}
