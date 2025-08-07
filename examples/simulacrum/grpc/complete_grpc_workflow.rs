// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Complete gRPC workflow example using Simulacrum
//!
//! This example demonstrates how to:
//! 1. Create a simulacrum instance with transactions
//! 2. Implement GrpcStateReader for simulacrum
//! 3. Set up a gRPC service using the trait abstraction
//! 4. Stream checkpoints via gRPC
//! 5. Test epoch boundary functionality

use std::sync::Arc;

use iota_grpc_api::{
    CheckpointGrpcService, GrpcReader, GrpcStateReader,
    checkpoint::{CheckpointStreamRequest, checkpoint_service_server::CheckpointService},
};
use iota_grpc_types::CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary;
use iota_types::{
    base_types::IotaAddress, full_checkpoint_content::CheckpointData,
    messages_checkpoint::CertifiedCheckpointSummary,
};
use simulacrum::Simulacrum;
use tokio_stream::StreamExt;
use tonic::Request;

/// Implementation of GrpcStateReader for Simulacrum
/// This demonstrates how to implement the storage abstraction trait
/// to work with simulacrum instead of requiring a full database
pub struct SimulacrumGrpcReader {
    simulacrum: Arc<Simulacrum>,
}

impl SimulacrumGrpcReader {
    pub fn new(simulacrum: Arc<Simulacrum>) -> Self {
        Self { simulacrum }
    }
}

impl GrpcStateReader for SimulacrumGrpcReader {
    fn get_latest_checkpoint_sequence(&self) -> Option<u64> {
        self.simulacrum
            .store()
            .get_highest_checkpoint()
            .map(|checkpoint| *checkpoint.sequence_number())
    }

    fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary> {
        self.simulacrum
            .store()
            .get_checkpoint_by_sequence_number(seq)
            .map(CertifiedCheckpointSummary::from)
    }

    fn get_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let checkpoint = self
            .simulacrum
            .store()
            .get_checkpoint_by_sequence_number(seq)?;
        let contents = self
            .simulacrum
            .store()
            .get_checkpoint_contents(&checkpoint.content_digest)?;

