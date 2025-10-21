// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::{common as grpc_common, write as grpc_write};
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
    ) -> Result<grpc_common::TransactionResponse, tonic::Status> {
        let response = self.client.execute_transaction(request).await?;
        Ok(response.into_inner())
    }

    /// Dev inspect a transaction and return the response.
    pub async fn dev_inspect_transaction(
        &mut self,
        request: grpc_write::DevInspectTransactionRequest,
    ) -> Result<grpc_write::DevInspectTransactionResponse, tonic::Status> {
        let response = self.client.dev_inspect_transaction(request).await?;
        Ok(response.into_inner())
    }

    /// Dry run a transaction and return the response.
    pub async fn dry_run_transaction(
        &mut self,
        tx_bytes: Vec<u8>,
    ) -> Result<grpc_write::DryRunTransactionResponse, tonic::Status> {
        let request = grpc_write::DryRunTransactionRequest {
            tx_bytes: Some(grpc_common::BcsData { data: tx_bytes }),
        };
        let response = self.client.dry_run_transaction(request).await?;
        Ok(response.into_inner())
    }
}
