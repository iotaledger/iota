// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to publish and upgrade a move package.
//!
//! cargo run --example move_package

mod utils;

use iota_json_rpc_types::ObjectChange;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, _) = setup_for_write().await?;

    let coins = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?;
    let gas_coin_object_id = coins.data[0].coin_object_id;

    let gas_budget = 10_000_000;

    // Bytes below are for this example module
    // module example::example {
    //    entry fun example_num(): u64 {
    //        1337
    //     }
    // }
    let tx_data = client
        .transaction_builder()
        .publish(
            sender,
            vec![vec![
                161, 28, 235, 11, 6, 0, 0, 0, 6, 1, 0, 2, 3, 2, 5, 5, 7, 3, 7, 10, 20, 8, 30, 32,
                12, 62, 16, 0, 0, 0, 1, 0, 1, 0, 0, 1, 3, 7, 101, 120, 97, 109, 112, 108, 101, 11,
                101, 120, 97, 109, 112, 108, 101, 95, 110, 117, 109, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 2,
                6, 57, 5, 0, 0, 0, 0, 0, 0, 2, 0,
            ]],
            vec!["0x1".parse()?, "0x2".parse()?],
            Some(gas_coin_object_id),
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
            if let ObjectChange::Created { .. } = c {
                Some(c.object_id())
            } else {
                None
            }
        })
        .expect("missing upgrade cap");

    // Same as above, just returns 1338 now
    let compiled_modules = vec![vec![
        161, 28, 235, 11, 6, 0, 0, 0, 6, 1, 0, 2, 3, 2, 5, 5, 7, 3, 7, 10, 20, 8, 30, 32, 12, 62,
        16, 0, 0, 0, 1, 0, 1, 0, 0, 1, 3, 7, 101, 120, 97, 109, 112, 108, 101, 11, 101, 120, 97,
        109, 112, 108, 101, 95, 110, 117, 109, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 2, 6, 58, 5, 0, 0, 0, 0, 0, 0,
        2, 0,
    ]];
    let deps = vec!["0x1".parse()?, "0x2".parse()?];

    let package_digest = iota_types::move_package::MovePackage::compute_digest_for_modules_and_deps(
        &compiled_modules,
        &deps,
        true,
    );
    let tx_data = client
        .transaction_builder()
        .upgrade(
            sender,
            package_id,
            compiled_modules,
            deps,
            upgrade_capability,
            0,
            package_digest.to_vec(),
            Some(gas_coin_object_id),
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
