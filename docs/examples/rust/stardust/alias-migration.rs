// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the conversion of a stardust Alias into a custom
//! user's NFT collections controller. In order to work, it requires a network
//! with test objects generated from
//! iota-genesis-builder/src/stardust/test_outputs.

use std::str::FromStr;

use anyhow::{anyhow, Result};
use docs_examples::utils::{clean_keystore, publish_custom_nft_package, setup_keystore};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::{
    rpc_types::{IotaData, IotaObjectDataOptions, IotaTransactionBlockResponseOptions},
    types::{
        base_types::ObjectID,
        crypto::SignatureScheme::ED25519,
        gas_coin::GAS,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        stardust::output::AliasOutput,
        transaction::{Argument, CallArg, ObjectArg, Transaction, TransactionData},
        TypeTag, IOTA_FRAMEWORK_PACKAGE_ID, STARDUST_PACKAGE_ID,
    },
    IotaClientBuilder,
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Build an iota client for a local network
    let iota_client = IotaClientBuilder::default().build_localnet().await?;

    // Setup the temporary file based keystore
    let mut keystore = setup_keystore()?;

    // Derive the address of the first account and set it as default
    let sender = keystore.import_from_mnemonic(MAIN_ADDRESS_MNEMONIC, ED25519, None)?;

    println!("{sender:?}");

    // Publish the package of a custom NFT collection and then get the package id.
    // The custom NFT module is obtained from a Move example in the docs.
    // It is the same used in the Nft migration example.
    let custom_nft_package_id =
        publish_custom_nft_package(sender, &mut keystore, &iota_client).await?;

    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found"))?;

    // Get an AliasOutput object id
    let alias_output_object_id = ObjectID::from_hex_literal(
        "0x354a1864c8af23fde393f7603bc133f755a9405353b30878e41b929eb7e37554",
    )?;

    // Get an AliasOutput object
    let alias_output_object = iota_client
        .read_api()
        .get_object_with_options(
            alias_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .ok_or(anyhow!("Alias output not found"))?;
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
    let mut df_type_keys: Vec<String> = vec![];
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

    // Create a PTB that extracts the related stardust Alias from the AliasOutput
    // and then calls the
    // `custom_nft::collection::convert_alias_to_collection_controller_cap` function
    // to convert it into an NFT collection controller, create a collection and mint
    // a few NFTs.
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        let arguments = vec![builder.obj(ObjectArg::ImmOrOwnedObject(alias_output_object_ref))?];
        // Call the nft_output::extract_assets function
        if let Argument::Result(extracted_assets) = builder.programmable_move_call(
            STARDUST_PACKAGE_ID,
            ident_str!("alias_output").to_owned(),
            ident_str!("extract_assets").to_owned(),
            vec![GAS::type_tag()],
            arguments,
        ) {
            // The alias output can always be unlocked by the governor address. So the
            // command will be successful and will return a `base_token` (i.e., IOTA)
            // balance, a `Bag` of the related native tokens and the related Alias object.
            let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
            let mut extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);
            let alias_asset = Argument::NestedResult(extracted_assets, 2);

            // Call the conversion function to create an NFT collection controller from the
            // extracted alias.
            let nft_collection_controller = builder.programmable_move_call(
                custom_nft_package_id,
                ident_str!("collection").to_owned(),
                ident_str!("convert_alias_to_collection_controller_cap").to_owned(),
                vec![],
                vec![alias_asset],
            );

            // Create an NFT collection
            let nft_collection_name = builder
                .input(CallArg::Pure(bcs::to_bytes("Collection name").unwrap()))
                .unwrap();

            let nft_collection = builder.programmable_move_call(
                custom_nft_package_id,
                ident_str!("collection").to_owned(),
                ident_str!("create_collection").to_owned(),
                vec![],
                vec![nft_collection_controller, nft_collection_name],
            );

            // Mint a collection-related NFT
            let nft_name = builder
                .input(CallArg::Pure(bcs::to_bytes("NFT name").unwrap()))
                .unwrap();
            let nft_description = builder
                .input(CallArg::Pure(bcs::to_bytes("NFT description").unwrap()))
                .unwrap();
            let nft_url_value = builder
                .input(CallArg::Pure(bcs::to_bytes("NFT URL").unwrap()))
                .unwrap();
            let nft_url = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("url").to_owned(),
                ident_str!("new_unsafe").to_owned(),
                vec![],
                vec![nft_url_value],
            );

            let nft = builder.programmable_move_call(
                custom_nft_package_id,
                ident_str!("nft").to_owned(),
                ident_str!("mint_collection_related").to_owned(),
                vec![],
                vec![nft_collection, nft_name, nft_description, nft_url],
            );

            // Transfer the NFT
            builder.transfer_arg(sender, nft);

            // Drop the NFT collection to make impossible to mint new related NFTs
            builder.programmable_move_call(
                custom_nft_package_id,
                ident_str!("collection").to_owned(),
                ident_str!("drop_collection").to_owned(),
                vec![],
                vec![nft_collection_controller, nft_collection],
            );

            // Transfer the NFT collection controller
            builder.transfer_arg(sender, nft_collection_controller);

            // Extract IOTA balance
            let iota_coin = builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("coin").to_owned(),
                ident_str!("from_balance").to_owned(),
                vec![GAS::type_tag()],
                vec![extracted_base_token],
            );

            // Transfer IOTA balance
            builder.transfer_arg(sender, iota_coin);

            // Extract the native tokens from the bag.
            for type_key in df_type_keys {
                let type_arguments = vec![TypeTag::from_str(&format!("0x{type_key}"))?];
                let arguments = vec![extracted_native_tokens_bag, builder.pure(sender)?];

                // Extract a native token balance.
                extracted_native_tokens_bag = builder.programmable_move_call(
                    STARDUST_PACKAGE_ID,
                    ident_str!("utilities").to_owned(),
                    ident_str!("extract_and_send_to").to_owned(),
                    type_arguments,
                    arguments,
                );
            }

            // Cleanup bag.
            builder.programmable_move_call(
                IOTA_FRAMEWORK_PACKAGE_ID,
                ident_str!("bag").to_owned(),
                ident_str!("destroy_empty").to_owned(),
                vec![],
                vec![extracted_native_tokens_bag],
            );
        }
        builder.finish()
    };

    // Setup gas budget and gas price
    let gas_budget = 10_000_000;
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