        Some(CheckpointData {
            checkpoint_summary: CertifiedCheckpointSummary::from(checkpoint),
            checkpoint_contents: contents,
            transactions: vec![],
        })
    }

    fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        // Simple implementation for simulacrum - find the last checkpoint of the given
        // epoch
        let latest_seq = self.get_latest_checkpoint_sequence().unwrap_or(0);

        for seq in (0..=latest_seq).rev() {
            if let Some(summary) = self.get_checkpoint_summary(seq) {
                if summary.epoch() == epoch {
                    return Ok(Some(summary));
                }
            }
        }
        Ok(None)
    }
}

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

    // Wrap simulacrum for sharing
    let simulacrum = Arc::new(simulacrum);

    // Create the gRPC reader using our trait implementation
    let simulacrum_reader = SimulacrumGrpcReader::new(simulacrum.clone());
    let grpc_reader = Arc::new(GrpcReader::new(Arc::new(simulacrum_reader)));

    // Verify the reader works
    let latest_seq = grpc_reader.get_latest_checkpoint_sequence().unwrap();
    println!("Latest checkpoint sequence: {}", latest_seq);

    // Create broadcast channels for the gRPC service
    let (checkpoint_summary_tx, _) = tokio::sync::broadcast::channel(100);
    let (checkpoint_data_tx, _) = tokio::sync::broadcast::channel(100);

    // Create the gRPC service using our trait abstraction
    let service = CheckpointGrpcService::new(
        grpc_reader,
        checkpoint_summary_tx.clone(),
        checkpoint_data_tx.clone(),
    );

    // Example 1: Stream checkpoint summaries
    println!("\n=== Streaming checkpoint summaries ===");
    let request = CheckpointStreamRequest {
        start_sequence_number: Some(0),
        end_sequence_number: Some(latest_seq),
        full: false,
    };

    let mut stream = service
        .stream_checkpoints(Request::new(request))
        .await?
        .into_inner();

    while let Some(result) = StreamExt::next(&mut stream).await {
        match result {
            Ok(checkpoint) => {
                println!(
                    "Summary - Seq: {}, Full: {}",
                    checkpoint.sequence_number, checkpoint.is_full
                );
            }
            Err(e) => {
                println!("Stream error: {:?}", e);
                break;
            }
        }
    }

    // Example 2: Stream full checkpoint data
    println!("\n=== Streaming full checkpoint data ===");
    let request = CheckpointStreamRequest {
        start_sequence_number: Some(1),
        end_sequence_number: Some(2),
        full: true,
    };

    let mut stream = service
        .stream_checkpoints(Request::new(request))
        .await?
        .into_inner();

    while let Some(result) = StreamExt::next(&mut stream).await {
        match result {
            Ok(checkpoint) => {
                println!(
                    "Full Data - Seq: {}, Full: {}, Has BCS data: {}",
                    checkpoint.sequence_number,
                    checkpoint.is_full,
                    checkpoint.bcs_data.is_some()
                );
            }
            Err(e) => {
                println!("Stream error: {:?}", e);
                break;
            }
        }
    }

    // Example 3: Test epoch functionality
    println!("\n=== Testing epoch functionality ===");
    let epoch_request = iota_grpc_api::checkpoint::EpochRequest { epoch: 0 };

    match service
        .get_epoch_first_checkpoint_sequence_number(Request::new(epoch_request))
        .await
    {
        Ok(response) => {
            println!(
                "First checkpoint of epoch 0: {}",
                response.into_inner().sequence_number
            );
        }
        Err(e) => {
            println!("Epoch request error: {:?}", e);
        }
    }

    // Example 4: Demonstrate live streaming (simulated)
    println!("\n=== Simulating live streaming ===");

    tokio::spawn({
        let _simulacrum = simulacrum.clone();
        let checkpoint_summary_tx = checkpoint_summary_tx.clone();
        let _checkpoint_data_tx = checkpoint_data_tx.clone();

        async move {
            // Create mock checkpoints with correct incrementing sequence numbers
            for i in 1..=15 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                // Create a mock checkpoint with the expected sequence number
                let expected_seq = latest_seq + i;

                // Create a simple mock checkpoint summary
                use iota_types::{
                    crypto::AuthorityStrongQuorumSignInfo,
                    messages_checkpoint::{
                        CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary,
                    },
                };

                // Create a mock checkpoint contents digest
                let mock_contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);

                let summary = CheckpointSummary {
                    epoch: 0,
                    sequence_number: expected_seq,
                    network_total_transactions: 0,
                    content_digest: *mock_contents.digest(),
                    previous_digest: None,
                    epoch_rolling_gas_cost_summary: Default::default(),
                    timestamp_ms: 1000 + (i * 1000), // Increment timestamp
                    checkpoint_commitments: vec![],
                    end_of_epoch_data: None,
                    version_specific_data: vec![],
                };

                // Create certified checkpoint summary
                let sig = AuthorityStrongQuorumSignInfo {
                    epoch: 0,
                    signature: Default::default(),
                    signers_map: Default::default(),
                };
                let certified_summary =
                    CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig);
                let grpc_summary = GrpcCertifiedCheckpointSummary::from(certified_summary);

                let _ = checkpoint_summary_tx.send(Arc::new(grpc_summary));

                println!(
                    "Sent live checkpoint summary #{} (seq: {})",
                    i, expected_seq
                );

                if i >= 15 {
                    break;
                }
            }
        }
    });

    // Give the background task a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Example 5: Stream with no end (would normally wait for live data)
    println!("\n=== Testing open-ended stream (limited for demo) ===");
    let request = CheckpointStreamRequest {
        start_sequence_number: Some(latest_seq + 1), // Start from next checkpoint
        end_sequence_number: None,                   // Open-ended
        full: false,
    };

    let mut stream = service
        .stream_checkpoints(Request::new(request))
        .await?
        .into_inner();

    // Stop after receiving 10 live checkpoints
    let mut count = 0;
    while let Some(result) = StreamExt::next(&mut stream).await {
        match result {
            Ok(checkpoint) => {
                println!(
                    "Live stream - Seq: {}, Full: {}",
                    checkpoint.sequence_number, checkpoint.is_full
                );
                count += 1;
                if count >= 10 {
                    println!("Received 10 live checkpoints, stopping stream");
                    break;
                }
            }
            Err(e) => {
                println!("Live stream error: {:?}", e);
                break;
            }
        }
    }

    println!("\n=== Demo complete ===");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_simulacrum_with_grpc().await
}
