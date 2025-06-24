// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_data_ingestion::GrpcBlobWorker;
use iota_data_ingestion_core::Worker;
use iota_grpc_api::client::GrpcNodeClient;
use iota_types::full_checkpoint_content::CheckpointData;
use object_store::memory::InMemory;
use test_cluster::TestClusterBuilder;
use tokio_stream::StreamExt;

/// Integration test: Streams the full CheckpointData for a single checkpoint
/// (using full=true, start_index=None, end_index=Some(idx)), decodes it, and
/// passes it to the GrpcBlobWorker to verify ingestion logic.
#[tokio::test]
async fn test_grpc_blob_worker_logic() {
    // Start a test cluster with gRPC enabled
    let grpc_port = 50063u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);
    let cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .build()
        .await;

    // Wait for several checkpoints to be created
    cluster.wait_for_checkpoint(4, None).await;

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
    let mut grpc_client = GrpcNodeClient::connect(&grpc_url).await.expect("failed to connect grpc client");
    let mut stream = grpc_client
        .stream_checkpoints(None, Some(4), Some(true))
        .await
        .expect("stream");
    while let Some(Ok(checkpoint)) = stream.next().await {
        let checkpoint_data: CheckpointData =
            match GrpcNodeClient::deserialize_checkpoint(&checkpoint)
                .expect("failed to deserialize checkpoint")
            {
                iota_grpc_api::client::CheckpointContent::Data(data) => data,
                iota_grpc_api::client::CheckpointContent::Summary(_) => {
                    panic!("Expected data, got summary")
                }
            };

        println!(
            "Streamed full CheckpointData for checkpoint {}",
            checkpoint.index
        );

        // Assert the checkpoint index is 4 before processing
        assert_eq!(checkpoint.index, 4, "Should have streamed checkpoint 4");

        println!("Checkpoint data: {:?}", checkpoint_data);

        let checkpoint_data_arc = std::sync::Arc::new(checkpoint_data.clone());
        worker
            .process_checkpoint(checkpoint_data_arc)
            .await
            .expect("worker should process checkpoint without error");

        break;
    }
}
