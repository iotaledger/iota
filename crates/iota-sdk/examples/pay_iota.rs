// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to pay IOTAs to another address.
//!
//! cargo run --example pay_iota

mod utils;
use iota_config::{iota_config_dir, IOTA_KEYSTORE_FILENAME};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use shared_crypto::intent::Intent;
use utils::setup_for_write;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 1) Get the Iota client, the sender and recipient that we will use
    // for the transaction
    let (iota, sender, recipient) = setup_for_write().await?;

    // 2) Get the coin we will use as gas and for the payment
    let coins = iota
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin = coins.data.into_iter().next().unwrap();

    let gas_budget = 5_000_000;
    let gas_price = iota.read_api().get_reference_gas_price().await?;

    // 3) Build the transaction data, to transfer 1_000 NANOS from the gas coin to
    //    the recipient address
    let tx_data = TransactionData::new_pay_iota(
        sender,
        vec![],
        vec![recipient],
        vec![1_000],
        gas_coin.object_ref(),
        gas_budget,
        gas_price,
    )?;

    // 4) Sign transaction
    let keystore = FileBasedKeystore::new(&iota_config_dir()?.join(IOTA_KEYSTORE_FILENAME))?;
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // 5) Execute the transaction
    println!("Executing the transaction...");
    let transaction_response = iota
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
