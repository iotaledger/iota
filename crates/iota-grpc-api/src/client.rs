// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

tonic::include_proto!("iota.grpc");

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
    ) -> Result<tonic::Streaming<crate::checkpoint::Checkpoint>, tonic::Status> {
        let request = crate::checkpoint::StreamRequest {
            start_index: start,
            end_index: end,
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
}
