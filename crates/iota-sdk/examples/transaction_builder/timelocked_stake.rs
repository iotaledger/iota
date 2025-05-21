// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to stake timelocked coins.
//!
//! cargo run --example timelocked_stake

#[path = "../utils.rs"]
mod utils;

use iota_json_rpc_types::{IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponseQuery};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use iota_sdk::{
    IotaClientBuilder,
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{quorum_driver_types::ExecuteTransactionRequestType, transaction::Transaction},
};
use iota_types::{
    crypto::SignatureScheme, iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
};
use shared_crypto::intent::Intent;
use utils::request_tokens_from_faucet;

const MNEMONIC_WITH_TIMELOCKED_IOTA: &str = "mesh dose off wage gas tent key light help girl faint catch sock trouble guard moon talk pill enemy hawk gain mix sad mimic";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = IotaClientBuilder::default().build_testnet().await?;

    let mut keystore = Keystore::from(
        FileBasedKeystore::new(&"staking-example.keystore".to_string().into()).unwrap(),
    );
    let address = keystore.import_from_mnemonic(
        MNEMONIC_WITH_TIMELOCKED_IOTA,
        SignatureScheme::ED25519,
        None,
        None,
    )?;
    println!("Sender address: {address:?}");

    request_tokens_from_faucet(address, &client).await?;
    let gas_coin = client
        .coin_read_api()
        .get_coins(address, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .expect("missing gas coin");

    let timelocked_objects = client
        .read_api()
        .get_owned_objects(
            address,
            IotaObjectResponseQuery::new(
                Some(IotaObjectDataFilter::StructType(
                    "0x2::timelock::TimeLock<0x2::balance::Balance<0x2::iota::IOTA>>".parse()?,
                )),
                Some(
                    IotaObjectDataOptions::new()
                        .with_type()
                        .with_owner()
                        .with_previous_transaction(),
                ),
            ),
            None,
            None,
        )
        .await?;

    let timelocked_object = timelocked_objects.data[1].object()?.object_id;
    println!("Timelocked object: {timelocked_object}");

    // Delegate some timelocked IOTAs
    let validator = match client
        .governance_api()
        .get_latest_iota_system_state()
        .await?
    {
        IotaSystemStateSummary::V1(v1) => v1.active_validators[0].clone(),
        IotaSystemStateSummary::V2(v2) => v2.active_validators[0].clone(),
        _ => panic!("unsupported IotaSystemStateSummary"),
    }
    .iota_address;

    let tx_data = client
        .transaction_builder()
        .request_add_timelocked_stake(
            address,
            timelocked_object,
            validator,
            gas_coin.coin_object_id,
            100_000_000,
        )
        .await?;

    let signature = keystore.sign_secure(&address, &tx_data, Intent::iota_transaction())?;

    let transaction_response = client
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_data(tx_data, vec![signature]),
            IotaTransactionBlockResponseOptions::new().with_object_changes(),
            ExecuteTransactionRequestType::WaitForLocalExecution,
        )
        .await?;
    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    // Wait for indexer to process the tx
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Check DelegatedTimelockedStake object
    let staked_iota = client
        .governance_api()
        .get_timelocked_stakes(address)
        .await?;
    println!("Staked: {staked_iota:?}\n\n");

    // Unstake timelocked IOTA, if staking for longer than 1 epoch already

    let current_epoch = client.read_api().get_checkpoints(None, 1, true).await?.data[0].epoch;

    if let Some(timelocked_staked_iota_id) = staked_iota.into_iter().find_map(|d| {
        d.stakes.into_iter().find_map(|s| {
            if s.stake_request_epoch < current_epoch {
                Some(s.timelocked_staked_iota_id)
            } else {
                None
            }
        })
    }) {
        let gas_coin = client
            .coin_read_api()
            .get_coins(address, None, None, None)
            .await?
            .data
            .into_iter()
            .next()
            .expect("missing gas coin");

        let tx_data = client
            .transaction_builder()
            .request_withdraw_timelocked_stake(
                address,
                timelocked_staked_iota_id,
                gas_coin.coin_object_id,
                100_000_000,
            )
            .await?;

        let signature = keystore.sign_secure(&address, &tx_data, Intent::iota_transaction())?;

        let transaction_response = client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(tx_data, vec![signature]),
                IotaTransactionBlockResponseOptions::full_content(),
                ExecuteTransactionRequestType::WaitForLocalExecution,
            )
            .await?;

        println!("Transaction sent {}", transaction_response.digest);
        println!("Object changes:");
        for object_change in transaction_response.object_changes.unwrap() {
            println!("{:?}", object_change);
        }

        // Wait for indexer to process the tx
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        // Check DelegatedTimelockedStake object
        let staked_iota = client
            .governance_api()
            .get_timelocked_stakes(address)
            .await?;
        println!("Staked: {staked_iota:?}");
    } else {
        println!("No stake found that can be unlocked (must be staked >= 1 epoch)")
    };

    // Cleanup
    std::fs::remove_file("staking-example.aliases")?;
    std::fs::remove_file("staking-example.keystore")?;

    Ok(())
}
