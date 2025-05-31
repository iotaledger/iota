// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_api::checkpoint::{
    StreamRequest, checkpoint_service_client::CheckpointServiceClient,
};
use test_cluster::TestClusterBuilder;
use tonic::Request;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_stream_checkpoints() {
    // Pick a port for gRPC
    let grpc_port = 50055u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);

    // Start a test cluster with gRPC enabled
    let cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .build()
        .await;
    println!("[DEBUG] Cluster built");
    // Wait for checkpoint 2 to be available
    println!("[DEBUG] Waiting for checkpoint 2");
    cluster.wait_for_checkpoint(2, None).await;
    println!("[DEBUG] Checkpoint 2 available");
    println!("Test cluster started with gRPC at {}", grpc_addr);

    // Wait a moment for the node to be up
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    println!("[DEBUG] Connecting to gRPC at {}", grpc_addr);
    let mut client = CheckpointServiceClient::connect(format!("http://{}", grpc_addr))
        .await
        .expect("connect gRPC");
    println!("[DEBUG] Connected to gRPC!");

    // Request all checkpoints
    println!("[DEBUG] Sending gRPC stream request");
    let request = Request::new(StreamRequest {
        start_index: None,
        end_index: None,
    });
    let mut stream = client
        .stream_checkpoints(request)
        .await
        .expect("gRPC call")
        .into_inner();
    let mut indices = vec![];
    let mut count = 0;
    println!("[DEBUG] Starting to stream checkpoints");
    while let Some(Ok(cp)) = stream.message().await.transpose() {
        println!("[DEBUG] Received checkpoint: {:?}", cp);
        indices.push(cp.index);
        count += 1;
    }
    if count == 0 {
        println!("[DEBUG] No checkpoints were streamed!");
    }
    // There should be at least one checkpoint (genesis)
    assert!(!indices.is_empty(), "Should stream at least one checkpoint");
}
