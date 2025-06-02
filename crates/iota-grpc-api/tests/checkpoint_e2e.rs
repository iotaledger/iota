// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::StreamExt;
use iota_grpc_api::{
    checkpoint::{StreamRequest, checkpoint_service_client::CheckpointServiceClient},
    client::GrpcNodeClient,
};
use test_cluster::TestClusterBuilder;
use tonic::Request;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_stream_checkpoints() {
    // Pick a port for gRPC
    let grpc_port = 50055u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);

    // Start a test cluster with gRPC enabled and pruning disabled
    let cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .build()
        .await;

    // Wait for checkpoint 2 to be available
    println!("Waiting for checkpoint 2");
    cluster.wait_for_checkpoint(2, None).await;

    println!("Connecting to gRPC at {}", grpc_addr);
    let mut client = CheckpointServiceClient::connect(format!("http://{}", grpc_addr))
        .await
        .expect("connect gRPC");
    println!("Connected to gRPC!");

    // Request all checkpoints
    println!("Sending gRPC stream request");
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
    println!("Starting to stream checkpoints");
    while let Some(Ok(cp)) = stream.next().await {
        let data: iota_types::full_checkpoint_content::CheckpointData =
            bcs::from_bytes(&cp.data).unwrap();
        println!(
            "Received checkpoint: index={}, epoch={}",
            cp.index, data.checkpoint_summary.epoch
        );
        indices.push(cp.index);
    }
    if indices.is_empty() {
        println!("No checkpoints were streamed!");
    }
    // There should be at least two checkpoints (genesis and at least one more)
    assert!(indices.len() >= 2, "Should stream at least two checkpoints");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_epoch_first_checkpoint_sequence_number() {
    // Pick a port for gRPC
    let grpc_port = 50058u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);

    // Start a test cluster with gRPC enabled and pruning disabled
    let cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .build()
        .await;

    let sender = cluster.get_address_0();
    let receiver = cluster.get_address_1();

    // Wait for 2 new checkpoint to be available
    cluster.wait_for_checkpoint(2, None).await;
    println!("Checkpoint 2 available, forcing new epoch");

    // Advance to a new epoch
    cluster.force_new_epoch().await;
    cluster.transfer_iota_must_exceed(sender, receiver, 1).await;
    let current_epoch = cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch());
    println!("Current epoch after force_new_epoch: {}", current_epoch);

    // Wait for 3 new checkpoints in the new epoch
    cluster.wait_for_checkpoint(3, None).await;
    cluster.force_new_epoch().await;
    cluster.transfer_iota_must_exceed(sender, receiver, 1).await;
    let current_epoch = cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state().epoch_store_for_testing().epoch());
    println!("Current epoch after force_new_epoch: {}", current_epoch);

    // Connect to the gRPC endpoint
    let mut client = GrpcNodeClient::connect(&format!("http://{}", grpc_addr))
        .await
        .expect("connect gRPC");

    // List all checkpoints and their epochs using the gRPC stream
    println!("Listing all checkpoints and their epochs via gRPC stream");
    let mut stream = client
        .stream_checkpoints(0, None)
        .await
        .expect("gRPC stream");
    let mut all_indices = vec![];
    let mut all_epochs = vec![];
    while let Some(Ok(cp)) = stream.next().await {
        let data: iota_types::full_checkpoint_content::CheckpointData =
            bcs::from_bytes(&cp.data).unwrap();
        let epoch = data.checkpoint_summary.epoch;
        println!("Checkpoint index: {}, epoch: {}", cp.index, epoch);
        all_indices.push(cp.index);
        all_epochs.push(epoch);
    }

    // Query for the first checkpoint of epoch 0 (should be 0)
    let first_0 = client
        .get_epoch_first_checkpoint_sequence_number(0)
        .await
        .expect("gRPC call");
    println!("First checkpoint of epoch 0: {}", first_0);
    assert_eq!(first_0, 0, "First checkpoint of epoch 0 should be 0");

    // Query for the first checkpoint of epoch 1 (should be >= 2)
    let first_1 = client
        .get_epoch_first_checkpoint_sequence_number(1)
        .await
        .expect("gRPC call");
    println!("First checkpoint of epoch 1: {}", first_1);
    assert!(
        first_1 >= 2,
        "First checkpoint of epoch 1 should be >= 2, got {}",
        first_1
    );
}
