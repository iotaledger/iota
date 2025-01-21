// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example showcases how to use the Event API.
//! At the end of the program it subscribes to the events on the IOTA testnet
//! and prints every incoming event to the console. The program will loop until
//! it is force stopped.
//!
//! cargo run --example event_api

mod utils;

use futures::stream::StreamExt;
use iota_sdk::{IotaClientBuilder, rpc_types::EventFilter};
use utils::{setup_for_write, split_coin_digest};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, active_address, _second_address) = setup_for_write().await?;

    println!(" *** Get events *** ");
    // for demonstration purposes, we set to make a transaction
    let digest = split_coin_digest(&client, &active_address).await?;
    let events = client.event_api().get_events(digest).await?;
    println!("{:?}", events);
    println!(" *** Get events ***\n ");

    let descending = true;
    let query_events = client
        .event_api()
        .query_events(EventFilter::All(vec![]), None, 5, descending) // query first 5 events in descending order
        .await?;
    println!(" *** Query events *** ");
    println!("{:?}", query_events);
    println!(" *** Query events ***\n ");

    let ws = IotaClientBuilder::default()
        .ws_url("wss://api.testnet.iota.cafe")
        .build("https://api.testnet.iota.cafe")
        .await?;
    println!("WS version {:?}", ws.api_version());

    let mut subscribe = ws
        .event_api()
        .subscribe_event(EventFilter::All(vec![]))
        .await?;

    loop {
        println!("{:?}", subscribe.next().await);
    }
}
