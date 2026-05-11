// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to pay IOTAs to another address with a dry run
//! before.
//!
//! cargo run --example tx_data

#[path = "../utils.rs"]
mod utils;

use anyhow::bail;
use iota_sdk_transaction_builder::TransactionBuilder;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, recipient) = setup_for_write().await?;

    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder.send_iota(recipient, 1_000_000u64);
    let dry_run_res = builder.clone().dry_run(false).await?;
    if let Some(error) = dry_run_res.error {
        bail!(error);
    }

    let tx_data = builder.finish().await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    Ok(())
}
