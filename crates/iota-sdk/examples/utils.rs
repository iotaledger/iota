// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This file contains utility functions for the other examples.

use std::{str::FromStr, time::Duration};

use anyhow::bail;
use futures::{future, stream::StreamExt};
use iota_config::{
    Config, IOTA_CLIENT_CONFIG, IOTA_KEYSTORE_FILENAME, PersistedConfig, iota_config_dir,
};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore};
use iota_sdk::{
    IotaClient, IotaClientBuilder,
    iota_client_config::{IotaClientConfig, IotaEnv},
    rpc_types::{
        Coin, IotaObjectDataOptions, IotaTransactionBlockResponse,
        IotaTransactionBlockResponseOptions,
    },
    types::{
        base_types::{IotaAddress, ObjectID},
        crypto::SignatureScheme::ED25519,
        digests::TransactionDigest,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{Argument, Command, Transaction, TransactionData},
    },
    wallet_context::WalletContext,
};
use reqwest::Client;
use serde_json::json;
use shared_crypto::intent::Intent;
use tracing::info;

#[derive(serde::Deserialize)]
struct FaucetResponse {
    task: String,
    error: Option<String>,
}

// const IOTA_FAUCET_BASE_URL: &str = "https://faucet.devnet.iota.cafe"; // devnet faucet

pub const IOTA_FAUCET_BASE_URL: &str = "https://faucet.testnet.iota.cafe"; // testnet faucet

// if you use the `iota start` subcommand and use the local network; if it does
// not work, try with port 5003. const IOTA_FAUCET_BASE_URL: &str = "http://127.0.0.1:9123";

/// Return an iota client to interact with the APIs,
/// the active address of the local wallet, and another address that can be used
/// as a recipient.
///
/// By default, this function will set up a wallet locally if there isn't any,
/// or reuse the existing one and its active address. This function should be
/// used when two addresses are needed, e.g., transferring objects from one
/// address to another.
pub async fn setup_for_write() -> Result<(IotaClient, IotaAddress, IotaAddress), anyhow::Error> {
    let (client, active_address) = setup_for_read().await?;
    // make sure we have some IOTA (5_000_000 NANOS) on this address
    let coin = fetch_coin(&client, &active_address).await?;
    if coin.is_none() {
        request_tokens_from_faucet(active_address, &client).await?;
    }
    let wallet = retrieve_wallet()?;
    let addresses = wallet.get_addresses();
    let addresses = addresses
        .into_iter()
        .filter(|address| address != &active_address)
        .collect::<Vec<_>>();
    let recipient = addresses
        .first()
        .expect("Cannot get the recipient address needed for writing operations. Aborting");

    Ok((client, active_address, *recipient))
}

/// Return an iota client to interact with the APIs and an active address from
/// the local wallet.
///
/// This function sets up a wallet in case there is no wallet locally,
/// and ensures that the active address of the wallet has IOTA on it.
/// If there is no IOTA owned by the active address, then it will request
/// IOTA from the faucet.
pub async fn setup_for_read() -> Result<(IotaClient, IotaAddress), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;
    println!("IOTA testnet version is: {}", client.api_version());
    let wallet = retrieve_wallet()?;
    assert!(wallet.get_addresses().len() >= 2);
    let active_address = wallet.active_address()?;

    println!("Wallet active address is: {active_address}");
    Ok((client, active_address))
}

