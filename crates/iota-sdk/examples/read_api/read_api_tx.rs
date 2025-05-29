// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example gets a transaction with options.
//!
//! cargo run --example read_api_tx

use iota_sdk::{
    IotaClientBuilder,
    rpc_types::{IotaObjectDataOptions, IotaTransactionBlockResponseOptions},
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;

    // Get a random tx that loaded child objects
    let object = client
        .read_api()
        .get_object_with_options(
            "0x5".parse()?,
            IotaObjectDataOptions::new().with_previous_transaction(),
        )
        .await?;
    let tx_id = object.data.unwrap().previous_transaction.unwrap();

    let tx_response = client
        .read_api()
        .get_transaction_with_options(tx_id, IotaTransactionBlockResponseOptions::new())
        .await?;
    println!("Tx: {tx_response:?}");

    Ok(())
}
