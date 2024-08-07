// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the conversion of a stardust NFT into a custom user's
//! NFT. In order to work, it requires a network with test objects
//! generated from iota-genesis-builder/src/stardust/test_outputs.

use std::{fs, path::PathBuf};

use anyhow::{anyhow, Result};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_move_build::BuildConfig;
use iota_sdk::{
    rpc_types::{
        IotaObjectDataOptions, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponseOptions,
    },
    types::{
        base_types::{IotaAddress, ObjectID},
        crypto::SignatureScheme::ED25519,
        gas_coin::GAS,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Argument, ObjectArg, Transaction, TransactionData},
        IOTA_FRAMEWORK_ADDRESS, STARDUST_ADDRESS,
    },
    IotaClient, IotaClientBuilder,
};
use move_core_types::ident_str;
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const MAIN_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";
const CUSTOM_NFT_PACKAGE_PATH: &str = "../move/custom_nft";

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
    let custom_nft_package_id =
        publish_custom_nft_package(sender, &mut keystore, &iota_client, CUSTOM_NFT_PACKAGE_PATH)
            .await?;

    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found"))?;

    // Get an NftOutput object id
    let nft_output_object_id = ObjectID::from_hex_literal(
        "0x6445847625cec7d1265ebb9d0da8050a2e43d2856c2746d3579df499a1a64226",
    )?;

    // Get an NftOutput object
    let nft_output_object = iota_client
        .read_api()
        .get_object_with_options(
            nft_output_object_id,
            IotaObjectDataOptions::new().with_bcs(),
        )
        .await?
        .data
        .ok_or(anyhow!("Nft output not found"))?;

    let nft_output_object_ref = nft_output_object.object_ref();

    // Create a PTB that extracts the stardust NFT from an NFTOutput and then calls
    // the `random_nft::convert` method for converting it into a custom NFT of the
    // collection just published.
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let type_arguments = vec![GAS::type_tag()];
        let arguments = vec![builder.obj(ObjectArg::ImmOrOwnedObject(nft_output_object_ref))?];
        // Call the nft_output::extract_assets function
        if let Argument::Result(extracted_assets) = builder.programmable_move_call(
            STARDUST_ADDRESS.into(),
            ident_str!("nft_output").to_owned(),
            ident_str!("extract_assets").to_owned(),
            type_arguments,
            arguments,
        ) {
            // If the nft output can be unlocked, the command will be succesful
            // and will return a `base_token` (i.e., IOTA) balance and a
            // `Bag` of native tokens and related nft object.
            let extracted_base_token = Argument::NestedResult(extracted_assets, 0);
            let extracted_native_tokens_bag = Argument::NestedResult(extracted_assets, 1);
            let nft_asset = Argument::NestedResult(extracted_assets, 2);

            // Call conversion function in order to create custom nft from stardust nft
            // asset.
            builder.programmable_move_call(
                custom_nft_package_id,
                ident_str!("custom_nft").to_owned(),
                ident_str!("convert").to_owned(),
                vec![],
                vec![nft_asset],
            );

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

fn setup_keystore() -> Result<FileBasedKeystore, anyhow::Error> {
    // Create a temporary keystore
    let keystore_path = PathBuf::from("iotatempdb");
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }
    // Read iota keystore
    FileBasedKeystore::new(&keystore_path)
}

fn clean_keystore() -> Result<(), anyhow::Error> {
    // Remove files
    fs::remove_file("iotatempdb")?;
    fs::remove_file("iotatempdb.aliases")?;
    Ok(())
}

async fn publish_custom_nft_package(
    sender: IotaAddress,
    keystore: &mut FileBasedKeystore,
    iota_client: &IotaClient,
    package_path: &str,
) -> Result<ObjectID> {
    // Get a gas coin
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found"))?;

    // Build custom nft package
    let compiled_package = BuildConfig::default().build(package_path.into())?;
    let modules = compiled_package
        .get_modules()
        .map(|module| {
            let mut buf = Vec::new();
            module.serialize(&mut buf)?;
            Ok(buf)
        })
        .collect::<Result<Vec<Vec<u8>>>>()?;
    let dependencies = compiled_package.get_dependency_original_package_ids();

    // Publish package
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.publish_immutable(modules, dependencies);
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

    println!(
        "Package publishing transaction digest: {}",
        transaction_response.digest
    );

    // Extract package id from the transaction effects
    let tx_effects = transaction_response
        .effects
        .expect("Transaction has no effects");
    let package_ref = tx_effects
        .created()
        .first()
        .expect("There are no created objects");
    let package_id = package_ref.reference.object_id;
    println!("Package ID: {}", package_id);
    Ok(package_id)
}
