// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A set of utility functions for the examples.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_move_build::BuildConfig;
use iota_sdk::{
    rpc_types::{IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponseOptions},
    types::{
        base_types::{IotaAddress, ObjectID},
        crypto::SignatureScheme::ED25519,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
    IotaClient,
};
use shared_crypto::intent::Intent;

/// Got from iota-genesis-builder/src/stardust/test_outputs/stardust_mix.rs
const SPONSOR_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

/// Move Custom NFT example relative path
const CUSTOM_NFT_PACKAGE_PATH: &str = "../move/custom_nft";

/// Creates a temporary keystore.
pub fn setup_keystore() -> Result<FileBasedKeystore, anyhow::Error> {
    let keystore_path = PathBuf::from("iotatempdb");
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }
    // Read iota keystore
    FileBasedKeystore::new(&keystore_path)
}

/// Deletes the temporary keystore.
pub fn clean_keystore() -> Result<(), anyhow::Error> {
    // Remove files
    fs::remove_file("iotatempdb")?;
    fs::remove_file("iotatempdb.aliases")?;
    Ok(())
}

/// Utility function for funding an address using the transfer of a coin.
pub async fn fund_address(
    iota_client: &IotaClient,
    keystore: &mut FileBasedKeystore,
    recipient: IotaAddress,
) -> Result<(), anyhow::Error> {
    // Derive the address of the sponsor.
    let sponsor = keystore.import_from_mnemonic(SPONSOR_ADDRESS_MNEMONIC, ED25519, None)?;

    println!("Sponsor address: {sponsor:?}");

    // Get a gas coin.
    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sponsor, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sponsor"))?;

    let pt = {
        // Init a programmable transaction builder.
        let mut builder = ProgrammableTransactionBuilder::new();
        // Pay all iotas from the gas object
        builder.pay_all_iota(recipient);
        builder.finish()
    };

    // Setup a gas budget and a gas price.
    let gas_budget = 10_000_000;
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    // Create a transaction data that will be sent to the network.
    let tx_data = TransactionData::new_programmable(
        sponsor,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // Sign the transaction.
    let signature = keystore.sign_secure(&sponsor, &tx_data, Intent::iota_transaction())?;

    // Execute the transaction.
    let transaction_response = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;

    println!(
        "Funding transaction digest: {}",
        transaction_response.digest
    );

    Ok(())
}

/// Utility function for publishing a custom NFT package found in the Move
/// examples.
pub async fn publish_custom_nft_package(
    sender: IotaAddress,
    keystore: &mut FileBasedKeystore,
    iota_client: &IotaClient,
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
    let package_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(CUSTOM_NFT_PACKAGE_PATH);
    let compiled_package = BuildConfig::default().build(package_path)?;
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
