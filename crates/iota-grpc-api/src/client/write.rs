// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::write as grpc_write;
use tonic::transport::Channel;

/// Dedicated client for write-related gRPC operations.
#[derive(Clone)]
pub struct WriteClient {
    client: grpc_write::write_service_client::WriteServiceClient<Channel>,
}

impl WriteClient {
    /// Create a new WriteClient from a shared gRPC channel.
    pub(super) fn new(channel: Channel) -> Self {
        Self {
            client: grpc_write::write_service_client::WriteServiceClient::new(channel),
        }
    }

    /// Execute a transaction and return the gRPC response.
    pub async fn execute_transaction(
        &mut self,
        request: grpc_write::ExecuteTransactionRequest,
    ) -> Result<grpc_write::ExecuteTransactionResponse, tonic::Status> {
        let response = self.client.execute_transaction(request).await?;
        Ok(response.into_inner())
    }
}
