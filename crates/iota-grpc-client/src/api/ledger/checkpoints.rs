// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for checkpoint queries.

use iota_grpc_types::{
    field::FieldMask,
    v0::ledger_service::{GetCheckpointDataRequest, get_checkpoint_data_request},
};
use iota_sdk_types::{CheckpointSequenceNumber, Digest, SignedCheckpointSummary};

use crate::{
    Client,
    api::{Result, TryFromProtoError},
};

impl Client {
    /// Get the latest checkpoint.
    ///
    /// Note: If you only need the latest checkpoint sequence number (not the
    /// full checkpoint data), use [`crate::ResponseExt::checkpoint_height()`]
    /// on any gRPC response instead.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let latest = client.get_latest_checkpoint().await?;
    /// println!("Latest checkpoint: {}", latest.checkpoint.sequence_number);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_latest_checkpoint(&self) -> Result<SignedCheckpointSummary> {
        self.get_checkpoint_internal(get_checkpoint_data_request::CheckpointId::Latest(true))
            .await
    }

    /// Get a checkpoint by sequence number.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let checkpoint = client.get_checkpoint(100).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_checkpoint(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<SignedCheckpointSummary> {
        self.get_checkpoint_internal(get_checkpoint_data_request::CheckpointId::SequenceNumber(
            sequence_number,
        ))
        .await
    }

    /// Get a checkpoint by digest.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::Digest;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let digest: Digest = todo!();
    /// let checkpoint = client.get_checkpoint_by_digest(&digest).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_checkpoint_by_digest(
        &self,
        digest: &Digest,
    ) -> Result<SignedCheckpointSummary> {
        self.get_checkpoint_internal(get_checkpoint_data_request::CheckpointId::Digest(
            (*digest).into(),
        ))
        .await
    }

    /// Internal helper to fetch a checkpoint by any ID type.
    async fn get_checkpoint_internal(
        &self,
        checkpoint_id: get_checkpoint_data_request::CheckpointId,
    ) -> Result<SignedCheckpointSummary> {
        let request = GetCheckpointDataRequest {
            checkpoint_id: Some(checkpoint_id),
            checkpoint_read_mask: Some(FieldMask {
                paths: vec!["summary.bcs".to_string()],
            }),
            transactions_filter: None,
            transaction_read_mask: None,
            events_filter: None,
            event_read_mask: None,
            max_message_size_bytes: self.max_decoding_message_size().map(|s| s as u32),
        };

        let mut client = self.ledger_service_client();

        let mut stream = client.get_checkpoint_data(request).await?.into_inner();

        // The stream may contain multiple message types (checkpoint, transactions,
        // events). Iterate to find the checkpoint payload and return early once
        // found.
        while let Some(data) = stream.message().await? {
            let Some(iota_grpc_types::v0::ledger_service::checkpoint_data::Payload::Checkpoint(
                checkpoint,
            )) = data.payload
            else {
                continue;
            };

            let summary_bcs = checkpoint
                .summary
                .as_ref()
                .and_then(|s| s.bcs.as_ref())
                .ok_or(TryFromProtoError::missing("summary.bcs"))?;

            return summary_bcs
                .deserialize()
                .map_err(|e| TryFromProtoError::invalid("summary.bcs", e).into());
        }

        Err(TryFromProtoError::missing("checkpoint").into())
    }
}
