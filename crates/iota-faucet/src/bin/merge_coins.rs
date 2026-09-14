// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use iota_config::{IOTA_CLIENT_CONFIG, iota_config_dir};
use iota_faucet::FaucetError;
use iota_grpc_client::{Client as GrpcClient, read_mask_fields::TransactionField};
use iota_keys::keystore::AccountKeystore;
use iota_sdk::wallet_context::WalletContext;
use iota_sdk_types::{
    Address, ObjectId, SignedTransaction, StructTag, Transaction, TransactionEffects,
    crypto::Intent,
};
use iota_types::{gas_coin::GasCoin, transaction::TransactionEnvelope};
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
    let tx = builder.finish_with_budget(50000000000).await?;

    let effects = _sign_and_execute(&client, &wallet, active_address, tx).await?;
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

    // Smash coins togethers 254 objects at a time
    for chunk in small_coins.chunks(254) {
        let total_balance: u64 = chunk.iter().map(|coin| coin.0.balance.value()).sum();

        let mut coin_vector = chunk
            .iter()
            .map(|coin| *coin.id())
            .collect::<Vec<ObjectId>>();

        // prepend big gas coin instance to vector
        coin_vector.insert(0, ObjectId::from_str(gas_coin).unwrap());

        let mut builder = client.transaction_builder(active_address);
        builder.pay(coin_vector, [(active_address, total_balance)]);
        let tx = builder.finish_with_budget(1000000).await?;

        _sign_and_execute(&client, &wallet, active_address, tx).await?;
    }
    Ok(())
}

async fn _sign_and_execute(
    client: &GrpcClient,
    wallet: &WalletContext,
    signer: Address,
    tx: Transaction,
) -> Result<TransactionEffects, anyhow::Error> {
    let signature =
        wallet
            .config()
            .keystore()
            .sign_secure(&signer, &tx, Intent::iota_transaction())?;
    let signed_tx: SignedTransaction = TransactionEnvelope::from_data(tx, vec![signature]).into();
    let executed = client
        .execute_transaction(signed_tx, None, [TransactionField::EFFECTS_BCS])
        .await?
        .into_parts()
        .0;
    Ok(executed.effects()?.effects()?)
}

pub fn create_wallet_context(timeout_secs: u64) -> Result<WalletContext, anyhow::Error> {
    let wallet_conf = iota_config_dir()?.join(IOTA_CLIENT_CONFIG);
    info!("Initialize wallet from config path: {wallet_conf:?}");
    Ok(WalletContext::new(&wallet_conf)?.with_request_timeout(Duration::from_secs(timeout_secs)))
}
