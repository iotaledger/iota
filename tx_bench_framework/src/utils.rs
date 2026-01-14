// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_move_build::{BuildConfig, CompiledPackage};
use iota_sdk::{
    IotaClient, IotaClientBuilder,
    rpc_types::{Coin, IotaObjectDataOptions, IotaTransactionBlockResponseOptions, ObjectChange},
    types::{
        base_types::{IotaAddress, ObjectID},
        crypto::SignatureScheme::ED25519,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Transaction, TransactionData},
    },
};
use iota_sdk_types::Intent;
use iota_types::{base_types::ObjectRef, error::IotaResult, signature::GenericSignature};
use move_core_types::{ident_str, identifier::Identifier};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const SPONSOR_ADDRESS_MNEMONIC: &str = "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt";

const FAUCET_URL: &str = "http://127.0.0.1:9123";
const FAUCET_TIMEOUT: Duration = Duration::from_secs(60);

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
        .post(format!("{FAUCET_URL}/v1/gas"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("faucet POST failed")?;

    if !response.status().is_success() {
        bail!("Faucet request failed with status {}", response.status());
    }

    let FaucetResponse { task, error } = response
        .json()
        .await
        .context("faucet response json failed")?;
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
    let started = std::time::Instant::now();

    let coin_id = loop {
        if started.elapsed() > FAUCET_TIMEOUT {
            bail!("Faucet timeout: task_id={task_id}");
        }

        let response = reqwest_client
            .get(format!("{FAUCET_URL}/v1/status/{task_id}"))
            .send()
            .await
            .context("faucet status GET failed")?
            .text()
            .await
            .context("faucet status read body failed")?;

        if response.contains("SUCCEEDED") {
            let json: serde_json::Value =
                serde_json::from_str(&response).context("parse faucet status json failed")?;
            let id = json
                .pointer("/status/transferred_gas_objects/sent/0/id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Failed to parse coin ID from faucet response"))?;
            break id.to_string();
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    };

    let object_id = IotaObjectDataOptions::new().with_owner();

    loop {
        if started.elapsed() > FAUCET_TIMEOUT {
            bail!("Faucet ownership wait timeout: coin_id={coin_id}");
        }

        let object = client
            .read_api()
            .get_object_with_options(ObjectID::from_str(&coin_id)?, object_id.clone())
            .await
            .context("get_object_with_options failed")?;

        if let Some(owner) = object.owner() {
            if owner.get_owner_address()? == *expected_owner {
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

pub async fn get_two_distinct_coins(
    client: &IotaClient,
    owner: IotaAddress,
) -> Result<(Coin, Coin)> {
    let page = client
        .coin_read_api()
        .get_coins(owner, None, None, Some(50))
        .await
        .context("get_coins failed")?;

    let mut coins = page.data;
    if coins.len() < 2 {
        bail!("need at least 2 coins for owner {owner}: one for gas, one to split/transfer");
    }

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
        .await
        .context("get_coins failed")?;

    coin_page
        .data
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No coin object found for address {addr}"))
}

pub fn setup_keystore() -> Result<FileBasedKeystore> {
    let keystore_path = PathBuf::from("iotatempdb");
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }
    FileBasedKeystore::new(&keystore_path).map_err(Into::into)
}

pub fn clean_keystore() -> Result<()> {
    fs::remove_file("iotatempdb").ok();
    fs::remove_file("iotatempdb.aliases").ok();
    Ok(())
}

pub async fn fund_address(
    iota_client: &IotaClient,
    keystore: &mut FileBasedKeystore,
    recipient: IotaAddress,
) -> Result<()> {
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

fn extract_published_package_id(changes: &[ObjectChange]) -> Option<ObjectID> {
    changes.iter().find_map(|change| match change {
        ObjectChange::Published { .. } => Some(change.object_ref().0),
        _ => None,
    })
}

fn extract_package_metadata_ref(changes: &[ObjectChange]) -> Option<ObjectRef> {
    changes.iter().find_map(|change| match change {
        ObjectChange::Created { object_type, .. } => {
            let ty = object_type.to_string();
            if ty.contains("0x2::package_metadata::PackageMetadataV1") {
                Some(change.object_ref())
            } else {
                None
            }
        }
        _ => None,
    })
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

    let changes = resp
        .object_changes
        .as_ref()
        .ok_or_else(|| anyhow!("No object_changes in response"))?;
    let package_id = extract_published_package_id(changes)
        .ok_or_else(|| anyhow!("Expected a Published object in the transaction response"))?;

    let metadata_ref = extract_package_metadata_ref(changes)
        .ok_or_else(|| anyhow!("Expected a package metadata object in the transaction response"))?;

    Ok((resp.digest.to_string(), package_id, metadata_ref))
}

pub fn canonical_path_str(p: &PathBuf) -> String {
    std::fs::canonicalize(p)
        .unwrap_or_else(|_| p.clone())
        .to_string_lossy()
        .to_string()
}

pub async fn build_client(rpc: &str) -> Result<IotaClient> {
    IotaClientBuilder::default()
        .build(rpc)
        .await
        .map_err(|e| anyhow!("Failed to build IotaClient for {rpc}: {e}"))
}

pub async fn create_immutable_bench_objects<K: AccountKeystore>(
    client: &IotaClient,
    payer: IotaAddress,
    keystore: &K,
    package_id: ObjectID,
    gas_budget: u64,
    entry_fn: &'static str,
    expected_count: usize,
) -> Result<Vec<ObjectRef>> {
    let gas_coin = get_coin(client, payer).await.context("get_coin failed")?;
    let gas_price = client
        .read_api()
        .get_reference_gas_price()
        .await
        .context("get_reference_gas_price failed")?;

    let module = ident_str!("abstract_account").to_owned();
    let function = Identifier::new(entry_fn)
        .map_err(|e| anyhow!("Bad entry function name '{entry_fn}': {e}"))?;

    let pt = {
        let mut b = ProgrammableTransactionBuilder::new();
        b.programmable_move_call(package_id, module, function, vec![], vec![]);
        b.finish()
    };

    let tx_data = TransactionData::new_programmable(
        payer,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    let sigs: Vec<GenericSignature> = vec![
        keystore
            .sign_secure(&payer, &tx_data, Intent::iota_transaction())?
            .into(),
    ];

    let resp = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(tx_data, sigs),
            IotaTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await
        .context("execute_transaction_block(create bench objects) failed")?;

    let changes = resp
        .object_changes
        .as_ref()
        .ok_or_else(|| anyhow!("No object_changes in response"))?;

    let bench_refs: Vec<ObjectRef> = changes
        .iter()
        .filter_map(|ch| match ch {
            ObjectChange::Created { object_type, .. } => {
                let ty = object_type.to_string();
                if ty.contains("::abstract_account::BenchObject") {
                    Some(ch.object_ref())
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect();

    ensure!(
        bench_refs.len() == expected_count,
        "Expected {expected_count} BenchObject, got {}",
        bench_refs.len()
    );

    Ok(bench_refs)
}
