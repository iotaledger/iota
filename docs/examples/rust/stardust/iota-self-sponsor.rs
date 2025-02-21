// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the self-sponsor scenario for claiming an IOTA basic
//! output. In order to work, it requires a network with test objects
//! generated from iota-genesis-builder/src/stardust/test_outputs.

use std::str::FromStr;

use anyhow::anyhow;
use bip32::DerivationPath;
use docs_examples::utils::{clean_keystore, setup_keystore};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClientBuilder,
    rpc_types::{IotaObjectDataOptions, IotaTransactionBlockResponseOptions},
    types::{
        IOTA_FRAMEWORK_ADDRESS, STARDUST_ADDRESS,
        base_types::ObjectID,
        crypto::SignatureScheme::ED25519,
        gas_coin::GAS,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Argument, ObjectArg, Transaction, TransactionData},
    },
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;

pub const IOTA_COIN_TYPE: u32 = 4218;

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "crazy drum raw dirt tooth where fee base warm beach trim rule sign silk fee fee dad large creek venue coin steel hub scale";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup the temporary file based keystore
    let mut keystore = setup_keystore()?;

    // For this example we need to derive addresses that are at different
    // indexes and coin_types, one for sponsoring with IOTA coin type and one for
    // claiming the Basic Output with IOTA coin type.
    let sponsor_derivation_path =
        DerivationPath::from_str(format!("m/44'/{IOTA_COIN_TYPE}'/0'/0'/5'").as_str())?;
    let sender_derivation_path =
        DerivationPath::from_str(format!("m/44'/{IOTA_COIN_TYPE}'/0'/0'/50'").as_str())?;

    // Derive the address of the sponsor
    let sponsor = keystore.import_from_mnemonic(
        MAIN_ADDRESS_MNEMONIC,
        ED25519,
        Some(sponsor_derivation_path),
        None,
    )?;
    println!("Sponsor address: {sponsor}");

    // Derive the address of the sender
    let sender = keystore.import_from_mnemonic(
        MAIN_ADDRESS_MNEMONIC,
        ED25519,
        Some(sender_derivation_path),
        None,
    )?;
    println!("Sender address: {sender}");

    // This object id was fetched manually. It refers to a Basic Output object that
    // contains some Native Tokens.
    let basic_output_object_id = ObjectID::from_hex_literal(
        "0xd0ed7f2c50366202585ebd52a38cde6a7a7282ef3f52db52c3ba87042bca6fba",
    )?;
    // Get Basic Output object
    let basic_output_object = iota_client
        .read_api()
        .get_object_with_options(
            basic_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .ok_or(anyhow!("Basic output not found"))?;
    let basic_output_object_ref = basic_output_object.object_ref();

    // Create a PTB to claim the assets related to the basic output.
    let pt = {
        // Init the builder
        let mut builder = ProgrammableTransactionBuilder::new();

        ////// Command #1: extract the base token and native tokens bag.
        // Type argument for a Basic Output holding IOTA coin
        let type_arguments = vec![GAS::type_tag()];
        // Then pass the basic output object as input
        let arguments = vec![builder.obj(ObjectArg::ImmOrOwnedObject(basic_output_object_ref))?];
        // Finally call the basic_output::extract_assets function
        if let Argument::Result(extracted_assets) = builder.programmable_move_call(
            STARDUST_ADDRESS.into(),
            ident_str!("basic_output").to_owned(),
            ident_str!("extract_assets").to_owned(),
            type_arguments,
            arguments,
        ) {
            // If the basic output can be unlocked, the command will be successful and will
            // return a `base_token` balance and a `Bag` of native tokens
            let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
            let extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);

            ////// Command #2: delete the empty native tokens bag
            let arguments = vec![extracted_native_tokens_bag];
            builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("bag").to_owned(),
                ident_str!("destroy_empty").to_owned(),
                vec![],
                arguments,
            );

            ////// Command #3: create a coin from the extracted IOTA balance
            // Type argument for the IOTA coin
            let type_arguments = vec![GAS::type_tag()];
            let arguments = vec![extracted_base_token];
            let new_iota_coin = builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("coin").to_owned(),
                ident_str!("from_balance").to_owned(),
                type_arguments,
                arguments,
            );

            ////// Command #5: send back the base token coin to the user.
            builder.transfer_arg(sender, new_iota_coin)
        }
        builder.finish()
    };

    // Setup gas budget and gas price
    let gas_budget = 50_000_000;
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sponsor, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sponsor"))?;

    // Create the transaction data that will be sent to the network and allow
    // sponsoring
    let tx_data = TransactionData::new_programmable_allow_sponsor(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
        sponsor,
    );

    // Sender signs the transaction
    let sender_signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // Sponsor signs the transaction
    let sponsor_signature = keystore.sign_secure(&sponsor, &tx_data, Intent::iota_transaction())?;

    // Execute transaction; the transaction data is created using the signature of
    // the sender and of the sponsor.
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![sender_signature, sponsor_signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction digest: {}", transaction_response.digest);

    // Finish and clean the temporary keystore file
    clean_keystore()
}
