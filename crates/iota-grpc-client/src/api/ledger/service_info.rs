// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for service info queries.

use iota_grpc_types::v1::ledger_service::{GetServiceInfoRequest, GetServiceInfoResponse};

use crate::{
    Client,
    api::{GET_SERVICE_INFO_READ_MASK, MetadataEnvelope, Result, field_mask_with_default},
};

impl Client {
    /// Get service info from the node.
    ///
    /// Returns the [`GetServiceInfoResponse`] proto type with fields populated
    /// according to the `read_mask`.
    ///
    /// # Available Read Mask Fields
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`GET_SERVICE_INFO_READ_MASK`].
    ///
    /// ## Network Fields
    /// - `chain_id` - the ID of the chain, which can be used to identify the
    ///   network
    /// - `chain` - the chain identifier, which can be used to identify the
    ///   network
    ///
    /// ## Current State Fields
    /// - `epoch` - the current epoch
    /// - `executed_checkpoint_height` - the height of the last executed
    ///   checkpoint
    /// - `executed_checkpoint_timestamp` - the timestamp of the last executed
    ///   checkpoint
    ///
    /// ## Availability Fields
    /// - `lowest_available_checkpoint` - lowest available checkpoint for which
    ///   transaction and checkpoint data can be requested
    /// - `lowest_available_checkpoint_objects` - lowest available checkpoint
    ///   for which object data can be requested
    ///
    /// ## Server Fields
    /// - `server` - the server version
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
    /// println!("Chain ID: {:?}", info.body().chain_id);
    /// println!("Epoch: {:?}", info.body().epoch);
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
    ) -> Result<MetadataEnvelope<GetServiceInfoResponse>> {
        let request = GetServiceInfoRequest::default().with_read_mask(field_mask_with_default(
            read_mask,
            GET_SERVICE_INFO_READ_MASK,
        ));

        let mut client = self.ledger_service_client();
        let response = client.get_service_info(request).await?;

        Ok(MetadataEnvelope::from(response))
    }
}
