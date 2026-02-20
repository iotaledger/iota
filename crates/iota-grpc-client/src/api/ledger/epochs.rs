// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for epoch queries.

use iota_grpc_types::{
    field::FieldMask,
    v0::{epoch::Epoch, ledger_service::GetEpochRequest},
};

use crate::{
    Client,
    api::{EPOCH_READ_MASK, Result, TryFromProtoError, field_mask_with_default},
};

impl Client {
    /// Get epoch information.
    ///
    /// Returns the [`Epoch`] proto type with fields populated according to the
    /// `read_mask`.
    ///
    /// # Parameters
    ///
    /// * `epoch` - The epoch to query. If `None`, returns the current epoch.
    /// * `read_mask` - Optional field mask specifying which fields to include.
    ///   If `None`, uses [`EPOCH_READ_MASK`].
    ///
    /// **Optional fields:**
    /// - `epoch` - The epoch number
    /// - `committee` - The validator committee for this epoch
    /// - `bcs_system_state` - BCS-encoded system state snapshot
    /// - `first_checkpoint` - First checkpoint in this epoch
    /// - `last_checkpoint` - Last checkpoint in this epoch
    /// - `start` - Epoch start timestamp
    /// - `end` - Epoch end timestamp
    /// - `reference_gas_price` - Reference gas price in NANOS
    /// - `protocol_config` - Protocol configuration for this epoch. The
    ///   sub-fields use a two-level path because each map is wrapped in its own
    ///   message:
    ///   - `protocol_config.protocol_version` - the protocol version number
    ///   - `protocol_config.feature_flags.flags` - all feature flags
    ///     (`map<string, bool>`)
    ///   - `protocol_config.feature_flags.flags.<name>` - a single named flag
    ///   - `protocol_config.attributes.attributes` - all numeric attributes
    ///     (`map<string, string>`)
    ///   - `protocol_config.attributes.attributes.<name>` - a single named
    ///     attribute
    ///
    ///   > **Note:** Requesting just `protocol_config.feature_flags` (without
    ///   > `.flags`) or just `protocol_config.attributes` (without
    ///   > `.attributes`) results in the respective wrapper field being `None`
    ///   > in the response. This lets clients distinguish "field not requested"
    ///   > from "empty map". Always include the inner field name to receive
    ///   > data.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    ///
    /// // Get current epoch with default fields
    /// let epoch = client.get_epoch(None, None).await?;
    /// println!("Epoch: {:?}", epoch.epoch);
    ///
    /// // Get specific epoch with custom fields
    /// let epoch = client
    ///     .get_epoch(Some(0), Some("epoch,reference_gas_price,first_checkpoint"))
    ///     .await?;
    ///
    /// // Get all feature flags for the current epoch
    /// let epoch = client
    ///     .get_epoch(None, Some("protocol_config.feature_flags.flags"))
    ///     .await?;
    /// let flags = epoch.protocol_config.unwrap().feature_flags.unwrap().flags;
    ///
    /// // Get a single named feature flag
    /// let epoch = client
    ///     .get_epoch(
    ///         None,
    ///         Some("protocol_config.feature_flags.flags.zklogin_auth"),
    ///     )
    ///     .await?;
    ///
    /// // Get a single named attribute
    /// let epoch = client
    ///     .get_epoch(
    ///         None,
    ///         Some("protocol_config.attributes.attributes.max_tx_gas"),
    ///     )
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_epoch(&self, epoch: Option<u64>, read_mask: Option<&str>) -> Result<Epoch> {
        let mut request = GetEpochRequest::default()
            .with_read_mask(field_mask_with_default(read_mask, EPOCH_READ_MASK));

        if let Some(epoch) = epoch {
            request = request.with_epoch(epoch);
        }

        let mut client = self.ledger_service_client();
        let response = client.get_epoch(request).await?.into_inner();

        response
            .epoch
            .ok_or(TryFromProtoError::missing("epoch").into())
    }

    /// Get the reference gas price for the current epoch.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let gas_price = client.get_reference_gas_price().await?;
    /// println!("Reference gas price: {gas_price} NANOS");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_reference_gas_price(&self) -> Result<u64> {
        self.get_epoch_field("reference_gas_price", |e| e.reference_gas_price)
            .await
    }

    /// Internal helper to fetch a single field from the current epoch.
    async fn get_epoch_field<T>(
        &self,
        field: &str,
        extractor: impl FnOnce(Epoch) -> Option<T>,
    ) -> Result<T> {
        // Current epoch (no epoch field set)
        let request = GetEpochRequest::default().with_read_mask(FieldMask {
            paths: vec![field.to_string()],
        });

        let mut client = self.ledger_service_client();
        let response = client.get_epoch(request).await?.into_inner();

        response
            .epoch
            .and_then(extractor)
            .ok_or(TryFromProtoError::missing(field).into())
    }
}
