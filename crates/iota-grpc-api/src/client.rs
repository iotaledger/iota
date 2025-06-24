// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

tonic::include_proto!("iota.grpc");

use iota_types::grpc::{CertifiedCheckpointSummary, CheckpointData};
use tonic::transport::Channel;

use crate::checkpoint::checkpoint_service_client::CheckpointServiceClient;

/// Shared gRPC client for checkpoint streaming.
pub struct GrpcNodeClient {
    client: CheckpointServiceClient<Channel>,
}

impl GrpcNodeClient {
    pub async fn connect(url: &str) -> Result<Self, tonic::transport::Error> {
        let client = CheckpointServiceClient::connect(url.to_string()).await?;
        Ok(Self { client })
    }

    /// Stream checkpoints with any combination of start and end indices.
    pub async fn stream_checkpoints(
        &mut self,
        start: Option<u64>,
        end: Option<u64>,
        full: Option<bool>,
    ) -> Result<tonic::Streaming<crate::checkpoint::Checkpoint>, tonic::Status> {
        let request = crate::checkpoint::StreamRequest {
            start_index: start,
            end_index: end,
            full,
        };
        let response = self.client.stream_checkpoints(request).await?;
        Ok(response.into_inner())
    }

    pub async fn get_epoch_first_checkpoint_sequence_number(
        &mut self,
        epoch: u64,
    ) -> Result<u64, tonic::Status> {
        let request = crate::checkpoint::EpochRequest { epoch };
        let response = self
            .client
            .get_epoch_first_checkpoint_sequence_number(request)
            .await?;
        Ok(response.into_inner().sequence_number)
    }

    /// Deserialize checkpoint data from the gRPC stream, handling versioned
    /// types
    pub fn deserialize_checkpoint_data(
        checkpoint: &crate::checkpoint::Checkpoint,
    ) -> Result<
        iota_types::full_checkpoint_content::CheckpointData,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        // First try to deserialize as versioned data
        match bcs::from_bytes::<CheckpointData>(&checkpoint.data) {
            Ok(versioned) => versioned
                .into_v1()
                .ok_or_else(|| "Unsupported checkpoint data version".into()),
            Err(_) => {
                // Fallback: try direct deserialization for backward compatibility
                bcs::from_bytes::<iota_types::full_checkpoint_content::CheckpointData>(
                    &checkpoint.data,
                )
                .map_err(|e| format!("Failed to deserialize checkpoint data: {}", e).into())
            }
        }
    }

    /// Deserialize checkpoint summary from the gRPC stream, handling versioned
    /// types
    pub fn deserialize_checkpoint_summary(
        checkpoint: &crate::checkpoint::Checkpoint,
    ) -> Result<
        iota_types::messages_checkpoint::CertifiedCheckpointSummary,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        // First try to deserialize as versioned summary
        match bcs::from_bytes::<CertifiedCheckpointSummary>(&checkpoint.data) {
            Ok(versioned) => versioned
                .into_v1()
                .ok_or_else(|| "Unsupported checkpoint summary version".into()),
            Err(_) => {
                // Fallback: try direct deserialization for backward compatibility
                bcs::from_bytes::<iota_types::messages_checkpoint::CertifiedCheckpointSummary>(
                    &checkpoint.data,
                )
                .map_err(|e| format!("Failed to deserialize checkpoint summary: {}", e).into())
            }
        }
    }
}
