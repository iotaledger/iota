// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to split and merge coins.
//!
//! cargo run --example split_merge_coins

#[path = "../utils.rs"]
mod utils;

use std::time::Duration;

use iota_sdk_transaction_builder::{TransactionBuilder, assigned, unresolved::Argument};
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

    // Split equal (-1 IOTA to cover gas)
    const SPLIT_COUNT: usize = 4;
    let amount = (gas_coin.balance - 1_000_000_000) / SPLIT_COUNT as u64;
    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder
        .split_coins(Argument::Gas, vec![amount; SPLIT_COUNT])
        .transfer_objects(
            sender,
            (0..SPLIT_COUNT)
                .map(|i| Argument::NestedResult(0, i as u16))
                .collect::<Vec<_>>(),
        )
        .gas(vec![gas_coin.coin_object_id])
        .gas_budget(1_000_000_000);
    let tx_data = builder.finish().await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    // Wait some time for the indexer to process the tx
    sleep(Duration::from_secs(3)).await;

    // Split specific amounts
    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder
        .split_coins(Argument::Gas, [1_000u64, 1_000_000])
        .assign(vec!["coin0", "coin1"])
        .transfer_objects(sender, [assigned("coin0"), assigned("coin1")]);
    let tx_data = builder.finish().await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    // Wait some time for the indexer to process the tx
    sleep(Duration::from_secs(3)).await;

    // Merge coins
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let coin_object_ids: Vec<_> = coins.data.into_iter().map(|c| c.coin_object_id).collect();
    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder.merge_coins(coin_object_ids[0], [coin_object_ids[1]]);
    let tx_data = builder.finish().await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    Ok(())
}
