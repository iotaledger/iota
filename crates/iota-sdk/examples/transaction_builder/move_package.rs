// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to publish and upgrade a move package.
//!
//! cargo run --example move_package

#[path = "../utils.rs"]
mod utils;

use std::path::Path;

use iota_json_rpc_types::ObjectChange;
use iota_move_build::BuildConfig;
use iota_types::move_package::UpgradeCap;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, _) = setup_for_write().await?;

    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin_object_id = coins.data[0].coin_object_id;

    let gas_budget = 50_000_000;

    let package_path = Path::new("../../examples/move/first_package");
    let module = BuildConfig::default().build(package_path)?;

    let tx_data = client
        .transaction_builder()
        .publish(
            sender,
            module.get_package_bytes(false),
            module.get_dependency_storage_package_ids(),
            gas_coin_object_id,
            gas_budget,
        )
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    let object_changes = transaction_response.object_changes.unwrap();
    for object_change in &object_changes {
        println!("{:?}", object_change);
    }

    // Wait some time for the indexer to process the tx
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Upgrade

    let package_id = object_changes
        .iter()
        .find_map(|c| {
            if let ObjectChange::Published { .. } = c {
                Some(c.object_id())
            } else {
                None
            }
        })
        .expect("missing published package");
    let upgrade_capability = object_changes
        .iter()
        .find_map(|c| {
            if let ObjectChange::Created { object_type, .. } = c {
                if object_type == &UpgradeCap::type_() {
                    Some(c.object_id())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .expect("missing upgrade cap");

    // In reality you would like to do some changes to the package before upgrading
    let module = BuildConfig::default().build(package_path)?;
    let deps = module.get_dependency_storage_package_ids();
    let package_bytes = module.get_package_bytes(false);

    let tx_data = client
        .transaction_builder()
        .upgrade(
            sender,
            package_id,
            package_bytes,
            deps,
            upgrade_capability,
            0,
            gas_coin_object_id,
            gas_budget,
        )
        .await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{:?}", object_change);
    }

    Ok(())
}
