// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use iota_config::{IOTA_CLIENT_CONFIG, iota_config_dir};
use iota_faucet::FaucetError;
use iota_keys::keystore::AccountKeystore;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_transaction_builder::WaitForTransaction;
use iota_sdk_types::{ObjectId, StructTag};
use iota_types::gas_coin::GasCoin;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let wallet = create_wallet_context(60)?;
    let active_address = wallet
        .active_address()
        .map_err(|err| FaucetError::Wallet(err.to_string()))?;
    println!("SimpleFaucet::new with active address: {active_address}");

    // Example scripts
    // merge_coins(
    //     "0x0215b800acc47d80a50741f0eecfa507fc2c21f5a9aa6140a219686ad20d7f4c",
    //     wallet,
    // )
    // .await?;

    // split_coins_equally(
    //     "0xd42a75242975780037e170486540f28ab3c9be07dbb1f6f2a9430ad268e3b1d1",
    //     wallet,
    //     1000,
    // )
    // .await?;

    Ok(())
}

async fn _split_coins_equally(
    gas_coin: &str,
    wallet: WalletContext,
    count: u64,
) -> Result<(), anyhow::Error> {
    let active_address = wallet
        .active_address()
        .map_err(|err| FaucetError::Wallet(err.to_string()))?;
    let client = wallet.get_grpc_client().await?;
    let coin_object_id = ObjectId::from_str(gas_coin).unwrap();

    let mut builder = client.transaction_builder(active_address);
    let count = builder.pure(count);
    builder
        .move_call(ObjectId::FRAMEWORK, "pay", "divide_and_keep")
        .type_tags([StructTag::new_gas().into()])
        .arguments((coin_object_id, count));
    builder.gas_budget(50000000000);

    let signer = wallet.config().keystore().get_key(&active_address)?;
    let effects = builder
        .execute(signer.as_keypair()?, WaitForTransaction::Finalized)
        .await?;
    println!("{effects:?}");
    Ok(())
}

async fn _merge_coins(gas_coin: &str, wallet: WalletContext) -> Result<(), anyhow::Error> {
    let active_address = wallet
        .active_address()
        .map_err(|err| FaucetError::Wallet(err.to_string()))?;
    let client = wallet.get_grpc_client().await?;
    // Pick a gas coin here that isn't in use by the faucet otherwise there will be
    // some contention.
    let small_coins = wallet
        .gas_objects(active_address)
        .await
        .map_err(|e| FaucetError::Wallet(e.to_string()))?
        .iter()
        // Ok to unwrap() since `get_gas_objects` guarantees gas
        .map(|q| GasCoin::try_from(&q.1).unwrap())
        // Everything less than 1 iota
        .filter(|coin| coin.0.balance.value() <= 10000000000)
        .collect::<Vec<GasCoin>>();

    let signer = wallet.config().keystore().get_key(&active_address)?;

    // Smash coins togethers 254 objects at a time
    for chunk in small_coins.chunks(254) {
        let total_balance: u64 = chunk.iter().map(|coin| coin.0.balance.value()).sum();

        let mut coin_vector = chunk
            .iter()
            .map(|coin| *coin.id())
            .collect::<Vec<ObjectId>>();

        // prepend big gas coin instance to vector
        coin_vector.insert(0, ObjectId::from_str(gas_coin).unwrap());

        // The coins pay for the transaction, so that gas smashing is what merges
        // them, and the whole balance is split back to the sender.
        let mut builder = client.transaction_builder(active_address);
        builder
            .pay_iota([(active_address, total_balance)])
            .gas(coin_vector);
        builder.gas_budget(1000000);

        builder
            .execute(signer.as_keypair()?, WaitForTransaction::Finalized)
            .await?;
    }
    Ok(())
}

pub fn create_wallet_context(timeout_secs: u64) -> Result<WalletContext, anyhow::Error> {
    let wallet_conf = iota_config_dir()?.join(IOTA_CLIENT_CONFIG);
    info!("Initialize wallet from config path: {wallet_conf:?}");
    Ok(WalletContext::new(&wallet_conf)?.with_request_timeout(Duration::from_secs(timeout_secs)))
}
