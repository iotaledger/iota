// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example uses the Read API to get checkpoints.
//!
//! cargo run --example checkpoints

use iota_sdk::IotaClientBuilder;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;

    let latest_checkpoint_number = client
        .read_api()
        .get_latest_checkpoint_sequence_number()
        .await?;
    println!("Latest checkpoint sequence number: {latest_checkpoint_number}");

    let checkpoint = client
        .read_api()
        .get_checkpoint(latest_checkpoint_number.into())
        .await?;
    println!(
        "Latest checkpoint:\n{}",
        serde_json::to_string_pretty(&checkpoint)?
    );

    // Get the 2 checkpoints before the latest one
    let checkpoints = client
        .read_api()
        .get_checkpoints(Some((latest_checkpoint_number - 3).into()), 2, false)
        .await?;
    println!("Second and third last checkpoints:\n{:?}", checkpoints);

    Ok(())
}
