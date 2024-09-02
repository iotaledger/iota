// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to split and merge coins.
//!
//! cargo run --example split_merge_coins

mod utils;

use std::time::Duration;

use tokio::time::sleep;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, _) = setup_for_write().await?;

    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin = coins.data.into_iter().next().unwrap();

    let gas_budget = 50_000_000;

    // Split equal
    let tx_data = client
        .transaction_builder()
        .split_coin_equal(sender, gas_coin.coin_object_id, 10, None, gas_budget)
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    // Wait some time for the indexer to process the tx
    sleep(Duration::from_secs(3)).await;

    // Split specific amounts
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin = coins.data.into_iter().next().unwrap();

    let tx_data = client
        .transaction_builder()
        .split_coin(
            sender,
            gas_coin.coin_object_id,
            vec![1_000, 1_000_000],
            None,
            gas_budget,
        )
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    // Wait some time for the indexer to process the tx
    sleep(Duration::from_secs(3)).await;

    // Merge coins
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let coin_object_ids: Vec<_> = coins.data.into_iter().map(|c| c.coin_object_id).collect();
    let tx_data = client
        .transaction_builder()
        .merge_coins(
            sender,
            coin_object_ids[0],
            coin_object_ids[1],
            None,
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
