// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

use iota_sdk::IotaClientBuilder;

// This example shows the few basic ways to connect to a Iota network.
// There are several in-built methods for connecting to the
// Iota devnet, tesnet, and localnet (running locally),
// as well as a custom way for connecting to custom URLs.
// The example prints out the API versions of the different networks,
// and finally, it prints the list of available RPC methods
// and the list of subscriptions.
// Note that running this code will fail if there is no Iota network
// running locally on the default address: 127.0.0.1:9000

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let iota = IotaClientBuilder::default()
        .build("http://127.0.0.1:9000") // local network address
        .await?;
    println!("Iota local network version: {}", iota.api_version());

    // local Iota network, like the above one but using the dedicated function
    let iota_local = IotaClientBuilder::default().build_localnet().await?;
    println!("Iota local network version: {}", iota_local.api_version());

    // Iota devnet -- https://fullnode.devnet.iota.io:443
    let iota_devnet = IotaClientBuilder::default().build_devnet().await?;
    println!("Iota devnet version: {}", iota_devnet.api_version());

    // Iota testnet -- https://fullnode.testnet.iota.io:443
    let iota_testnet = IotaClientBuilder::default().build_testnet().await?;
    println!("Iota testnet version: {}", iota_testnet.api_version());

    println!("{:?}", iota_local.available_rpc_methods());
    println!("{:?}", iota_local.available_subscriptions());

    Ok(())
}
