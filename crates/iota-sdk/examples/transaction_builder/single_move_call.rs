// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to update a PTB with a single move call.
//!
//! cargo run --example single_move_call

#[path = "../utils.rs"]
mod utils;

use iota_sdk_transaction_builder::TransactionBuilder;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, _recipient) = setup_for_write().await?;

    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder
        .move_call(iota_sdk_types::Address::STD, "u8", "max")
        .arguments((0u8, 1u8));
    let tx_data = builder.finish().await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    Ok(())
}
