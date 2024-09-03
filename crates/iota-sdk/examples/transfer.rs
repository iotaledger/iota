// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to transfer IOTAs or an object.
//!
//! cargo run --example transfer

mod utils;

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

    let gas_budget = 5_000_000;

    // Build the transaction data to transfer the gas coin to the recipient address
    let tx_data = client
        .transaction_builder()
        .transfer_iota(sender, gas_coin.coin_object_id, gas_budget, recipient, None)
        .await?;

    println!("Executing the transaction...");
    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    // Very similar to above, but works with any object, not just with IOTAs

    let object_to_transfer = coins.next().expect("missing coin");
    let gas_coin = coins.next().expect("missing gas coin");

    // Build the transaction data to transfer the object to the recipient address
    let tx_data = client
        .transaction_builder()
        .transfer_object(
            sender,
            object_to_transfer.coin_object_id,
            Some(gas_coin.coin_object_id),
            gas_budget,
            recipient,
        )
        .await?;

    println!("Executing the transaction...");
    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
