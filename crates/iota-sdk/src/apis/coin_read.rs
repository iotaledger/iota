// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{future, sync::Arc};

use futures::{StreamExt, stream};
use futures_core::Stream;
use iota_json_rpc_api::CoinReadApiClient;
use iota_json_rpc_types::{Balance, Coin, CoinPage, IotaCirculatingSupply, IotaCoinMetadata};
use iota_types::{
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
};

use crate::{
    RpcClient,
    error::{Error, IotaRpcResult},
};

/// Defines methods that retrieve information from the IOTA network regarding
/// the coins owned by an address.
#[derive(Debug, Clone)]
pub struct CoinReadApi {
    api: Arc<RpcClient>,
}

impl CoinReadApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Get coins for the given address filtered by coin type.
    /// Results are paginated.
    ///
    /// The coin type defaults to `0x2::iota::IOTA`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::{IotaClientBuilder, types::base_types::IotaAddress};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let coin_type = String::from("0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC");
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .get_coins(address, coin_type, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_coins(
        &self,
        owner: IotaAddress,
        coin_type: impl Into<Option<String>>,
        cursor: impl Into<Option<ObjectID>>,
        limit: impl Into<Option<usize>>,
    ) -> IotaRpcResult<CoinPage> {
        Ok(self
            .api
            .http
            .get_coins(owner, coin_type.into(), cursor.into(), limit.into())
            .await?)
    }

    /// Get all the coins for the given address regardless of coin type.
    /// Results are paginated.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::{IotaClientBuilder, types::base_types::IotaAddress};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .get_all_coins(address, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_all_coins(
        &self,
        owner: IotaAddress,
        cursor: impl Into<Option<ObjectID>>,
        limit: impl Into<Option<usize>>,
    ) -> IotaRpcResult<CoinPage> {
        Ok(self
            .api
            .http
            .get_all_coins(owner, cursor.into(), limit.into())
            .await?)
    }

    /// Get the coins for the given address filtered by coin type.
    /// Returns a stream.
    ///
    /// The coin type defaults to `0x2::iota::IOTA`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::{IotaClientBuilder, types::base_types::IotaAddress};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let coin_type = String::from("0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC");
    ///     let coins = iota.coin_read_api().get_coins_stream(address, coin_type);
    ///     Ok(())
    /// }
    /// ```
    pub fn get_coins_stream(
        &self,
        owner: IotaAddress,
        coin_type: impl Into<Option<String>>,
    ) -> impl Stream<Item = Coin> + '_ {
        let coin_type = coin_type.into();

        stream::unfold(
            (
                vec![],
                // cursor
                None,
                // has_next_page
                true,
                coin_type,
            ),
            move |(mut data, cursor, has_next_page, coin_type)| async move {
                if let Some(item) = data.pop() {
                    Some((item, (data, cursor, /* has_next_page */ true, coin_type)))
                } else if has_next_page {
                    let page = self
                        .get_coins(owner, coin_type.clone(), cursor, Some(100))
                        .await
                        .ok()?;
                    let mut data = page.data;
                    data.reverse();
                    data.pop().map(|item| {
                        (
                            item,
                            (data, page.next_cursor, page.has_next_page, coin_type),
                        )
                    })
                } else {
                    None
                }
            },
        )
    }

    /// Get a list of coins for the given address filtered by coin type with at
    /// least `amount` total value.
    ///
    /// If it is not possible to select enough coins, this function will return
    /// an [`Error::InsufficientFunds`].
    ///
    /// The coin type defaults to `0x2::iota::IOTA`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::{IotaClientBuilder, types::base_types::IotaAddress};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let coin_type = String::from("0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC");
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .select_coins(address, coin_type, 5, vec![])
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn select_coins(
        &self,
        address: IotaAddress,
        coin_type: impl Into<Option<String>>,
        amount: u128,
        exclude: Vec<ObjectID>,
    ) -> IotaRpcResult<Vec<Coin>> {
        let mut total = 0u128;
        let coins = self
            .get_coins_stream(address, coin_type.into())
            .filter(|coin: &Coin| future::ready(!exclude.contains(&coin.coin_object_id)))
            .take_while(|coin: &Coin| {
                let ready = future::ready(total < amount);
                total += coin.balance as u128;
                ready
            })
            .collect::<Vec<_>>()
            .await;

        if total < amount {
            return Err(Error::InsufficientFunds { address, amount });
        }
        Ok(coins)
    }

    /// Get the balance for the given address filtered by coin type.
    ///
    /// The coin type defaults to `0x2::iota::IOTA`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::{IotaClientBuilder, types::base_types::IotaAddress};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let balance = iota.coin_read_api().get_balance(address, None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_balance(
        &self,
        owner: IotaAddress,
        coin_type: impl Into<Option<String>>,
    ) -> IotaRpcResult<Balance> {
        Ok(self.api.http.get_balance(owner, coin_type.into()).await?)
    }

    /// Get a list of balances grouped by coin type and owned by the given
    /// address.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::{IotaClientBuilder, types::base_types::IotaAddress};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let all_balances = iota.coin_read_api().get_all_balances(address).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_all_balances(&self, owner: IotaAddress) -> IotaRpcResult<Vec<Balance>> {
        Ok(self.api.http.get_all_balances(owner).await?)
    }

    /// Get the coin metadata (name, symbol, description, decimals, etc.) for a
    /// given coin type.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let coin_metadata = iota
    ///         .coin_read_api()
    ///         .get_coin_metadata("0x2::iota::IOTA")
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_coin_metadata(
        &self,
        coin_type: impl Into<String>,
    ) -> IotaRpcResult<Option<IotaCoinMetadata>> {
        Ok(self.api.http.get_coin_metadata(coin_type.into()).await?)
    }

    /// Get the total supply for a given coin type.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let total_supply = iota
    ///         .coin_read_api()
    ///         .get_total_supply("0x2::iota::IOTA")
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_total_supply(&self, coin_type: impl Into<String>) -> IotaRpcResult<Supply> {
        Ok(self.api.http.get_total_supply(coin_type.into()).await?)
    }

    /// Get the IOTA circulating supply summary.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let circulating_supply = iota.coin_read_api().get_circulating_supply().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_circulating_supply(&self) -> IotaRpcResult<IotaCirculatingSupply> {
        Ok(self.api.http.get_circulating_supply().await?)
    }
}
