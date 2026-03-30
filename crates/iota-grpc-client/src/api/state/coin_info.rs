// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for coin info queries.

use iota_grpc_types::v1::state_service::{GetCoinInfoRequest, GetCoinInfoResponse};
use iota_sdk_types::StructTag;

use crate::{
    Client,
    api::{MetadataEnvelope, Result},
};

impl Client {
    /// Get information about a coin type.
    ///
    /// Returns the [`GetCoinInfoResponse`] proto type with metadata, treasury,
    /// and regulation information for the specified coin type.
    ///
    /// # Parameters
    ///
    /// - `coin_type` - The coin type as a [`StructTag`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::StructTag;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let coin_type: StructTag = "0x2::iota::IOTA".parse()?;
    ///
    /// let response = client.get_coin_info(coin_type).await?;
    /// let info = response.body();
    /// println!("Coin info: {:?}", info);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_coin_info(
        &self,
        coin_type: StructTag,
    ) -> Result<MetadataEnvelope<GetCoinInfoResponse>> {
        let request = GetCoinInfoRequest::default().with_coin_type(coin_type.to_string());

        let mut client = self.state_service_client();
        let response = client.get_coin_info(request).await?;

        Ok(MetadataEnvelope::from(response))
    }
}
