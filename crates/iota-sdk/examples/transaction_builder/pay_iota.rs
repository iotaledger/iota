// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to pay IOTAs to another address.
//!
//! cargo run --example pay_iota

#[path = "../utils.rs"]
mod utils;

use iota_config::{IOTA_KEYSTORE_FILENAME, iota_config_dir};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{quorum_driver_types::ExecuteTransactionRequestType, transaction::Transaction},
};
use shared_crypto::intent::Intent;
use utils::setup_for_write;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 1) Get the IOTA client, the sender and recipient that we will use
    // for the transaction
    let (client, sender, recipient) = setup_for_write().await?;

    // 2) Get the coin we will use as gas and for the payment
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin = coins.data.into_iter().next().unwrap();

    let gas_budget = 5_000_000;

    // 3) Build the transaction data, to transfer 1_000 NANOS from the gas coin to
    //    the recipient address
    let tx_data = client
        .transaction_builder()
        .pay_iota(
            sender,
            vec![gas_coin.coin_object_id],
            vec![recipient],
            vec![1_000],
            gas_budget,
        )
        .await?;

    // 4) Sign transaction
    let keystore = FileBasedKeystore::new(&iota_config_dir()?.join(IOTA_KEYSTORE_FILENAME))?;
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // 5) Execute the transaction
    println!("Executing the transaction...");
    let transaction_response = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
