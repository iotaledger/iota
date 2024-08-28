// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows the few basic ways to connect to a Iota network.
//! There are several in-built methods for connecting to the
//! Iota devnet, tesnet, and localnet (running locally),
//! as well as a custom way for connecting to custom URLs.
//! The example prints out the API versions of the different networks,
//! and finally, it prints the list of available RPC methods
//! and the list of subscriptions.
//! Note that running this code will fail if there is no Iota network
//! running locally on the default address: 127.0.0.1:9000
//!
//! cargo run --example iota_client

use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default()
        .build("http://127.0.0.1:9000") // local network address
        .await?;
    println!("Iota local network version: {}", client.api_version());

    // local Iota network, like the above one but using the dedicated function
    let local_client = IotaClientBuilder::default().build_localnet().await?;
    println!("Iota local network version: {}", local_client.api_version());

    // Iota devnet -- https://fullnode.devnet.iota.io:443
    let devnet_client = IotaClientBuilder::default().build_devnet().await?;
    println!("Iota devnet version: {}", devnet_client.api_version());

    // Iota testnet -- https://fullnode.testnet.iota.io:443
    let testnet_client = IotaClientBuilder::default().build_testnet().await?;
    println!("Iota testnet version: {}", testnet_client.api_version());

    println!("{:?}", local_client.available_rpc_methods());
    println!("{:?}", local_client.available_subscriptions());

    Ok(())
}
