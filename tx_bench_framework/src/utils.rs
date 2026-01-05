// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_move_build::{BuildConfig, CompiledPackage};
use iota_sdk::{
    IotaClient,
    rpc_types::{Coin, IotaObjectDataOptions, IotaTransactionBlockResponseOptions, ObjectChange},
    types::{
        base_types::{IotaAddress, ObjectID},
        crypto::SignatureScheme::ED25519,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use iota_types::{
    base_types::ObjectRef,
    signature::GenericSignature,
};
use iota_sdk_types::Intent;
use iota_types::error::IotaResult;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const SPONSOR_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

#[derive(Deserialize)]
struct FaucetResponse {
    task: String,
    error: Option<String>,
}

pub async fn request_tokens(client: &IotaClient, address: IotaAddress) -> Result<()> {
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
            .get_object_with_options(ObjectID::from_str(&coin_id)?, object_id.clone())
            .await?;

        if let Some(owner) = object.owner() {
            if owner.get_owner_address()? == *expected_owner {
                break;
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

pub async fn get_two_distinct_coins(
    client: &IotaClient,
    owner: IotaAddress,
) -> Result<(iota_sdk::rpc_types::Coin, iota_sdk::rpc_types::Coin)> {
    let page = client
        .coin_read_api()
        .get_coins(owner, None, None, Some(50))
        .await
        .expect("get_coins failed");

    let mut coins = page.data;
    if coins.len() < 2 {
        bail!("need at least 2 coins for owner {owner}: one for gas, one to split/transfer");
    }

    // We're getting 2 first coin_object_id
    coins.sort_by(|a, b| b.balance.cmp(&a.balance));
    let gas_coin = coins.remove(0);
    let pay_coin = coins
        .into_iter()
        .find(|c| c.coin_object_id != gas_coin.coin_object_id)
        .ok_or_else(|| anyhow!("could not find second distinct coin"))?;

    Ok((gas_coin, pay_coin))
}

pub fn compile_package(path_str: &str) -> IotaResult<CompiledPackage> {
    BuildConfig::new_for_testing().build(Path::new(path_str))
}

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

pub fn setup_keystore() -> Result<FileBasedKeystore, anyhow::Error> {
    let keystore_path = PathBuf::from("iotatempdb");
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }
    FileBasedKeystore::new(&keystore_path)
}

pub fn clean_keystore() -> Result<(), anyhow::Error> {
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
    let sponsor = keystore.import_from_mnemonic(SPONSOR_ADDRESS_MNEMONIC, ED25519, None, None)?;

    println!("Sponsor address: {sponsor:?}");

    let gas_coin = iota_client
        .coin_read_api()
        .get_coins(sponsor, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sponsor"))?;

    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_all_iota(recipient);
        builder.finish()
    };

    let gas_budget = 10_000_000;
    let gas_price = iota_client.read_api().get_reference_gas_price().await?;

    let tx_data = TransactionData::new_programmable(
        sponsor,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    let signature = keystore.sign_secure(&sponsor, &tx_data, Intent::iota_transaction())?;

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

pub async fn publish_move_package<K: AccountKeystore>(
    client: &IotaClient,
    sender: IotaAddress,
    keystore: &K,
    move_pkg_path: &PathBuf,
    gas_budget: u64,
) -> Result<(String, ObjectID, ObjectRef)> {
    let compiled = compile_package(
        move_pkg_path
            .to_str()
            .ok_or_else(|| anyhow!("Bad package_path"))?,
    )
    .context("compile_package failed")?;

    let gas_coin = get_coin(client, sender).await.context("get_coin failed")?;

    let tx_data = client
        .transaction_builder()
        .publish(
            sender,
            compiled.get_package_bytes(false),
            compiled.get_dependency_storage_package_ids(),
            gas_coin.coin_object_id,
            gas_budget,
        )
        .await
        .context("transaction_builder().publish failed")?;

    let signatures: Vec<GenericSignature> = vec![
        keystore
            .sign_secure(&sender, &tx_data, Intent::iota_transaction())?
            .into(),
    ];

    let resp = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, signatures),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .context("execute_transaction_block failed")?;

    println!("\n--- Raw response (pretty) ---");
    println!("{}", serde_json::to_string_pretty(&resp)?);

    let package_id: ObjectID = resp
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes.iter().find_map(|change| match change {
                ObjectChange::Published { .. } => Some(change.object_ref().0),
                _ => None,
            })
        })
        .expect("Expected a Published object in the transaction response");

    let metadata_ref: ObjectRef = resp
        .object_changes
        .as_ref()
        .and_then(|changes| {
            changes.iter().find_map(|change| match change {
                ObjectChange::Created {
                    object_type, ..
                } => {
                    let ty = object_type.to_string();
                    let is_package_metadata = ty.contains("0x2::package_metadata::PackageMetadataV1");
                    if is_package_metadata {
                        Some(change.object_ref())
                    } else {
                        None
                    }
                }
                _ => None,
            })
        })
        .ok_or_else(|| anyhow!("Expected a package metadata object in the transaction response"))?;

    Ok((resp.digest.to_string(), package_id, metadata_ref))
}
