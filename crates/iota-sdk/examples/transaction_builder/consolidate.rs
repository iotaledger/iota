// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to consolidate/merge all coin objects of an address
//! into a single one.
//!
//! cargo run --example consolidate

#[path = "../utils.rs"]
mod utils;

use futures::StreamExt;
use iota_sdk::{
    rpc_types::ObjectChange,
    types::{
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{Argument, Command, ObjectArg, TransactionData, TransactionKind},
    },
};
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, _) = setup_for_write().await?;

    let gas_budget = 5_000_000;
    let mut gas_coin_ref = None;

    let mut chunks = client
        .coin_read_api()
        .get_coins_stream(sender, None)
        // Max inputs with current params and without exceeding the transaction size is 1676
        .chunks(1000)
        .boxed();
    while let Some(mut input_coins_one_tx) = chunks.next().await {
        println!("Merging {} coin objects...", input_coins_one_tx.len());

        if gas_coin_ref.is_none() {
            gas_coin_ref.replace(
                input_coins_one_tx
                    .remove(
                        input_coins_one_tx
                            .iter()
                            .position(|c| c.balance >= gas_budget)
                            .expect("no coin with enough balance for gas"),
                    )
                    .object_ref(),
            );
        }

        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            // Max arguments for a programmable transaction command -1 because of the
            // GasCoin argument
            for input_coins_one_cmd in input_coins_one_tx.chunks(511) {
                let coin_args = input_coins_one_cmd
                    .iter()
                    .map(|c| {
                        builder
                            .obj(ObjectArg::ImmOrOwnedObject(c.object_ref()))
                            .unwrap()
                    })
                    .collect::<Vec<_>>();
                builder.command(Command::MergeCoins(Argument::GasCoin, coin_args));
            }
            builder.finish()
        };
        let kind = TransactionKind::ProgrammableTransaction(pt);

        let gas_price = client.read_api().get_reference_gas_price().await?;
        let tx_data =
            TransactionData::new(kind, sender, gas_coin_ref.unwrap(), gas_budget, gas_price);

        let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;
        println!("Transaction sent {}", transaction_response.digest);

        // Update the gas_coin_ref for the next transaction
        for object_change in transaction_response
            .object_changes
            .expect("missing object changes")
        {
            if let ObjectChange::Mutated {
                object_id,
                version,
                digest,
                ..
            } = object_change
            {
                gas_coin_ref.replace((object_id, version, digest));
            }
        }
    }

    Ok(())
}
