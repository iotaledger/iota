// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to get an object ref from its object id.
//!
//! cargo run --example object_ref

use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;

    let object_ref = client
        .transaction_builder()
        .get_object_ref("0x8".parse()?)
        .await?;

    println!("Single object ref: {object_ref:?}");

    let object_refs = client
        .transaction_builder()
        .input_refs(&vec!["0x5".parse()?, "0x8".parse()?])
        .await?;
    println!("Multiple object refs: {object_refs:?}");

    Ok(())
}
