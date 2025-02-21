// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the claim of an NFT output.
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
        stardust::output::NftOutput,
        transaction::{Argument, ObjectArg, Transaction, TransactionData},
    },
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
        .ok_or(anyhow!("No coins found"))?;

    // Get an NftOutput object
    let nft_output_object_id = ObjectID::from_hex_literal(
        "0xad87a60921c62f84d57301ea127d1706b406cde5ec6fa4d3af2a80f424fab93a",
    )?;

    let nft_output_object = iota_client
        .read_api()
        .get_object_with_options(
            nft_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .ok_or(anyhow!("Nft not found"))?;

    let nft_output_object_ref = nft_output_object.object_ref();

    let nft_output = bcs::from_bytes::<NftOutput>(
        &nft_output_object
            .bcs
            .expect("should contain bcs")
            .try_as_move()
            .expect("should convert it to a move object")
            .bcs_bytes,
    )?;

    let mut df_type_keys = vec![];
    let native_token_bag = nft_output.native_tokens;
    if native_token_bag.size > 0 {
        // Get the dynamic fieldss of the native tokens bag
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
                        .to_string()
                })
                .collect::<Vec<_>>(),
        );
    }

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();

        // Extract nft assets(base token, native tokens bag, nft asset itself).
        let type_arguments = vec![GAS::type_tag()];
        let arguments = vec![builder.obj(ObjectArg::ImmOrOwnedObject(nft_output_object_ref))?];
        // Finally call the nft_output::extract_assets function
        if let Argument::Result(extracted_assets) = builder.programmable_move_call(
            STARDUST_ADDRESS.into(),
            ident_str!("nft_output").to_owned(),
            ident_str!("extract_assets").to_owned(),
            type_arguments,
            arguments,
        ) {
            // If the nft output can be unlocked, the command will be successful and will
            // return a `base_token` (i.e., IOTA) balance and a `Bag` of native tokens and
            // related nft object.
            let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
            let mut extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);
            let nft_asset = Argument::NestedResult(extracted_assets, 2);

            // Extract IOTA balance
            let arguments = vec![extracted_base_token];
            let type_arguments = vec![GAS::type_tag()];
            let iota_coin = builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("coin").to_owned(),
                ident_str!("from_balance").to_owned(),
                type_arguments,
                arguments,
            );

            // Transfer IOTA balance
            builder.transfer_arg(sender, iota_coin);

            for type_key in df_type_keys {
                let type_arguments = vec![TypeTag::from_str(&format!("0x{type_key}"))?];
                // Then pass the bag and the receiver address as input
                let arguments = vec![extracted_native_tokens_bag, builder.pure(sender)?];

                // Extract native tokens from the bag.
                // Extract native token balance
                // Transfer native token balance
                extracted_native_tokens_bag = builder.programmable_move_call(
                    STARDUST_ADDRESS.into(),
                    ident_str!("utilities").to_owned(),
                    ident_str!("extract_and_send_to").to_owned(),
                    type_arguments,
                    arguments,
                );
            }

            // Transferring nft asset
            builder.transfer_arg(sender, nft_asset);

            // Cleanup bag.
            let arguments = vec![extracted_native_tokens_bag];
            builder.programmable_move_call(
                IOTA_FRAMEWORK_ADDRESS.into(),
                ident_str!("bag").to_owned(),
                ident_str!("destroy_empty").to_owned(),
                vec![],
                arguments,
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
