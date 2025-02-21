// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example uses the ReadApi to get transaction blocks with options.
//!
//! cargo run --example transactions_with_options

#[path = "../utils.rs"]
mod utils;

use std::time::Duration;

use iota_sdk::rpc_types::IotaTransactionBlockResponseOptions;
use tokio::time::sleep;
use utils::{setup_for_write, split_coin_digest};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, active_address, _) = setup_for_write().await?;

    // We make dummy transactions which returns a transaction digest
    let tx_digest_0 = split_coin_digest(&client, &active_address).await?;
    println!("Tx 0: {tx_digest_0}");
    // Wait some time for the indexer to process the tx and make it available
    sleep(Duration::from_secs(2)).await;
    let tx_digest_1 = split_coin_digest(&client, &active_address).await?;
    println!("Tx 1: {tx_digest_1}");
    sleep(Duration::from_secs(2)).await;

    // Get a single transaction block with options
    let tx_response = client
        .read_api()
        .get_transaction_with_options(
            tx_digest_0,
            IotaTransactionBlockResponseOptions {
                show_input: true,
                show_raw_input: true,
                show_effects: true,
                show_events: true,
                show_object_changes: true,
                show_balance_changes: true,
                show_raw_effects: true,
            },
        )
        .await?;
    println!("Transaction succeeded: {:?}\n", tx_response.status_ok());
    println!("Transaction data:\n{tx_response:?}");

    // Get multiple transaction blocks with options
    let tx_responses = client
        .read_api()
        .multi_get_transactions_with_options(
            vec![tx_digest_0, tx_digest_1],
            IotaTransactionBlockResponseOptions::default(),
        )
        .await?;
    println!("Transaction data:\n{tx_responses:?}");

    Ok(())
}
