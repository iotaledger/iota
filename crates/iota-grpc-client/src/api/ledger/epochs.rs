// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for epoch queries.

use iota_grpc_types::{
    field::FieldMask,
    v0::{epoch::Epoch, ledger_service::GetEpochRequest},
};

use crate::{
    Client,
    api::{Result, TryFromProtoError},
};

impl Client {
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
        let request = GetEpochRequest {
            epoch: None, // Current epoch
            read_mask: Some(FieldMask {
                paths: vec![field.to_string()],
            }),
        };

        let mut client = self.ledger_service_client();
        let response = client.get_epoch(request).await?.into_inner();

        response
            .epoch
            .and_then(extractor)
            .ok_or(TryFromProtoError::missing(field).into())
    }
}
