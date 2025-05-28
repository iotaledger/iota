// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to pay IOTAs to another address with a dry run
//! before.
//!
//! cargo run --example tx_data

#[path = "../utils.rs"]
mod utils;

use iota_sdk::rpc_types::{IotaExecutionStatus, IotaTransactionBlockEffects};
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, recipient) = setup_for_write().await?;

    let gas_budget = 5_000_000;
    let rgp = client.read_api().get_reference_gas_price().await.unwrap();

    let tx_kind = client
        .transaction_builder()
        .pay_iota_tx_kind(vec![recipient], vec![1_000_000])?;

    let tx_data = client
        .transaction_builder()
        .tx_data_for_dry_run(sender, tx_kind.clone(), gas_budget, rgp, None, None)
        .await;
    let dry_run_tx_resp = client.read_api().dry_run_transaction_block(tx_data).await?;
    let IotaTransactionBlockEffects::V1(effects) = dry_run_tx_resp.effects;
    // Error if the dry run failed to save gas
    if let IotaExecutionStatus::Failure { error } = effects.status {
        return Err(anyhow::anyhow!(error));
    }

    let tx_data = client
        .transaction_builder()
        .tx_data(sender, tx_kind, gas_budget, rgp, vec![], None)
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
