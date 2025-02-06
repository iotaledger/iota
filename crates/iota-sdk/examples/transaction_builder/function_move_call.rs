// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[path = "../utils.rs"]
mod utils;

use anyhow::anyhow;
use iota_config::{IOTA_KEYSTORE_FILENAME, iota_config_dir};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        Identifier,
        base_types::ObjectID,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Argument, CallArg, Command, Transaction, TransactionData},
    },
};
use shared_crypto::intent::Intent;
use utils::setup_for_write;

// This example shows how to use programmable transactions to chain multiple
// commands into one transaction, and specifically how to call a function from a
// move package These are the following steps:
// 1) finds a coin from the active address that has IOTA,
// 2) creates a PTB and adds an input to it,
// 3) adds a move call to the PTB,
// 4) signs the transaction,
// 5) executes it.
// For some of these actions it prints some output.
// Finally, at the end of the program it prints the number of coins for the
// IOTA address that received the coin.
// If you run this program several times, you should see the number of coins
// for the recipient address increases.

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // 1) get the IOTA client, the sender and recipient that we will use
    // for the transaction, and find the coin we use as gas
    let (iota, sender, _recipient) = setup_for_write().await?;

    // we need to find the coin we will use as gas
    let coins = iota
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let coin = coins.data.into_iter().next().unwrap();

    // 2) create a programmable transaction builder to add commands and create a PTB
    let mut ptb = ProgrammableTransactionBuilder::new();

    // Create an Argument::Input for Pure 6 value of type u64
    let input_value = 10u64;
    let input_argument = CallArg::Pure(bcs::to_bytes(&input_value).unwrap());

    // Add this input to the builder
    ptb.input(input_argument)?;

    // 3) add a move call to the PTB
    // Replace the pkg_id with the package id you want to call
    let pkg_id = "0x883393ee444fb828aa0e977670cf233b0078b41d144e6208719557cb3888244d";
    let package = ObjectID::from_hex_literal(pkg_id).map_err(|e| anyhow!(e))?;
    let module = Identifier::new("hello_world").map_err(|e| anyhow!(e))?;
    let function = Identifier::new("hello_world").map_err(|e| anyhow!(e))?;
    ptb.command(Command::move_call(
        package,
        module,
        function,
        vec![],
        vec![Argument::Input(0)],
    ));

    // build the transaction block by calling finish on the ptb
    let builder = ptb.finish();

    let gas_budget = 10_000_000;
    let gas_price = iota.read_api().get_reference_gas_price().await?;
    // create the transaction data that will be sent to the network
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![coin.object_ref()],
        builder,
        gas_budget,
        gas_price,
    );

    // 4) sign transaction
    let keystore = FileBasedKeystore::new(&iota_config_dir()?.join(IOTA_KEYSTORE_FILENAME))?;
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // 5) execute the transaction
    print!("Executing the transaction...");
    let transaction_response = iota
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;
    println!("{}", transaction_response);
    Ok(())
}
