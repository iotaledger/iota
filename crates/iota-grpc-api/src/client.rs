// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

tonic::include_proto!("iota.grpc");

use iota_types::grpc::{CertifiedCheckpointSummary, CheckpointData};
use tonic::transport::Channel;

use crate::{BcsConvertible, checkpoint::checkpoint_service_client::CheckpointServiceClient};

/// Enum representing the content of a checkpoint, either full data or summary.
pub enum CheckpointContent {
    Data(iota_types::full_checkpoint_content::CheckpointData),
    Summary(iota_types::messages_checkpoint::CertifiedCheckpointSummary),
}
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
        let request = crate::checkpoint::CheckpointStreamRequest {
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

    /// Auto-deserialize checkpoint based on the is_full field.
    /// Returns either checkpoint data or summary depending on the checkpoint
    /// type.
    pub fn deserialize_checkpoint(
        checkpoint: &crate::checkpoint::Checkpoint,
    ) -> Result<CheckpointContent, Box<dyn std::error::Error + Send + Sync>> {
        let bcs_data = checkpoint
            .bcs_data
            .as_ref()
            .ok_or("Missing BCS data in checkpoint")?;

        if checkpoint.is_full {
            let checkpoint_data = CheckpointData::from_bcs(&bcs_data.data)?
                .into_v1()
                .ok_or("Unsupported checkpoint data version")?;
            Ok(CheckpointContent::Data(checkpoint_data))
        } else {
            let checkpoint_summary = CertifiedCheckpointSummary::from_bcs(&bcs_data.data)?
                .into_v1()
                .ok_or("Unsupported checkpoint summary version")?;
            Ok(CheckpointContent::Summary(checkpoint_summary))
        }
    }
}
