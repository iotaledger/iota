// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::{Stream, StreamExt};
use iota_grpc_types::{CertifiedCheckpointSummary, CheckpointData};
use tonic::transport::Channel;

use crate::checkpoint::checkpoint_service_client::CheckpointServiceClient;

/// Enum representing the content of a checkpoint, either full data or summary.
#[derive(Debug, Clone)]
pub enum CheckpointContent {
    Data(CheckpointData),
    Summary(CertifiedCheckpointSummary),
}

/// Dedicated client for checkpoint-related gRPC operations.
///
/// This client handles all checkpoint service interactions including streaming
/// checkpoints and querying epoch information.
#[derive(Clone)]
pub struct CheckpointClient {
    client: CheckpointServiceClient<Channel>,
}

impl CheckpointClient {
    /// Create a new CheckpointClient from a shared gRPC channel.
    pub(super) fn new(channel: Channel) -> Self {
        Self {
            client: CheckpointServiceClient::new(channel),
        }
    }

    /// Stream checkpoints with automatic deserialization.
    ///
    /// # Arguments
    /// * `start_sequence_number` - Optional starting sequence number
    /// * `end_sequence_number` - Optional ending sequence number
    /// * `full` - Whether to stream full checkpoint data or just summaries
    ///
    /// # Returns
    /// A stream of checkpoint content (either data or summaries)
    pub async fn stream_checkpoints(
        &mut self,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        full: bool,
    ) -> Result<impl Stream<Item = Result<CheckpointContent, tonic::Status>>, tonic::Status> {
        let request = crate::checkpoint::CheckpointStreamRequest {
            start_sequence_number,
            end_sequence_number,
            full,
        };
        let stream = self.client.stream_checkpoints(request).await?.into_inner();

        Ok(stream.map(|result| {
            result.and_then(|checkpoint| {
                Self::deserialize_checkpoint(&checkpoint).map_err(|e| {
                    tonic::Status::internal(format!("Failed to deserialize checkpoint: {e}"))
                })
            })
        }))
    }

    /// Get the first checkpoint sequence number for a given epoch.
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

    // ========================================
    // Private Helper Methods
    // ========================================

    /// Deserialize checkpoint data based on the checkpoint type (full or
    /// summary). Returns either checkpoint data or summary depending on the
    /// checkpoint type.
    fn deserialize_checkpoint(
        checkpoint: &crate::checkpoint::Checkpoint,
    ) -> anyhow::Result<CheckpointContent> {
        let bcs_data = checkpoint
            .bcs_data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing BCS data in checkpoint"))?;

        if checkpoint.is_full {
            let checkpoint_data = bcs_data.deserialize_into::<CheckpointData>()?;
            Ok(CheckpointContent::Data(checkpoint_data))
        } else {
            let checkpoint_summary = bcs_data.deserialize_into::<CertifiedCheckpointSummary>()?;
            Ok(CheckpointContent::Summary(checkpoint_summary))
        }
    }
}
