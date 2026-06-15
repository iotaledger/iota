// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A set of utility functions for the examples.

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_move_build::BuildConfig;
use iota_sdk::{
    IotaClient,
    rpc_types::{
        Coin, IotaObjectDataOptions, IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponse,
        IotaTransactionBlockResponseOptions, ObjectChange,
    },
    types::{
        base_types::{IotaAddress, ObjectRef},
        crypto::SignatureScheme::ED25519,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use iota_sdk_types::{ObjectId, ProgrammableTransaction, crypto::Intent};
use iota_types::{move_package, transaction::TransactionDataAPI};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

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
    let sponsor = keystore.import_from_mnemonic(SPONSOR_ADDRESS_MNEMONIC, ED25519, None, None)?;

    println!("Sponsor address: {sponsor:?}");

    // Get a gas coin.
    let gas_coin = get_coin(iota_client, sponsor).await?;

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
    iota_client: &IotaClient,
    keystore: &mut FileBasedKeystore,
    publisher: IotaAddress,
) -> Result<ObjectId> {
    let transaction_response =
        publish_package(iota_client, keystore, publisher, CUSTOM_NFT_PACKAGE_PATH).await?;

    let tx_effects = transaction_response
        .effects
        .expect("Transaction has no effects");
    let package_ref = tx_effects
        .created()
        .first()
        .expect("There are no created objects");
    let package_id = package_ref.reference.object_id;
    println!("Package ID: {package_id}");
    Ok(package_id)
}

/// Utility function for publishing an account package found in the Move
/// examples.
pub async fn publish_aa_package<Keystore: AccountKeystore>(
    iota_client: &IotaClient,
    keystore: &mut Keystore,
    publisher: IotaAddress,
    package: &str,
) -> Result<(ObjectId, ObjectRef)> {
    let transaction_response = publish_package(iota_client, keystore, publisher, package).await?;

    let package_ref = transaction_response
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes
                .iter()
                .find(|change| matches!(change, ObjectChange::Published { .. }))
                .map(|change| change.object_ref())
        })
        .expect("Package ref must be found");
    let package_id = package_ref.object_id;
    println!("Package ID: {package_id}");

    let package_metadata_id = move_package::derive_package_metadata_id(package_id);
    println!("Package Metadata ID: {package_metadata_id}");

    let package_metadata_object = iota_client
        .read_api()
        .get_object_with_options(package_metadata_id, IotaObjectDataOptions::new().with_bcs())
        .await?
        .data
        .ok_or(anyhow!("Package metadata not found"))?;

    let package_metadata_ref = package_metadata_object.object_ref();

    Ok((package_id, package_metadata_ref))
}

/// Utility function for publishing a package found in the Move examples.
pub async fn publish_package<Keystore: AccountKeystore>(
    iota_client: &IotaClient,
    keystore: &mut Keystore,
    publisher: IotaAddress,
    package: &str,
) -> Result<IotaTransactionBlockResponse> {
    // Get a gas coin
    let gas_coin = get_coin(iota_client, publisher).await?;

    // Build custom nft package
    let package_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(package);
    let compiled_package = BuildConfig::new_for_testing().build(&package_path)?;
    let modules = compiled_package
        .get_modules()
        .map(|module| {
            let mut buf = Vec::new();
            module.serialize(&mut buf)?;
            Ok(buf)
        })
        .collect::<Result<Vec<Vec<u8>>>>()?;
    let dependencies = compiled_package.get_dependency_storage_package_ids();

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
        publisher,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // Sign the transaction
    let signature = keystore.sign_secure(&publisher, &tx_data, Intent::iota_transaction())?;

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

    Ok(transaction_response)
}

#[derive(Deserialize)]
struct FaucetResponse {
    task: String,
    error: Option<String>,
}

/// Utility function to request tokens from the local faucet.
pub async fn request_tokens_from_faucet(client: &IotaClient, address: IotaAddress) -> Result<()> {
    let address_str = address.to_string();
    let reqwest_client = Client::new();
    let body = json!({ "FixedAmountRequest": { "recipient": &address_str } });

    let response = reqwest_client
        .post("http://127.0.0.1:9123/v1/gas")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        bail!("Faucet request failed with status {}", response.status());
    }

    let FaucetResponse { task, error } = response.json().await?;
    if let Some(err) = error {
        bail!("Faucet request error: {}", err);
    }

    wait_for_faucet_completion(client, &reqwest_client, &task, &address).await
}

/// Utility function to wait for the faucet task to complete.
async fn wait_for_faucet_completion(
    client: &IotaClient,
    reqwest_client: &Client,
    task_id: &str,
    expected_owner: &IotaAddress,
) -> Result<()> {
    let coin_id = loop {
        let response = reqwest_client
            .get(format!("http://127.0.0.1:9123/v1/status/{task_id}"))
            .send()
            .await?
            .text()
            .await?;

        if response.contains("SUCCEEDED") {
            let json: serde_json::Value = serde_json::from_str(&response)?;
            let id = json
                .pointer("/status/transferred_gas_objects/sent/0/id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Failed to parse coin ID from faucet response"))?;
            break id.to_string();
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    };

    let object_id = IotaObjectDataOptions::new().with_owner();
    loop {
        let object = client
            .read_api()
            .get_object_with_options(ObjectId::from_str(&coin_id)?, object_id.clone())
            .await?;

        if let Some(owner) = object.owner() {
            if owner.into_address() == *expected_owner {
                break;
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

/// Utility function to get a coin for an address.
pub async fn get_coin(iota_client: &IotaClient, addr: IotaAddress) -> Result<Coin> {
    let coin_page = iota_client
        .coin_read_api()
        .get_coins(addr, None, None, None)
        .await?;

    coin_page
        .data
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No coin object found for address {addr}"))
}

/// Utility function to create a transaction data.
pub async fn create_transaction_data(
    iota_client: &IotaClient,
    sender: IotaAddress,
    pt: ProgrammableTransaction,
) -> Result<TransactionData> {
    let gas_coin = get_coin(iota_client, sender).await?;

    let gas_budget = 50_000_000;
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    Ok(TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    ))
}

/// Utility function to create and sign a transaction.
pub async fn create_and_sign_transaction<Keystore: AccountKeystore>(
    iota_client: &IotaClient,
    keystore: &mut Keystore,
    sender: IotaAddress,
    pt: ProgrammableTransaction,
) -> Result<Transaction> {
    let tx_data = create_transaction_data(iota_client, sender, pt).await?;

    let signature = keystore.sign_secure(&sender, &tx_data, Intent::iota_transaction())?;

    Ok(Transaction::from_data(tx_data, vec![signature]))
}

/// Utility function to execute a transaction.
pub async fn execute_transaction(
    iota_client: &IotaClient,
    transaction: Transaction,
) -> Result<IotaTransactionBlockResponse> {
    Ok(iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            transaction,
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?)
}

/// Utility function to execute a PTB transaction.
pub async fn execute_ptb<Keystore: AccountKeystore>(
    iota_client: &IotaClient,
    keystore: &mut Keystore,
    sender: IotaAddress,
    pt: ProgrammableTransaction,
) -> Result<IotaTransactionBlockResponse> {
    let transaction = create_and_sign_transaction(iota_client, keystore, sender, pt).await?;
    execute_transaction(iota_client, transaction).await
}
