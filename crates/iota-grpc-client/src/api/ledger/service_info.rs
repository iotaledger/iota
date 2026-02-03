// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for service info queries.

use iota_grpc_types::v0::ledger_service::{GetServiceInfoRequest, GetServiceInfoResponse};

use crate::{
    Client,
    api::{Result, SERVICE_INFO_READ_MASK, field_mask_with_default},
};

impl Client {
    /// Get service info from the node.
    ///
    /// Returns the [`GetServiceInfoResponse`] proto type with fields populated
    /// according to the `read_mask`.
    ///
    /// # Field Mask
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`SERVICE_INFO_READ_MASK`].
    ///
    /// **Optional fields:**
    /// - `chain_id` - The chain identifier (digest of the genesis checkpoint)
    /// - `chain` - Human-readable chain name (e.g., `mainnet`, `testnet`)
    /// - `epoch` - Current epoch of the node
    /// - `executed_checkpoint_height` - Height of the most recently executed
    ///   checkpoint
    /// - `executed_checkpoint_timestamp` - Timestamp of the most recently
    ///   executed checkpoint
    /// - `lowest_available_checkpoint` - Lowest checkpoint with data available
    /// - `lowest_available_checkpoint_objects` - Lowest checkpoint with object
    ///   data available
    /// - `server` - Software version of the service
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    ///
    /// // Get service info with default fields
    /// let info = client.get_service_info(None).await?;
    /// println!("Chain ID: {:?}", info.chain_id);
    /// println!("Epoch: {:?}", info.epoch);
    ///
    /// // Get service info with all fields
    /// let info = client
    ///     .get_service_info(Some(
    ///         "chain_id,chain,epoch,executed_checkpoint_height,\
    ///          executed_checkpoint_timestamp,lowest_available_checkpoint,\
    ///          lowest_available_checkpoint_objects,server",
    ///     ))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_service_info(
        &self,
        read_mask: Option<&str>,
    ) -> Result<GetServiceInfoResponse> {
        let request = GetServiceInfoRequest {
            read_mask: Some(field_mask_with_default(read_mask, SERVICE_INFO_READ_MASK)),
        };

        let mut client = self.ledger_service_client();
        let response = client.get_service_info(request).await?.into_inner();

        Ok(response)
    }
}
