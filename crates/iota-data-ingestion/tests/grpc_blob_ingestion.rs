// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use bcs;
use iota_data_ingestion::GrpcBlobWorker;
use iota_data_ingestion_core::Worker;
use iota_grpc_api::client::GrpcNodeClient;
use iota_types::full_checkpoint_content::CheckpointData;
use object_store::memory::InMemory;
use test_cluster::TestClusterBuilder;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_grpc_blob_ingestion() {
    // Pick a port for gRPC
    let grpc_port = 50056u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);

    // Start a test cluster with gRPC enabled
    let _cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .build()
        .await;

    // Connect to the gRPC endpoint
    let grpc_url = format!("http://{}", grpc_addr);
    let mut client = GrpcNodeClient::connect(&grpc_url).await.expect("connect");

    // Use in-memory object store for test
    let remote_store = std::sync::Arc::new(InMemory::new());
    let worker = GrpcBlobWorker::new(remote_store, grpc_url.clone(), 5 * 1024 * 1024, 0);

    // Stream a few checkpoints and process them
    let mut stream = client.stream_checkpoints(0, Some(5)).await.expect("stream");
    let mut count = 0;
    while let Some(Ok(checkpoint)) = stream.next().await {
        // Decode the BCS-encoded CheckpointData from the gRPC Checkpoint
        let checkpoint_data: CheckpointData =
            bcs::from_bytes(&checkpoint.data).expect("bcs decode");
        let checkpoint_data = std::sync::Arc::new(checkpoint_data);
        worker
            .process_checkpoint(checkpoint_data)
            .await
            .expect("process");
        count += 1;
    }
    assert!(count > 0, "Should process at least one checkpoint");
}
