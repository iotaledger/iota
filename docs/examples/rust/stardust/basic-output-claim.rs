// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the claim of a basic output.
//! In order to work, it requires a network with test objects
//! generated from iota-genesis-builder/src/stardust/test_outputs.

use std::str::FromStr;

use anyhow::anyhow;
use docs_examples::utils::{clean_keystore, setup_keystore};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    IotaClientBuilder,
    rpc_types::{IotaData, IotaObjectDataOptions, IotaTransactionBlockResponseOptions},
    types::{
        IOTA_FRAMEWORK_ADDRESS, STARDUST_ADDRESS, TypeTag,
        base_types::ObjectID,
        crypto::SignatureScheme::ED25519,
        gas_coin::GAS,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        stardust::output::BasicOutput,
        transaction::{Argument, ObjectArg, Transaction, TransactionData},
    },
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup the temporary file based keystore
    let mut keystore = setup_keystore()?;

    // Derive the address of the first account and set it as default
    let sender = keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, None, None)?;

    println!("Sender address: {sender}");

    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sender"))?;

    // This object id was fetched manually. It refers to a Basic Output object that
    // contains some Native Tokens.
    let basic_output_object_id = ObjectID::from_hex_literal(
        "0xde09139ed46b9f5f876671e4403f312fad867c5ae5d300a252e4b6a6f1fa1fbd",
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

    // Convert the basic output object into its Rust representation
    let basic_output = bcs::from_bytes::<BasicOutput>(
        &basic_output_object
            .bcs
            .expect("should contain bcs")
            .try_as_move()
            .expect("should convert it to a move object")
            .bcs_bytes,
    )?;

    // Extract the keys of the native_tokens bag if this is not empty; here the keys
    // are the type_arg of each native token, so they can be used later in the PTB.
    let mut df_type_keys = vec![];
    let native_token_bag = basic_output.native_tokens;
    if native_token_bag.size > 0 {
        // Get the dynamic fields owned by the native tokens bag
        let dynamic_field_page = iota_client
            .read_api()
            .get_dynamic_fields(*native_token_bag.id.object_id(), None, None)
            .await?;
        // should have only one page
        assert!(!dynamic_field_page.has_next_page);

        // Extract the dynamic fields keys, i.e., the native token type
        df_type_keys.extend(
            dynamic_field_page
                .data
                .into_iter()
                .map(|dyi| {
                    dyi.name
                        .value
                        .as_str()
                        .expect("should be a string")
                        .to_owned()
                })
                .collect::<Vec<_>>(),
        );
    }

    // Create a PTB to for claiming the assets of a basic output
    let pt = {
        // Init the builder
        let mut builder = ProgrammableTransactionBuilder::new();

        ////// Command #1: extract the base token and native tokens bag.
        // Type argument for a Basic Output coming from the IOTA network, i.e., the IOTA
        // token or Gas type tag
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
            // return a `base_token` (i.e., IOTA) balance and a `Bag` of native tokens
            let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
            let mut extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);

            ////// Command #2: extract the native tokens from the Bag and send them to sender.
            for type_key in df_type_keys {
                // Type argument for a Native Token contained in the basic output bag
                let type_arguments = vec![TypeTag::from_str(&format!("0x{type_key}"))?];
                // Then pass the bag and the receiver address as input
                let arguments = vec![extracted_native_tokens_bag, builder.pure(sender)?];
                extracted_native_tokens_bag = builder.programmable_move_call(
                    STARDUST_ADDRESS.into(),
                    ident_str!("utilities").to_owned(),
                    ident_str!("extract_and_send_to").to_owned(),
                    type_arguments,
                    arguments,
                );
            }

            ////// Command #3: delete the bag
            let arguments = vec![extracted_native_tokens_bag];
            builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("bag").to_owned(),
                ident_str!("destroy_empty").to_owned(),
                vec![],
                arguments,
            );

            ////// Command #4: create a coin from the extracted IOTA balance
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

    // Create the transaction data that will be sent to the network
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // Sign the transaction
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // Execute transaction
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction digest: {}", transaction_response.digest);

    // Finish and clean the temporary keystore file
    clean_keystore()
}
