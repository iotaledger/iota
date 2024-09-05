// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to pay coins to another address. This also works with
//! non-IOTA coins.
//!
//! cargo run --example pay

mod utils;

use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, recipient) = setup_for_write().await?;

    // Get the coins we will use for the payment and gas
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let coin_object_id = coins.data[0].coin_object_id;
    let gas_coin_object_id = coins.data[1].coin_object_id;

    let gas_budget = 5_000_000;

    // Build the transaction data to transfer 1_000 from the provided coin to the
    // recipient address
    let tx_data = client
        .transaction_builder()
        .pay(
            sender,
            vec![coin_object_id],
            vec![recipient],
            vec![1_000],
            gas_coin_object_id,
            gas_budget,
        )
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
