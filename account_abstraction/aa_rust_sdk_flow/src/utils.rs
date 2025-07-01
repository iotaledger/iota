// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use anyhow::{Result, anyhow};
use iota_move_build::{BuildConfig, CompiledPackage};
use iota_sdk::{
    IotaClient,
    rpc_types::{Coin, IotaTransactionBlockResponse, ObjectChange},
    types::{
        base_types::{IotaAddress, ObjectID},
        error::IotaResult,
    },
};

/// Default gas budget used for all programmable transactions.
///
/// This is passed to `TransactionData::new_programmable` to define
/// how much gas the transaction is allowed to consume.
pub const GAS_BUDGET: u64 = 100_000_000;

/// Default threshold used for multi-signature initialization.
pub const THRESHOLD: u16 = 2;

/// Default weights used for multi-signature initialization.
pub const WEIGHTS: &[u8] = &[1, 2];

/// Selects the first available coin object for the given address.
///
/// This is used to pay for gas or as an input to a transaction.
pub async fn get_coin(iota_client: &IotaClient, addr: IotaAddress) -> Result<Coin> {
    let coin_page = iota_client
        .coin_read_api()
        .get_coins(addr, None, None, None)
        .await?;

    coin_page
        .data
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No coin object found for address {addr}"))
}

/// Compiles a Move package located at the given path.
///
/// Returns a `CompiledPackage` that can be published via a transaction.
///
/// # Arguments
/// - `path_str`: Path to the package root directory.
pub fn compile_package(path_str: &str) -> IotaResult<CompiledPackage> {
    BuildConfig::default().build(Path::new(path_str))
}

/// Extracts the published package's `ObjectID` from a transaction response.
///
/// This function is typically used after publishing a Move package
/// to get the ID of the deployed package object.
pub fn package_id(tx_response: IotaTransactionBlockResponse) -> ObjectID {
    tx_response
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes.iter().find_map(|change| match change {
                ObjectChange::Published { .. } => Some(change.object_ref().0),
                _ => None,
            })
        })
        .expect("Expected a Published object in the transaction response")
}

/// Checks whether the recipient's total balance matches the expected withdrawal
/// amount.
///
/// # Returns
/// `true` if the balance matches, otherwise `false`.
pub async fn check_recipient_balance(
    iota_client: &IotaClient,
    recipient_addr: IotaAddress,
    expected_balance: u128,
) -> Result<bool> {
    let addr_balance = iota_client
        .coin_read_api()
        .get_balance(recipient_addr, None)
        .await?;
    Ok(addr_balance.total_balance == expected_balance)
}
