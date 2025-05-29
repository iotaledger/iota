// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to simulate a transaction, without actually executing
//! it.
//!
//! cargo run --example simulate_transaction

#[path = "../utils.rs"]
mod utils;

use iota_sdk::types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TransactionData, TransactionKind},
};
use utils::setup_for_write;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, recipient) = setup_for_write().await?;

    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin = coins.data.into_iter().next().unwrap();

    let programmable_transaction = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder
            .pay_iota(vec![recipient], vec![1_000_000_000])
            .unwrap();
        builder.finish()
    };

    let gas_budget = 5_000_000;
    let gas_price = client.read_api().get_reference_gas_price().await?;

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        programmable_transaction.clone(),
        gas_budget,
        gas_price,
    );

    let dry_run_tx_resp = client.read_api().dry_run_transaction_block(tx_data).await?;
    println!("{dry_run_tx_resp:?}");

    let dev_inspect_result = client
        .read_api()
        .dev_inspect_transaction_block(
            sender,
            TransactionKind::programmable(programmable_transaction),
            None,
            None,
            None,
        )
        .await?;
    println!("{dev_inspect_result:?}");

    Ok(())
}
