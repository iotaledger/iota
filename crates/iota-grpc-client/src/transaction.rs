// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use futures::{Stream, StreamExt};
use iota_grpc_types::v0::transactions as grpc_transactions;
use iota_types::effects::TransactionEffects;
use tonic::transport::Channel;

/// Dedicated client for transaction-related gRPC operations.
///
/// This client handles all transaction service interactions including streaming
/// transactions with filtering capabilities.
#[derive(Clone)]
pub struct TransactionClient {
    client: grpc_transactions::transaction_service_client::TransactionServiceClient<Channel>,
}

impl TransactionClient {
    /// Create a new TransactionClient from a shared gRPC channel.
    pub(super) fn new(channel: Channel) -> Self {
        Self {
            client: grpc_transactions::transaction_service_client::TransactionServiceClient::new(
                channel,
            ),
        }
    }

    /// Stream transaction effects with automatic BCS deserialization and
    /// filtering.
    ///
    /// # Arguments
    /// * `filter` - Transaction filter to apply to the stream
    ///
    /// # Returns
    /// A stream of IOTA transaction effects that match the specified filter
    pub async fn stream_transactions(
        &mut self,
        filter: grpc_transactions::TransactionFilter,
    ) -> Result<impl Stream<Item = Result<TransactionEffects, tonic::Status>>, tonic::Status> {
        let request = grpc_transactions::TransactionStreamRequest {
            filter: Some(filter),
        };
        let response = self.client.stream_transactions(request).await?;
        let stream = response.into_inner();

        // Transform the stream to deserialize Transaction protobuf messages
        // into native TransactionEffects
        Ok(stream.map(|transaction_result| {
            transaction_result.and_then(|transaction| {
                Self::deserialize_transaction(&transaction).map_err(|e| {
                    tonic::Status::internal(format!("Failed to deserialize transaction: {e}"))
                })
            })
        }))
    }

    /// Deserialize transaction effects from BCS-encoded data.
    fn deserialize_transaction(
        tx: &grpc_transactions::Transaction,
    ) -> anyhow::Result<TransactionEffects> {
        let bcs_data = tx
            .bcs_data
            .as_ref()
            .ok_or_else(|| anyhow!("Transaction missing BCS data"))?;

        // Deserialize directly as core TransactionEffects - no conversion needed!
        bcs::from_bytes(&bcs_data.data)
            .map_err(|e| anyhow!("Failed to deserialize TransactionEffects from BCS: {e}"))
    }
}
