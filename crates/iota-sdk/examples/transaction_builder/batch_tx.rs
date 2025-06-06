// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to create a batch transaction.
//!
//! cargo run --example batch_tx

#[path = "../utils.rs"]
mod utils;

use iota_sdk::rpc_types::{RPCTransactionRequestParams, TransferObjectParams};
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, recipient) = setup_for_write().await?;

    // Get the coins we will transfer and use as gas
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin_object_id = coins.data[0].coin_object_id;
    let coin_object_id_1 = coins.data[1].coin_object_id;
    let coin_object_id_2 = coins.data[2].coin_object_id;

    let gas_budget = 5_000_000;

    // Build the transaction data, to transfer the coins to the recipient address
    let tx_data = client
        .transaction_builder()
        .batch_transaction(
            sender,
            vec![
                RPCTransactionRequestParams::TransferObjectRequestParams(TransferObjectParams {
                    recipient,
                    object_id: coin_object_id_1,
                }),
                RPCTransactionRequestParams::TransferObjectRequestParams(TransferObjectParams {
                    recipient,
                    object_id: coin_object_id_2,
                }),
            ],
            gas_coin_object_id,
            gas_budget,
        )
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
