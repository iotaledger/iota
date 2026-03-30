// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for epoch queries.

use iota_grpc_types::{
    field::FieldMask,
    v1::{epoch::Epoch, ledger_service::GetEpochRequest},
};

use crate::{
    Client,
    api::{
        GET_EPOCH_READ_MASK, MetadataEnvelope, Result, TryFromProtoError, field_mask_with_default,
    },
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
    ///   If `None`, uses [`GET_EPOCH_READ_MASK`].
    ///
    /// # Available Read Mask Fields
    ///
    /// ## Epoch Fields
    /// - `epoch` - the epoch number
    /// - `committee` - the validator committee for this epoch
    /// - `bcs_system_state` - the BCS-encoded system state at the beginning of
    ///   the epoch for past epochs or the current system state for the current
    ///   epoch, which can be used for historical state queries or to get the
    ///   current state respectively
    ///
    /// ## Checkpoint Fields
    /// - `first_checkpoint` - the first checkpoint included in the epoch
    /// - `last_checkpoint` - the last checkpoint included in the epoch, which
    ///   may be unavailable for the current epoch if it has not ended yet
    ///
    /// ## Timing Fields
    /// - `start` - the timestamp of the first checkpoint included in the epoch
    /// - `end` - the timestamp of the last checkpoint included in the epoch,
    ///   which may be unavailable for the current epoch if it has not ended yet
    ///
    /// ## Gas Fields
    /// - `reference_gas_price` - the reference gas price during the epoch,
    ///   denominated in NANOS
    ///
    /// ## Protocol Configuration Fields
    /// - `protocol_config` - the protocol configuration during the epoch
    ///   - `protocol_config.protocol_version` - the protocol version during the
    ///     epoch
    ///   - `protocol_config.feature_flags` - the individual protocol feature
    ///     flags during the epoch (use `protocol_config.feature_flags.<key>` to
    ///     filter specific flags)
    ///   - `protocol_config.attributes` - the individual protocol attributes
    ///     during the epoch (use `protocol_config.attributes.<key>` to filter
    ///     specific attributes)
    ///
    ///   > **Note:** Other than for all other fields, wildcards don't work for
    ///   > `protocol_config.feature_flags` and `protocol_config.attributes`
    ///   > since they are maps (`protocol_config` is not enough). If you want
    ///   > all entries, you must specify the map directly, or single entries of
    ///   > it by name.
    ///   > (e.g. `protocol_config.feature_flags` to get all entries, or
    ///   > `protocol_config.feature_flags.zklogin_auth` to get a single flag)
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
    /// println!("Epoch: {:?}", epoch.body().epoch);
    ///
    /// // Get specific epoch with custom fields
    /// let epoch = client
    ///     .get_epoch(Some(0), Some("epoch,reference_gas_price,first_checkpoint"))
    ///     .await?;
    ///
    /// // Get all feature flags for the current epoch
    /// let epoch = client
    ///     .get_epoch(None, Some("protocol_config.feature_flags"))
    ///     .await?
    ///     .into_inner();
    /// let flags = epoch.protocol_config.unwrap().feature_flags.unwrap().flags;
    ///
    /// // Get a single named feature flag
    /// let epoch = client
    ///     .get_epoch(None, Some("protocol_config.feature_flags.zklogin_auth"))
    ///     .await?;
    ///
    /// // Get all protocol attributes for the current epoch
    /// let epoch = client
    ///     .get_epoch(None, Some("protocol_config.attributes"))
    ///     .await?
    ///     .into_inner();
    /// let attributes = epoch
    ///     .protocol_config
    ///     .unwrap()
    ///     .attributes
    ///     .unwrap()
    ///     .attributes;
    ///
    /// // Get a single named attribute
    /// let epoch = client
    ///     .get_epoch(None, Some("protocol_config.attributes.max_tx_gas"))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_epoch(
        &self,
        epoch: Option<u64>,
        read_mask: Option<&str>,
    ) -> Result<MetadataEnvelope<Epoch>> {
        let mut request = GetEpochRequest::default()
            .with_read_mask(field_mask_with_default(read_mask, GET_EPOCH_READ_MASK));

        if let Some(epoch) = epoch {
            request = request.with_epoch(epoch);
        }

        let mut client = self.ledger_service_client();
        let response = client.get_epoch(request).await?;

        MetadataEnvelope::from(response).try_map(|r| {
            r.epoch
                .ok_or_else(|| TryFromProtoError::missing("epoch").into())
        })
    }

    /// Get the reference gas price for the current epoch.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let gas_price = client.get_reference_gas_price().await?.into_inner();
    /// println!("Reference gas price: {gas_price} NANOS");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_reference_gas_price(&self) -> Result<MetadataEnvelope<u64>> {
        self.get_epoch_field("reference_gas_price", |e| e.reference_gas_price)
            .await
    }

    /// Internal helper to fetch a single field from the current epoch.
    async fn get_epoch_field<T>(
        &self,
        field: &str,
        extractor: impl FnOnce(Epoch) -> Option<T>,
    ) -> Result<MetadataEnvelope<T>> {
        // Current epoch (no epoch field set)
        let request = GetEpochRequest::default().with_read_mask(FieldMask {
            paths: vec![field.to_string()],
        });

        let mut client = self.ledger_service_client();
        let response = client.get_epoch(request).await?;

        MetadataEnvelope::from(response).try_map(|r| {
            r.epoch
                .and_then(extractor)
                .ok_or_else(|| TryFromProtoError::missing(field).into())
        })
    }
}
