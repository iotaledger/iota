// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to transfer IOTAs or an object.
//!
//! cargo run --example transfer

#[path = "../utils.rs"]
mod utils;

use iota_sdk_transaction_builder::TransactionBuilder;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, recipient) = setup_for_write().await?;

    // Get the coin we will use as gas and for the payment
    let coins_page = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let mut coins = coins_page.data.into_iter();
    let gas_coin = coins.next().expect("missing gas coin");
    let coin_to_transfer = coins.next().expect("missing coin to transfer");

    // Build the transaction data to transfer a coin to the recipient address
    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder
        .transfer_objects(recipient, vec![coin_to_transfer.coin_object_id])
        .gas(vec![gas_coin.coin_object_id]);

    let tx_data = builder.finish().await?;

    println!("Executing the transaction...");
    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    // Very similar to above, but works with any object, not just with IOTAs
    let object_to_transfer = coins.next().expect("missing coin");

    // Build the transaction data to transfer the object to the recipient address
    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder
        .transfer_objects(recipient, vec![object_to_transfer.coin_object_id])
        .gas(vec![gas_coin.coin_object_id]);
    let tx_data = builder.finish().await?;

    println!("Executing the transaction...");
    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    Ok(())
}
