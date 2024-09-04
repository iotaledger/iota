// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example gets a move package by its object id.
//!
//! cargo run --example read_package

use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;

    let move_package = client
        .read_api()
        .get_normalized_move_modules_by_package("0x1".parse()?)
        .await?;
    println!("Move package: {move_package:?}");

    Ok(())
}
