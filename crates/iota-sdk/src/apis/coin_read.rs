// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{future, sync::Arc};

use futures::{stream, StreamExt};
use futures_core::Stream;
use iota_json_rpc_api::CoinReadApiClient;
use iota_json_rpc_types::{Balance, Coin, CoinPage, IotaCoinMetadata};
use iota_types::{
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
};

use crate::{
    error::{Error, IotaRpcResult},
    RpcClient,
};

/// Coin Read API provides the functionality needed to get information from the
/// Iota network regarding the coins owned by an address.
#[derive(Debug, Clone)]
pub struct CoinReadApi {
    api: Arc<RpcClient>,
}

impl CoinReadApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Return a paginated response with the coins for the given address, or an
    /// error upon failure.
    ///
    /// The coins can be filtered by `coin_type` (e.g.,
    /// 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
    /// or use `None` for the default `Coin<IOTA>`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .get_coins(address, None, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_coins(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> IotaRpcResult<CoinPage> {
        Ok(self
            .api
            .http
            .get_coins(owner, coin_type, cursor, limit)
            .await?)
    }

    /// Return a paginated response with all the coins for the given address, or
    /// an error upon failure.
    ///
    /// This function includes all coins. If needed to filter by coin type, use
    /// the `get_coins` method instead.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
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
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> IotaRpcResult<CoinPage> {
        Ok(self.api.http.get_all_coins(owner, cursor, limit).await?)
    }

    /// Return the coins for the given address as a stream.
    ///
    /// The coins can be filtered by `coin_type` (e.g.,
    /// 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
    /// or use `None` for the default `Coin<IOTA>`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let coins = iota.coin_read_api().get_coins_stream(address, None);
    ///     Ok(())
    /// }
    /// ```
    pub fn get_coins_stream(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
    ) -> impl Stream<Item = Coin> + '_ {
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

    /// Return a list of coins for the given address, or an error upon failure.
    ///
    /// Note that the function selects coins to meet or exceed the requested
    /// `amount`. If that it is not possible, it will fail with an
    /// insufficient fund error.
    ///
    /// The coins can be filtered by `coin_type` (e.g.,
    /// 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
    /// or use `None` to use the default `Coin<IOTA>`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .select_coins(address, None, 5, vec![])
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn select_coins(
        &self,
        address: IotaAddress,
        coin_type: Option<String>,
        amount: u128,
        exclude: Vec<ObjectID>,
    ) -> IotaRpcResult<Vec<Coin>> {
        let mut total = 0u128;
        let coins = self
            .get_coins_stream(address, coin_type)
            .filter(|coin: &Coin| future::ready(!exclude.contains(&coin.coin_object_id)))
            .take_while(|coin: &Coin| {
                let ready = future::ready(total < amount);
                total += coin.balance as u128;
                ready
            })
            .collect::<Vec<_>>()
            .await;

        if total < amount {
            return Err(Error::InsufficientFund { address, amount });
        }
        Ok(coins)
    }

    /// Return the balance for the given coin type owned by address, or an error
    /// upon failure.
    ///
    /// Note that this function sums up all the balances of all the coins
    /// matching the given coin type. By default, if `coin_type` is set to
    /// `None`, it will use the default `Coin<IOTA>`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let balance = iota.coin_read_api().get_balance(address, None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_balance(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
    ) -> IotaRpcResult<Balance> {
        Ok(self.api.http.get_balance(owner, coin_type).await?)
    }

    /// Return a list of balances for each coin type owned by the given address,
    /// or an error upon failure.
    ///
    /// Note that this function groups the coins by coin type, and sums up all
    /// their balances.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let all_balances = iota.coin_read_api().get_all_balances(address).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_all_balances(&self, owner: IotaAddress) -> IotaRpcResult<Vec<Balance>> {
        Ok(self.api.http.get_all_balances(owner).await?)
    }

    /// Return the coin metadata (name, symbol, description, decimals, etc.) for
    /// a given coin type, or an error upon failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let coin_metadata = iota
    ///         .coin_read_api()
    ///         .get_coin_metadata("0x2::iota::IOTA".to_string())
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_coin_metadata(
        &self,
        coin_type: String,
    ) -> IotaRpcResult<Option<IotaCoinMetadata>> {
        Ok(self.api.http.get_coin_metadata(coin_type).await?)
    }

    /// Return the total supply for a given coin type, or an error upon failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let total_supply = iota
    ///         .coin_read_api()
    ///         .get_total_supply("0x2::iota::IOTA".to_string())
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_total_supply(&self, coin_type: String) -> IotaRpcResult<Supply> {
        Ok(self.api.http.get_total_supply(coin_type).await?)
    }
}
