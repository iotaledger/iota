// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_api::client::GrpcNodeClient;
use test_cluster::TestClusterBuilder;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_grpc_checkpoint_stream() {
    // Pick a port for gRPC
    let grpc_port = 50055u16;
    let grpc_addr = format!("127.0.0.1:{}", grpc_port);

    // Start a test cluster with gRPC enabled
    let _cluster = TestClusterBuilder::new()
        .with_fullnode_grpc_api_address(grpc_addr.clone())
        .build()
        .await;

    // Connect to the gRPC endpoint
    let grpc_url = format!("http://{}", grpc_addr);
    let mut client = GrpcNodeClient::connect(&grpc_url).await.expect("connect");

    let mut stream = client
        .stream_checkpoints(0, Some(10))
        .await
        .expect("stream");
    let mut count = 0;
    while let Some(Ok(_checkpoint)) = stream.next().await {
        count += 1;
    }
    assert!(count > 0, "Should receive at least one checkpoint");
}
