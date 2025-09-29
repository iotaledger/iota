// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::local_ip_utils;
use iota_data_ingestion::GrpcBlobWorker;
use iota_data_ingestion_core::Worker;
use iota_grpc_api::NodeClient;
use iota_types::full_checkpoint_content::CheckpointData;
use object_store::memory::InMemory;
use test_cluster::TestClusterBuilder;
use tokio_stream::StreamExt;

/// Integration test: Streams the full CheckpointData for a single checkpoint
/// (using full=true, start_sequence_number=None,
/// end_sequence_number=Some(idx)), decodes it, and passes it to the
/// GrpcBlobWorker to verify ingestion logic.
#[tokio::test]
async fn test_grpc_blob_worker_logic() {
    // Start a test cluster with gRPC enabled
    let localhost = local_ip_utils::localhost_for_testing();
    let grpc_port = local_ip_utils::get_available_port(&localhost);
    let grpc_addr = format!("{localhost}:{grpc_port}");
    let cluster = TestClusterBuilder::new()
        .with_num_validators(1)
        .with_fullnode_grpc_api_address(grpc_addr.parse().expect("Invalid gRPC address"))
        .build()
        .await;

    // Wait for several checkpoints to be created
    cluster.wait_for_checkpoint(4, None).await;

    // Simulate a stale local watermark (behind the node's first available
    // checkpoint)
    let stale_epoch = 0u64; // Start with epoch 0, but node may be at a later epoch
    let grpc_url = format!("http://{grpc_addr}");
    let remote_store: std::sync::Arc<object_store::DynObjectStore> =
        std::sync::Arc::new(InMemory::new());
    let worker = GrpcBlobWorker::new(
        remote_store.clone(),
        grpc_url.clone(),
        5 * 1024 * 1024,
        stale_epoch,
    );

    // Connect to the gRPC endpoint and get the first available checkpoint
    let node_client = NodeClient::connect(&grpc_url)
        .await
        .expect("failed to connect grpc client");
    let mut checkpoint_client = node_client
        .checkpoint_client()
        .expect("Checkpoint client not available");
    let mut stream = checkpoint_client
        .stream_checkpoints(None, Some(4), true)
        .await
        .expect("failed to stream checkpoints");
    if let Some(Ok(checkpoint_content)) = stream.next().await {
        let checkpoint_data: CheckpointData = match checkpoint_content {
            iota_grpc_api::CheckpointContent::Data(grpc_data) => {
                grpc_data.into_v1().expect("Expected v1 checkpoint data")
            }
            iota_grpc_api::CheckpointContent::Summary(_) => {
                panic!("Expected data, got summary")
            }
        };

        println!(
            "Streamed full CheckpointData for checkpoint {}",
            checkpoint_data.checkpoint_summary.sequence_number
        );

        // Assert the checkpoint sequence number is 4 before processing
        assert_eq!(
            checkpoint_data.checkpoint_summary.sequence_number, 4,
            "Should have streamed checkpoint 4"
        );

        println!("Checkpoint data: {checkpoint_data:?}");

        let checkpoint_data_arc = std::sync::Arc::new(checkpoint_data.clone());
        worker
            .process_checkpoint(checkpoint_data_arc)
            .await
            .expect("worker should process checkpoint without error");
    }
}
