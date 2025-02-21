// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to use programmable transactions to chain multiple
//! actions into one transaction. Specifically, the example retrieves two
//! addresses from the local wallet, and then
//! 1) finds a coin from the active address that has IOTA,
//! 2) splits the coin into one coin of 1000 NANOS and the rest,
//! 3) transfers the split coin to second IOTA address,
//! 4) signs and executes the transaction.
//!
//! For some of these actions it prints some output.
//! Finally, at the end of the program it prints the number of coins for the
//! IOTA address that received the coin. If you run this program several times,
//! you should see the number of coins for the recipient address increases.
//!
//! cargo run --example programmable_transactions_api

#[path = "../utils.rs"]
mod utils;

use iota_sdk::types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{Argument, Command, TransactionData},
};
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 1) Get the IOTA client, the sender and recipient that we will use
    // for the transaction
    let (client, sender, recipient) = setup_for_write().await?;

    // We need to find the coin we will use as gas
    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let coin = coins.data.into_iter().next().unwrap();

    // Programmable transactions allows the user to bundle a number of actions into
    // one transaction
    let mut ptb = ProgrammableTransactionBuilder::new();

    // 2) Split coin
    // The amount we want in the new coin, 1000 NANOS
    let split_coin_amount = ptb.pure(1000u64)?; // note that we need to specify the u64 type
    ptb.command(Command::SplitCoins(
        Argument::GasCoin,
        vec![split_coin_amount],
    ));

    // 3) Transfer the new coin to a different address
    let argument_address = ptb.pure(recipient)?;
    ptb.command(Command::TransferObjects(
        vec![Argument::Result(0)],
        argument_address,
    ));

    // Finish building the transaction block by calling finish on the ptb
    let transaction = ptb.finish();

    let gas_budget = 5_000_000;
    let gas_price = client.read_api().get_reference_gas_price().await?;
    // Create the transaction data that will be sent to the network
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![coin.object_ref()],
        transaction,
        gas_budget,
        gas_price,
    );

    // 5) Sign and execute transaction
    print!("Executing the transaction...");
    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;
    print!("done\n Transaction information: ");
    println!("{:?}", transaction_response);

    let coins = client
        .coin_read_api()
        .get_coins(recipient, None, None, None)
        .await?;

    println!(
        "After the transfer, the recipient address {recipient} has {} coins",
        coins.data.len()
    );
    Ok(())
}
