// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example showcases how to use the Read API to listen for transactions.
//! It subscribes to the transactions that transfer IOTA on the IOTA testnet and
//! prints every incoming transaction to the console. The program will loop
//! until it is force stopped.
//!
//! cargo run --example transaction_subscription

use futures::stream::StreamExt;
use iota_sdk::{IotaClientBuilder, rpc_types::TransactionFilter};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default()
        .ws_url("wss://api.testnet.iota.cafe")
        .build("https://api.testnet.iota.cafe")
        .await?;
    println!("WS version {:?}", client.api_version());

    let mut subscribe = client
        .read_api()
        .subscribe_transaction(TransactionFilter::MoveFunction {
            package: "0x2".parse()?,
            module: Some("iota".to_owned()),
            function: Some("transfer".to_owned()),
        })
        .await?;

    loop {
        println!("{:?}", subscribe.next().await);
    }
}
