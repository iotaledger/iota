// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to stake/withdraw staked coins and get events for the
//! staking address.
//!
//! cargo run --example stake

#[path = "../utils.rs"]
mod utils;

use futures::StreamExt;
use iota_json_rpc_types::EventFilter;
use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Get the IOTA client, the sender and recipient that we will use
    // for the transaction
    let (client, sender, _) = setup_for_write().await?;

    // Get the coin we will use for the amount
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let coin = coins.data.into_iter().next().unwrap();

    let gas_budget = 50_000_000;

    // Get a validator
    let validator = match client
        .governance_api()
        .get_latest_iota_system_state()
        .await?
    {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].clone(),
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].clone(),
        _ => panic!("unsupported IotaSystemStateSummary"),
    }
    .iota_address;

    // Build the transaction data, to stake 1 IOTA
    let tx_data = client
        .transaction_builder()
        .request_add_stake(
            sender,
            vec![coin.coin_object_id],
            // Min delegation amount is 1 IOTA
            1_000_000_000,
            validator,
            None,
            gas_budget,
        )
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    // Unstake IOTA, if staking for longer than 1 epoch already

    let current_epoch = client.read_api().get_checkpoints(None, 1, true).await?.data[0].epoch;
    let staked_iota = client.governance_api().get_stakes(sender).await?;

    if let Some(staked_iota_id) = staked_iota.into_iter().find_map(|d| {
        d.stakes.into_iter().find_map(|s| {
            if s.stake_request_epoch < current_epoch {
                Some(s.staked_iota_id)
            } else {
                None
            }
        })
    }) {
        let tx_data = client
            .transaction_builder()
            .request_withdraw_stake(sender, staked_iota_id, None, gas_budget)
            .await?;

        let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

        println!("Transaction sent {}", transaction_response.digest);
        println!("Object changes:");
        for object_change in transaction_response.object_changes.unwrap() {
            println!("{:?}", object_change);
        }
    } else {
        println!("No stake found that can be unlocked (must be staked >= 1 epoch)")
    };

    // Wait some time to let the indexer process the tx, before requesting the
    // events
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let events = client
        .event_api()
        .get_events_stream(EventFilter::Sender(sender), None, true)
        .collect::<Vec<_>>()
        .await;
    println!("{events:?}");

    Ok(())
}
