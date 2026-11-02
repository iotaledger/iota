// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example showcases how to use the Event API.
//!
//! cargo run --example event_api

mod utils;

use iota_sdk::rpc_types::EventFilter;
use utils::{setup_for_write, split_coin_digest};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, active_address, _second_address) = setup_for_write().await?;

    println!(" *** Get events *** ");
    // for demonstration purposes, we set to make a transaction
    let digest = split_coin_digest(&client, &active_address).await?;
    let events = client.event_api().get_events(digest).await?;
    println!("{events:?}");
    println!(" *** Get events ***\n ");

    let descending = true;
    let query_events = client
        .event_api()
        .query_events(EventFilter::All(vec![]), None, 5, descending) // query first 5 events in descending order
        .await?;
    println!(" *** Query events *** ");
    println!("{query_events:?}");
    println!(" *** Query events ***\n ");

    Ok(())
}
