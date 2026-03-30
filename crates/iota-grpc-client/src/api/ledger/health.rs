// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for health check queries.

use iota_grpc_types::v1::ledger_service::{GetHealthRequest, GetHealthResponse};

use crate::{
    Client,
    api::{MetadataEnvelope, Result},
};

impl Client {
    /// Check the health of the node.
    ///
    /// Returns a [`MetadataEnvelope`]`<`[`GetHealthResponse`]`>` with the
    /// latest checkpoint sequence number and an estimated validator latency
    /// field (reserved for future use).
    ///
    /// If the node's latest checkpoint is stale (beyond the threshold), the
    /// server returns an `UNAVAILABLE` error.
    ///
    /// # Parameters
    ///
    /// - `threshold_ms` - Optional threshold in milliseconds. If provided, the
    ///   node is considered healthy only if the latest executed checkpoint
    ///   timestamp is within this many milliseconds of the current system time.
    ///   If `None`, the server applies its default threshold (5 seconds).
    pub async fn get_health(
        &self,
        threshold_ms: Option<u64>,
    ) -> Result<MetadataEnvelope<GetHealthResponse>> {
        let mut request = GetHealthRequest::default();
        if let Some(ms) = threshold_ms {
            request = request.with_threshold_ms(ms);
        }

        let mut client = self.ledger_service_client();
        let response = client.get_health(request).await?;

        Ok(MetadataEnvelope::from(response))
    }
}
