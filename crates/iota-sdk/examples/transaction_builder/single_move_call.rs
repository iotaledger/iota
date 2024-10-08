// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to update a PTB with a single move call.
//!
//! cargo run --example single_move_call

#[path = "../utils.rs"]
mod utils;

use iota_json::IotaJsonValue;
use iota_types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder, transaction::TransactionData,
};
use serde_json::json;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, recipient) = setup_for_write().await?;

    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let coin = &coins.data[0];
    let gas_coin = &coins.data[1];

    let gas_budget = 5_000_000;
    let gas_price = client.read_api().get_reference_gas_price().await?;

    let mut ptb = ProgrammableTransactionBuilder::new();
    client
        .transaction_builder()
        .single_move_call(
            &mut ptb,
            "0x2".parse()?,
            "iota",
            "transfer",
            vec![],
            vec![
                IotaJsonValue::new(json!(coin.coin_object_id))?,
                IotaJsonValue::new(json!(recipient))?,
            ],
        )
        .await?;
    let pt = ptb.finish();

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
