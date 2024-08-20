// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example showcases how to use the Read API to listen
//! for transactions. It subscribes to the transactions that
//! transfer IOTA on the Iota testnet and prints every incoming
//! transaction to the console. The program will loop until it
//! is force stopped.
//!
//! cargo run --example transaction_subscription

use futures::stream::StreamExt;
use iota_json_rpc_types::TransactionFilter;
use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let ws = IotaClientBuilder::default()
        .ws_url("wss://rpc.testnet.iota.io:443")
        .build("https://fullnode.testnet.iota.io:443")
        .await?;
    println!("WS version {:?}", ws.api_version());

    let mut subscribe = ws
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