/// Request tokens from the Faucet for the given address
pub async fn request_tokens_from_faucet(
    address: IotaAddress,
    client: &IotaClient,
) -> Result<(), anyhow::Error> {
    let address_str = address.to_string();
    let json_body = json![{
        "FixedAmountRequest": {
            "recipient": &address_str
        }
    }];

    // make the request to the faucet JSON RPC API for coin
    let reqwest_client = Client::new();
    let resp = reqwest_client
        .post(format!("{IOTA_FAUCET_BASE_URL}/v1/gas"))
        .header("Content-Type", "application/json")
        .json(&json_body)
        .send()
        .await?;
    println!(
        "Faucet request for address {address_str} has status: {}",
        resp.status()
    );
    println!("Waiting for the faucet to complete the gas request...");
    let faucet_resp: FaucetResponse = resp.json().await?;

    let task_id = if let Some(err) = faucet_resp.error {
        bail!("Faucet request was unsuccessful. Error is {err:?}")
    } else {
        faucet_resp.task
    };

    println!("Faucet request task id: {task_id}");

    // wait for the faucet to finish the batch of token requests
    let coin_id = loop {
        let resp = reqwest_client
            .get(format!("{IOTA_FAUCET_BASE_URL}/v1/status/{task_id}"))
            .send()
            .await?;
        let text = resp.text().await?;
        if text.contains("SUCCEEDED") {
            let resp_json: serde_json::Value = serde_json::from_str(&text).unwrap();

            break <&str>::clone(
                &resp_json
                    .pointer("/status/transferred_gas_objects/sent/0/id")
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .to_string();
        } else {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    };

    // wait until the fullnode has the coin object, and check if it has the same
    // owner
    loop {
        let owner = client
            .read_api()
            .get_object_with_options(
                ObjectID::from_str(&coin_id)?,
                IotaObjectDataOptions::new().with_owner(),
            )
            .await?;

        if owner.owner().is_some() {
            let owner_address = owner.owner().unwrap().get_owner_address()?;
            if owner_address == address {
                break;
            }
        } else {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    Ok(())
}

/// Return the coin owned by the address that has at least 5_000_000 NANOS,
/// otherwise returns None
pub async fn fetch_coin(
    client: &IotaClient,
    sender: &IotaAddress,
) -> Result<Option<Coin>, anyhow::Error> {
    let coin_type = "0x2::iota::IOTA".to_string();
    let coins_stream = client.coin_read_api().get_coins_stream(*sender, coin_type);

    let mut coins = coins_stream
        .skip_while(|c| future::ready(c.balance < 5_000_000))
        .boxed();
    let coin = coins.next().await;
    Ok(coin)
}

/// Return a transaction digest from a split coin + merge coins transaction
pub async fn split_coin_digest(
    client: &IotaClient,
    sender: &IotaAddress,
) -> Result<TransactionDigest, anyhow::Error> {
    let coin = match fetch_coin(client, sender).await? {
        None => {
            request_tokens_from_faucet(*sender, client).await?;
            fetch_coin(client, sender)
                .await?
                .expect("Supposed to get a coin with IOTA, but didn't. Aborting")
        }
        Some(c) => c,
    };

    println!(
        "Address: {sender}. The selected coin for split is {} and has a balance of {}\n",
        coin.coin_object_id, coin.balance
    );

    // set the maximum gas budget
    let max_gas_budget = 5_000_000;

    // get the reference gas price from the network
    let gas_price = client.read_api().get_reference_gas_price().await?;

    // now we programmatically build the transaction through several commands
    let mut ptb = ProgrammableTransactionBuilder::new();
    // first, we want to split the coin, and we specify how much IOTA (in NANOS) we
    // want for the new coin
    let split_coin_amount = ptb.pure(1000u64)?; // note that we need to specify the u64 type here
    ptb.command(Command::SplitCoins(
        Argument::GasCoin,
        vec![split_coin_amount],
    ));
    // now we want to merge the coins (so that we don't have many coins with very
    // small values) observe here that we pass Argument::Result(0), which
    // instructs the PTB to get the result from the previous command
    ptb.command(Command::MergeCoins(
        Argument::GasCoin,
        vec![Argument::Result(0)],
    ));

    // we finished constructing our PTB and we need to call finish
    let builder = ptb.finish();

    // using the PTB that we just constructed, create the transaction data
    // that we will submit to the network
    let tx_data = TransactionData::new_programmable(
        *sender,
        vec![coin.object_ref()],
        builder,
        max_gas_budget,
        gas_price,
    );

    let transaction_response = sign_and_execute_transaction(client, sender, tx_data).await?;

    Ok(transaction_response.digest)
}

pub fn retrieve_wallet() -> Result<WalletContext, anyhow::Error> {
    let wallet_conf = iota_config_dir()?.join(IOTA_CLIENT_CONFIG);
    let keystore_path = iota_config_dir()?.join(IOTA_KEYSTORE_FILENAME);

    // check if a wallet exists and if not, create a wallet and an iota client
    // config
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }

    if !wallet_conf.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        let mut client_config = IotaClientConfig::new(keystore);

        client_config.add_env(IotaEnv::testnet());
        client_config.add_env(IotaEnv::devnet());
        client_config.add_env(IotaEnv::localnet());

        if client_config.active_env().is_none() {
            client_config
                .set_active_env(client_config.envs().first().map(|env| env.alias().clone()));
        }

        client_config.save(&wallet_conf)?;
        info!("Client config file is stored in {wallet_conf:?}.");
    }

    let mut keystore = FileBasedKeystore::new(&keystore_path)?;
    let mut client_config: IotaClientConfig = PersistedConfig::read(&wallet_conf)?;

    let default_active_address = if let Some(address) = keystore.addresses().first() {
        *address
    } else {
        keystore
            .generate_and_add_new_key(ED25519, None, None, None)?
            .0
    };

    if keystore.addresses().len() < 2 {
        keystore.generate_and_add_new_key(ED25519, None, None, None)?;
    }

    client_config.set_active_address(default_active_address);
    client_config.save(&wallet_conf)?;

    let wallet = WalletContext::new(&wallet_conf, std::time::Duration::from_secs(60), None)?;

    Ok(wallet)
}

pub async fn sign_and_execute_transaction(
    client: &IotaClient,
    sender: &IotaAddress,
    tx_data: TransactionData,
) -> Result<IotaTransactionBlockResponse, anyhow::Error> {
    let keystore = FileBasedKeystore::new(&iota_config_dir()?.join(IOTA_KEYSTORE_FILENAME))?;
    let signature = keystore.sign_secure(sender, &tx_data, Intent::iota_transaction())?;

    let transaction_block_response = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::full_content(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;

    Ok(transaction_block_response)
}

// this function should not be used. It is only used to make clippy happy,
// and to reduce the number of allow(dead_code) annotations to just this one
#[expect(dead_code)]
async fn just_for_clippy() -> Result<(), anyhow::Error> {
    let (client, sender, _recipient) = setup_for_write().await?;
    let _digest = split_coin_digest(&client, &sender).await?;
    Ok(())
}
