// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use bcs;
use iota_data_ingestion::GrpcBlobWorker;
use iota_data_ingestion_core::Worker;
use iota_grpc_api::client::GrpcNodeClient;
use iota_storage::object_store::ObjectStoreGetExt;
use iota_types::full_checkpoint_content::CheckpointData;
use object_store::memory::InMemory;
use test_cluster::TestClusterBuilder;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_grpc_blob_worker_reset_logic() {
    // Start a test cluster with gRPC enabled
    // TODO: Fix the test error because now we only have the checkpoint summary
    // in the gRPC stream.
    let grpc_port = 50063u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);
    let cluster = TestClusterBuilder::new()
        .with_num_validators(4)
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .build()
        .await;

    // Wait for several checkpoints to be created
    cluster.wait_for_checkpoint(3, None).await;

    // Simulate a stale local watermark (behind the node's first available
    // checkpoint)
    let stale_epoch = 0u64; // Start with epoch 0, but node may be at a later epoch
    let grpc_url = format!("http://{}", grpc_addr);
    let remote_store: std::sync::Arc<object_store::DynObjectStore> =
        std::sync::Arc::new(InMemory::new());
    let worker = GrpcBlobWorker::new(
        remote_store.clone(),
        grpc_url.clone(),
        5 * 1024 * 1024,
        stale_epoch,
    );

    // Connect to the gRPC endpoint and get the first available checkpoint
    let mut grpc_client = GrpcNodeClient::connect(&grpc_url).await.expect("connect");
    let mut stream = grpc_client
        .stream_checkpoints(Some(0), None)
        .await
        .expect("stream");
    let mut first_checkpoint_index = None;
    let mut checkpoints = vec![];
    while let Some(Ok(checkpoint)) = stream.next().await {
        let checkpoint_data: CheckpointData =
            bcs::from_bytes(&checkpoint.data).expect("bcs decode");
        first_checkpoint_index.get_or_insert(checkpoint.index);
        checkpoints.push((
            checkpoint.index,
            checkpoint_data.checkpoint_summary.epoch,
            checkpoint_data,
        ));
        println!("Streamed checkpoint {}", checkpoint.index);
    }
    assert!(
        !checkpoints.is_empty(),
        "Should have streamed at least one checkpoint"
    );

    // Now, process all checkpoints with the worker, simulating ingestion from a
    // stale state
    for (_idx, _epoch, checkpoint_data) in &checkpoints {
        let checkpoint_data = std::sync::Arc::new(checkpoint_data.clone());
        worker
            .process_checkpoint(checkpoint_data)
            .await
            .expect("process");
    }

    // Simulate a stale state by deleting blobs 0 and 1 from the remote store
    // (as if the client lost them or they were pruned)
    for idx in 0..3 {
        let path = iota_data_ingestion::GrpcBlobWorker::file_path(idx);
        if idx <= 1 {
            let _ = remote_store.delete(&path).await;
        }
    }

    // Check that the remote store contains blobs for all processed checkpoints
    // except 0 and 1
    let mut found_blobs = 0;
    for (idx, _epoch, _data) in &checkpoints {
        let path = iota_data_ingestion::GrpcBlobWorker::file_path(*idx);
        if ObjectStoreGetExt::get_bytes(&remote_store, &path)
            .await
            .is_ok()
        {
            found_blobs += 1;
            println!("Found blob for checkpoint {} (before reset)", idx);
        }
    }
    assert_eq!(
        found_blobs,
        checkpoints.len() - 2,
        "After deletion, only blobs >= 2 should be present in the remote store"
    );

    // --- Re-run the worker to simulate reset and recovery ---
    for (_idx, _epoch, checkpoint_data) in &checkpoints {
        let checkpoint_data = std::sync::Arc::new(checkpoint_data.clone());
        worker
            .process_checkpoint(checkpoint_data)
            .await
            .expect("process after reset");
    }

    // Now, all blobs should be present again
    let mut found_blobs_after_reset = 0;
    for (idx, _epoch, _data) in &checkpoints {
        let path = iota_data_ingestion::GrpcBlobWorker::file_path(*idx);
        if ObjectStoreGetExt::get_bytes(&remote_store, &path)
            .await
            .is_ok()
        {
            found_blobs_after_reset += 1;
            println!("Found blob for checkpoint {} (after reset)", idx);
        }
    }
    assert_eq!(
        found_blobs_after_reset,
        checkpoints.len(),
        "After reset, all blobs should be present in the remote store again"
    );
}
