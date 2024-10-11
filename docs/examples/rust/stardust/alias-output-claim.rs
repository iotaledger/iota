// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the claim of an alias output.
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
        stardust::output::AliasOutput,
        transaction::{Argument, ObjectArg, Transaction, TransactionData},
    },
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an IOTA client for a local network.
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup a temporary file based keystore.
    let mut keystore = setup_keystore()?;

    // Derive the address of the first account and set it as default.
    let sender = keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, None, None)?;

    println!("Sender address: {sender:?}");

    // Get a gas coin.
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sponsor"))?;

    // Get an AliasOutput object.
    let alias_output_object_id = ObjectID::from_hex_literal(
        "0x354a1864c8af23fde393f7603bc133f755a9405353b30878e41b929eb7e37554",
    )?;
    let alias_output_object = iota_client
        .read_api()
        .get_object_with_options(
            alias_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("alias not found"))?;
    let alias_output_object_ref = alias_output_object.object_ref();

    // Convert the AliasOutput object into its Rust representation.
    let alias_output = bcs::from_bytes::<AliasOutput>(
        &alias_output_object
            .bcs
            .expect("should contain bcs")
            .try_as_move()
            .expect("should convert it to a move object")
            .bcs_bytes,
    )?;

    // Extract the keys of the native_tokens bag if it is not empty; the keys
    // are the type_arg of each native token, so they can be used later in the PTB.
    let mut df_type_keys = vec![];
    let native_token_bag = alias_output.native_tokens;
    if native_token_bag.size > 0 {
        // Get the dynamic fields owned by the native tokens bag.
        let dynamic_field_page = iota_client
            .read_api()
            .get_dynamic_fields(*native_token_bag.id.object_id(), None, None)
            .await?;
        // Only one page should exist.
        assert!(!dynamic_field_page.has_next_page);

        // Extract the dynamic fields keys, i.e., the native token type.
        df_type_keys.extend(
            dynamic_field_page
                .data
                .into_iter()
                .map(|dyi| {
                    dyi.name
                        .value
                        .as_str()
                        .expect("should be a string")
                        .to_string()
                })
                .collect::<Vec<_>>(),
        );
    }

    // Create a PTB to claim the assets related to the alias output.
    let pt = {
        // Init a programmable transaction builder.
        let mut builder = ProgrammableTransactionBuilder::new();

        // Type argument for an AliasOutput coming from the IOTA network, i.e., the
        // IOTA token or the Gas type tag.
        let type_arguments = vec![GAS::type_tag()];
        // Then pass the AliasOutput object as an input.
        let arguments = vec![builder.obj(ObjectArg::ImmOrOwnedObject(alias_output_object_ref))?];
        // Finally call the alias_output::extract_assets function.
        if let Argument::Result(extracted_assets) = builder.programmable_move_call(
            STARDUST_ADDRESS.into(),
            ident_str!("alias_output").to_owned(),
            ident_str!("extract_assets").to_owned(),
            type_arguments,
            arguments,
        ) {
            // The alias output can always be unlocked by the governor address. So the
            // command will be successful and will return a `base_token` (i.e., IOTA)
            // balance, a `Bag` of the related native tokens and the related Alias object.
            let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
            let mut extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);
            let extracted_alias = Argument::NestedResult(extracted_assets, 2);

            // Extract the IOTA balance.
            let type_arguments = vec![GAS::type_tag()];
            let arguments = vec![extracted_base_token];
            let iota_coin = builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("coin").to_owned(),
                ident_str!("from_balance").to_owned(),
                type_arguments,
                arguments,
            );

            // Transfer the IOTA balance to the sender.
            builder.transfer_arg(sender, iota_coin);

            // Extract the native tokens from the bag.
            for type_key in df_type_keys {
                let type_arguments = vec![TypeTag::from_str(&format!("0x{type_key}"))?];
                let arguments = vec![extracted_native_tokens_bag, builder.pure(sender)?];

                // Extract a native token balance.
                extracted_native_tokens_bag = builder.programmable_move_call(
                    STARDUST_ADDRESS.into(),
                    ident_str!("utilities").to_owned(),
                    ident_str!("extract_and_send_to").to_owned(),
                    type_arguments,
                    arguments,
                );
            }

            // Cleanup the bag.
            let arguments = vec![extracted_native_tokens_bag];
            builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("bag").to_owned(),
                ident_str!("destroy_empty").to_owned(),
                vec![],
                arguments,
            );

            // Transfer the alias asset.
            builder.transfer_arg(sender, extracted_alias);
        }
        builder.finish()
    };

    // Setup a gas budget and a gas price.
    let gas_budget = 10_000_000;
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Create a transaction data that will be sent to the network.
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // Sign the transaction.
    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    // Execute the transaction.
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!("Transaction digest: {}", transaction_response.digest);

    // Finish and clean the temporary keystore file.
    clean_keystore()
}
