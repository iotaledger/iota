// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{Balance, CoinPage, IotaCoinMetadata};
use iota_open_rpc_macros::open_rpc;
use iota_types::{
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// The `CoinReadApi` trait provides a set of asynchronous methods for querying
/// coin-related data on the IOTA blockchain. This trait is designed to be used
/// in an RPC context, allowing clients to fetch various types of information
/// about coins owned by an address, including balances, metadata,
/// and total supply.
///
/// The following methods are available in this trait:
///
/// - `get_coins`: Retrieves a list of coin objects of an optionally specified
///   type owned by a given address.
/// - `get_all_coins`: Retrieves a list of all coin objects owned by a specified
///   address.
/// - `get_balance`: Obtains the total balance of a specific coin type for a
///   given address.
/// - `get_all_balances`: Returns the total balance of all coin types for a
///   given address.
/// - `get_coin_metadata`: Fetches metadata (such as symbol and decimals) for a
///   specific coin type.
/// - `get_total_supply`: Retrieves the total supply for a specific coin type.
#[open_rpc(namespace = "iotax", tag = "Coin Query API")]
#[rpc(server, client, namespace = "iotax")]
pub trait CoinReadApi {
    /// Return all Coin<`coin_type`> objects owned by an address.
    #[rustfmt::skip]
    #[method(name = "getCoins")]
    async fn get_coins(
        &self,
        /// the owner's Iota address
        owner: IotaAddress,
        /// optional type name for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC), default to 0x2::iota::IOTA if not specified.
        coin_type: Option<String>,
        /// optional paging cursor
        cursor: Option<ObjectID>,
        /// maximum number of items per page
        limit: Option<usize>,
    ) -> RpcResult<CoinPage>;

    /// Return all Coin objects owned by an address.
    #[rustfmt::skip]
    #[method(name = "getAllCoins")]
    async fn get_all_coins(
        &self,
        /// the owner's Iota address
        owner: IotaAddress,
        /// optional paging cursor
        cursor: Option<ObjectID>,
        /// maximum number of items per page
        limit: Option<usize>,
    ) -> RpcResult<CoinPage>;

    /// Return the total coin balance for one coin type, owned by the address owner.
    #[rustfmt::skip]
    #[method(name = "getBalance")]
    async fn get_balance(
        &self,
        /// the owner's Iota address
        owner: IotaAddress,
        /// optional type names for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC), default to 0x2::iota::IOTA if not specified.
        coin_type: Option<String>,
    ) -> RpcResult<Balance>;

    /// Return the total coin balance for all coin type, owned by the address owner.
    #[rustfmt::skip]
    #[method(name = "getAllBalances")]
    async fn get_all_balances(
        &self,
        /// the owner's Iota address
        owner: IotaAddress,
    ) -> RpcResult<Vec<Balance>>;

    /// Return metadata (e.g., symbol, decimals) for a coin.
    #[rustfmt::skip]
    #[method(name = "getCoinMetadata")]
    async fn get_coin_metadata(
        &self,
        /// type name for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
        coin_type: String,
    ) -> RpcResult<Option<IotaCoinMetadata>>;

    /// Return total supply for a coin.
    #[rustfmt::skip]
    #[method(name = "getTotalSupply")]
    async fn get_total_supply(
        &self,
        /// type name for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
        coin_type: String,
    ) -> RpcResult<Supply>;
}
