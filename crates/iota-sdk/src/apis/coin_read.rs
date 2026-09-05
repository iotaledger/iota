// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    future,
    pin::pin,
    sync::Arc,
};

use futures::{StreamExt, stream};
use futures_core::Stream;
use iota_json_rpc_api::CoinReadApiClient;
use iota_json_rpc_types::{
    Balance, Coin, CoinPage, IotaCirculatingSupply, IotaCoinMetadata, OwnedObjectCursor,
};
use iota_sdk_types::{Address, ObjectId};
use iota_types::balance::Supply;

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
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_sdk_types::Address;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = Address::from_str("0x0000....0000")?;
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
        owner: Address,
        coin_type: impl Into<Option<String>>,
        cursor: impl Into<Option<OwnedObjectCursor>>,
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
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_sdk_types::Address;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = Address::from_str("0x0000....0000")?;
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .get_all_coins(address, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_all_coins(
        &self,
        owner: Address,
        cursor: impl Into<Option<OwnedObjectCursor>>,
        limit: impl Into<Option<usize>>,
    ) -> IotaRpcResult<CoinPage> {
        Ok(self
            .api
            .http
            .get_all_coins(owner, cursor.into(), limit.into())
            .await?)
    }

    /// Get the coins for the given address filtered by coin type.
    /// Returns a stream, which yields each coin once: a coin that comes back
    /// on a later page, because its balance changed while the pages were read,
    /// is yielded as first seen. Where the coin's current version matters,
    /// collect the coins and keep each coin's highest version, as
    /// [`Self::select_coins`] does.
    ///
    /// The coin type defaults to `0x2::iota::IOTA`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_sdk_types::Address;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = Address::from_str("0x0000....0000")?;
    ///     let coin_type = String::from("0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC");
    ///     let coins = iota.coin_read_api().get_coins_stream(address, coin_type);
    ///     Ok(())
    /// }
    /// ```
    pub fn get_coins_stream(
        &self,
        owner: Address,
        coin_type: impl Into<Option<String>>,
    ) -> impl Stream<Item = Coin> + '_ {
        once_each(self.coin_pages(owner, coin_type.into()))
    }

    /// The coins of every page `get_coins` returns, in order, including a coin
    /// that comes back on a later page.
    fn coin_pages(
        &self,
        owner: Address,
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
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_sdk_types::Address;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = Address::from_str("0x0000....0000")?;
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
        address: Address,
        coin_type: impl Into<Option<String>>,
        amount: u128,
        exclude: Vec<ObjectId>,
    ) -> IotaRpcResult<Vec<Coin>> {
        let (coins, total) =
            select_from(self.coin_pages(address, coin_type.into()), amount, &exclude).await;
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
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_sdk_types::Address;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = Address::from_str("0x0000....0000")?;
    ///     let balance = iota.coin_read_api().get_balance(address, None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_balance(
        &self,
        owner: Address,
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
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_sdk_types::Address;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = Address::from_str("0x0000....0000")?;
    ///     let all_balances = iota.coin_read_api().get_all_balances(address).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_all_balances(&self, owner: Address) -> IotaRpcResult<Vec<Balance>> {
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
        Ok(self
            .api
            .http
            .get_total_supply(coin_type.into())
            .await?
            .into())
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

/// `coins` without the repeats of a coin.
///
/// Pages are ordered by balance, so a coin whose balance changes while they
/// are read can come back on a later page; used twice, it would be a
/// duplicate input.
fn once_each(coins: impl Stream<Item = Coin>) -> impl Stream<Item = Coin> {
    let mut seen = HashSet::new();
    coins.filter(move |coin: &Coin| future::ready(seen.insert(coin.coin_object_id)))
}

/// The coins of `coins`, in order, until they add up to `amount`, leaving out
/// `exclude`; with their total, which falls short of `amount` if `coins` runs
/// out first.
///
/// A coin that comes back with a higher version, because its balance changed
/// while the pages were read, takes the place of its earlier copy, so the
/// selection holds the version a transaction can use.
async fn select_from(
    coins: impl Stream<Item = Coin>,
    amount: u128,
    exclude: &[ObjectId],
) -> (Vec<Coin>, u128) {
    let mut selected: Vec<Coin> = Vec::new();
    let mut positions: HashMap<ObjectId, usize> = HashMap::new();
    let mut total = 0u128;
    let mut coins = pin!(coins);
    while total < amount {
        let Some(coin) = coins.next().await else {
            break;
        };
        if exclude.contains(&coin.coin_object_id) {
            continue;
        }
        match positions.entry(coin.coin_object_id) {
            Entry::Occupied(position) => {
                let held = &mut selected[*position.get()];
                if coin.version > held.version {
                    total = total - held.balance as u128 + coin.balance as u128;
                    *held = coin;
                }
            }
            Entry::Vacant(position) => {
                position.insert(selected.len());
                total += coin.balance as u128;
                selected.push(coin);
            }
        }
    }
    (selected, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coin(id: ObjectId, version: u64, balance: u64) -> Coin {
        Coin {
            coin_type: "0x2::iota::IOTA".to_string(),
            coin_object_id: id,
            version: version.into(),
            digest: iota_sdk_types::ObjectDigest::ZERO,
            balance,
            previous_transaction: iota_sdk_types::TransactionDigest::ZERO,
        }
    }

    /// The selected coins as `(id, version)`, in selection order.
    fn picked(coins: &[Coin]) -> Vec<(ObjectId, u64)> {
        coins
            .iter()
            .map(|coin| (coin.coin_object_id, coin.version.as_u64()))
            .collect()
    }

    /// The stream yields a coin that comes back on a later page once, as
    /// first seen.
    #[tokio::test]
    async fn a_coin_repeated_across_pages_is_yielded_once() {
        let (a, b) = (ObjectId::random(), ObjectId::random());
        let coins = once_each(stream::iter([coin(a, 1, 5), coin(b, 1, 3), coin(a, 2, 4)]));

        let yielded: Vec<_> = coins.collect().await;
        assert_eq!(picked(&yielded), [(a, 1), (b, 1)]);
    }

    /// A coin that comes back with a higher version replaces its earlier copy,
    /// and the total follows its new balance.
    #[tokio::test]
    async fn a_newer_copy_of_a_selected_coin_replaces_it() {
        let (a, b, c) = (ObjectId::random(), ObjectId::random(), ObjectId::random());
        let coins = stream::iter([coin(a, 1, 5), coin(b, 1, 3), coin(a, 2, 4), coin(c, 1, 9)]);

        let (selected, total) = select_from(coins, 15, &[]).await;
        assert_eq!(picked(&selected), [(a, 2), (b, 1), (c, 1)]);
        assert_eq!(total, 16);
    }

    /// A newer copy that lowers the total below the amount makes the selection
    /// take further coins.
    #[tokio::test]
    async fn a_lower_newer_balance_selects_further_coins() {
        let (a, b, c) = (ObjectId::random(), ObjectId::random(), ObjectId::random());
        let coins = stream::iter([coin(a, 1, 5), coin(b, 1, 3), coin(a, 2, 1), coin(c, 1, 9)]);

        let (selected, total) = select_from(coins, 8, &[]).await;
        assert_eq!(picked(&selected), [(a, 1), (b, 1)]);
        assert_eq!(total, 8, "the amount is covered before the newer copy");

        let coins = stream::iter([coin(a, 1, 5), coin(b, 1, 3), coin(a, 2, 1), coin(c, 1, 9)]);
        let (selected, total) = select_from(coins, 9, &[]).await;
        assert_eq!(picked(&selected), [(a, 2), (b, 1), (c, 1)]);
        assert_eq!(total, 13);
    }

    /// An older copy that comes later is left out, and so are excluded coins.
    #[tokio::test]
    async fn older_copies_and_excluded_coins_are_left_out() {
        let (a, b, c) = (ObjectId::random(), ObjectId::random(), ObjectId::random());
        let coins = stream::iter([coin(a, 2, 5), coin(b, 1, 7), coin(a, 1, 9), coin(c, 1, 4)]);

        let (selected, total) = select_from(coins, 100, &[b]).await;
        assert_eq!(picked(&selected), [(a, 2), (c, 1)]);
        assert_eq!(total, 9);
    }
}
