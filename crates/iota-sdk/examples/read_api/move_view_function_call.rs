// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to create a move view function call using the
//! transaction builder.
//!
//! cargo run --example move_view_function_call

#[path = "../utils.rs"]
mod utils;

use iota_sdk_transaction_builder::{Shared, TransactionBuilder};
use iota_sdk_types::{Address, ObjectId};
use utils::setup_for_read;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender) = setup_for_read().await?;

    // Move view function call to a public function: get the current timestamp in
    // milliseconds.
    let mut builder = TransactionBuilder::new(sender).with_client(client.clone());

    builder
        .move_call(Address::FRAMEWORK, "clock", "timestamp_ms")
        .arguments([Shared(ObjectId::from_hex("0x6")?)]);

    let public_call_results = builder.dry_run(true).await?.results;
    println!("{public_call_results:?}");

    // Move view function call to a private function: get the current
    // state.
    let mut builder = TransactionBuilder::new(sender).with_client(client);

    builder
        .move_call(Address::FRAMEWORK, "random", "load_inner")
        .arguments([Shared(ObjectId::from_hex("0x8")?)]);

    let private_call_results = builder.dry_run(true).await?.results;
    println!("{private_call_results:?}");

    Ok(())
}
