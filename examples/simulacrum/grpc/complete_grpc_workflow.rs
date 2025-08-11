// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Complete gRPC workflow example using Simulacrum
//!
//! This example demonstrates how to:
//! 1. Create a simulacrum instance with transactions and checkpoints
//! 2. Start a gRPC server from simulacrum
//! 3. Connect to the server using NodeClient
//! 4. Stream checkpoints via gRPC client
//! 5. Test epoch boundary functionality

use std::sync::Arc;

use iota_grpc_api::{
    Config,
    client::{CheckpointContent, NodeClient},
};
use iota_types::base_types::IotaAddress;
use simulacrum::Simulacrum;
use tokio_stream::StreamExt;

/// Creates and fills a simulacrum instance with dummy transactions and
/// checkpoints
fn fill_simulacrum_with_dummy_data(simulacrum: &mut Simulacrum) -> anyhow::Result<()> {
    let recipient1 = IotaAddress::random_for_testing_only();
    let recipient2 = IotaAddress::random_for_testing_only();

    // Execute some transactions
    let (tx1, _) = simulacrum.transfer_txn(recipient1);
    simulacrum.execute_transaction(tx1)?;

    // Advance time and create checkpoint
    simulacrum.advance_clock(std::time::Duration::from_secs(1));
    let checkpoint1 = simulacrum.create_checkpoint();
    println!(
        "Created checkpoint 1: seq={}",
        checkpoint1.sequence_number()
    );

    // More activity
    let (tx2, _) = simulacrum.transfer_txn(recipient2);
    simulacrum.execute_transaction(tx2)?;

    // Advance time and create another checkpoint
    simulacrum.advance_clock(std::time::Duration::from_secs(2));
    let checkpoint2 = simulacrum.create_checkpoint();
    println!(
        "Created checkpoint 2: seq={}",
        checkpoint2.sequence_number()
    );

    // Create one more checkpoint with time advancement
    simulacrum.advance_clock(std::time::Duration::from_secs(1));
    let checkpoint3 = simulacrum.create_checkpoint();
    println!(
        "Created checkpoint 3: seq={}",
        checkpoint3.sequence_number()
    );

    Ok(())
}

/// Sets up and runs the complete gRPC workflow with simulacrum
async fn run_simulacrum_with_grpc() -> anyhow::Result<()> {
    // Initialize the simulacrum and fill it with dummy data
    let mut simulacrum = Simulacrum::new();
    fill_simulacrum_with_dummy_data(&mut simulacrum)?;

    // Wrap simulacrum for sharing and start gRPC server
    let simulacrum = Arc::new(simulacrum);

    let grpc_config = Config::default();
    let server_address = grpc_config.address;

    println!("Starting gRPC server on: {}", server_address);
    let _server_handle = simulacrum.clone().start_grpc_server(grpc_config).await?;

    // Give the server a moment to start up
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Connect to the gRPC server using NodeClient
    let server_url = format!("http://{}", server_address);
    println!("Connecting to gRPC server at: {server_url}");

    let node_client = NodeClient::connect(&server_url).await?;
    let mut checkpoint_client = node_client
        .checkpoint_client()
        .expect("Checkpoint client should be available");

    // Stream full checkpoint data for specific range
    println!("\n=== Streaming full checkpoint data ===");
    let mut stream = checkpoint_client
        .stream_checkpoints(Some(1), Some(3), true)
        .await?;

    while let Some(result) = StreamExt::next(&mut stream).await {
        match result {
            Ok(CheckpointContent::Data(data)) => match data {
                iota_grpc_types::CheckpointData::V1(v1_data) => {
                    println!(
                        "Full Data - Seq: {}, Epoch: {}, Transaction count: {}",
                        v1_data.checkpoint_summary.sequence_number,
                        v1_data.checkpoint_summary.epoch,
                        v1_data.transactions.len()
                    );
                }
            },
            Ok(CheckpointContent::Summary(_)) => {
                println!("Unexpected summary content when requesting data");
            }
            Err(e) => {
                println!("Stream error: {e:?}");
                break;
            }
        }
    }

    // Test epoch functionality
    println!("\n=== Testing epoch functionality ===");
    match checkpoint_client
        .get_epoch_first_checkpoint_sequence_number(0)
        .await
    {
        Ok(first_seq) => {
            println!("First checkpoint of epoch 0: {first_seq}");
        }
        Err(e) => {
            println!("Epoch request error: {e:?}");
        }
    }

    println!("\n=== Demo complete ===");
    println!("This example showed how to:");
    println!("  1. Start a simulacrum with gRPC server");
    println!("  2. Connect using NodeClient");
    println!("  3. Stream checkpoint data");
    println!("  4. Query epoch information");

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_simulacrum_with_grpc().await
}
