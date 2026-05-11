// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to publish and upgrade a move package.
//!
//! cargo run --example move_package

#[path = "../utils.rs"]
mod utils;

use std::path::PathBuf;

use iota_move_build::BuildConfig;
use iota_sdk::rpc_types::ObjectChange;
use iota_sdk_transaction_builder::{TransactionBuilder, assigned};
use move_package::BuildConfig as MoveBuildConfig;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, _) = setup_for_write().await?;

    let package_path = [
        env!("CARGO_MANIFEST_DIR"),
        "../../examples/move/first_package",
    ]
    .iter()
    .collect::<PathBuf>();

    let build_config = BuildConfig {
        config: MoveBuildConfig {
            default_flavor: Some(move_compiler::editions::Flavor::Iota),
            ..MoveBuildConfig::default()
        },
        run_bytecode_verifier: true,
        print_diags_to_stderr: false,
        chain_id: None,
    };

    let module = build_config.clone().build(&package_path)?;

    let move_package_data = iota_sdk_types::MovePackageData::new(
        module.get_package_bytes(false),
        module.get_dependency_storage_package_ids(),
    );

    let mut builder = TransactionBuilder::new(sender).with_client(&client);
    builder
        .publish(move_package_data)
        .assign("upgrade_cap")
        // Transfer the upgrade cap to the sender address
        .transfer_objects(sender, [assigned("upgrade_cap")]);
    let tx_data = builder.finish().await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    let object_changes = transaction_response.object_changes.unwrap();
    for object_change in &object_changes {
        println!("{object_change:?}");
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
                if object_type.is_upgrade_cap() {
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
    let module = build_config.build(&package_path)?;
    let deps = module.get_dependency_storage_package_ids();
    let package_bytes = module.get_package_bytes(false);

    let move_package_data = iota_sdk_types::MovePackageData::new(package_bytes, deps);

    let mut builder = TransactionBuilder::new(sender).with_client(client.clone());
    builder
        // Authorize the upgrade by providing the upgrade cap object id to receive an upgrade
        // ticket
        .move_call(
            iota_sdk_types::Address::FRAMEWORK,
            "package",
            "authorize_upgrade",
        )
        .arguments((
            upgrade_capability,
            iota_sdk_types::UpgradePolicy::Compatible as u8,
            move_package_data.digest,
        ))
        .assign("upgrade_ticket")
        // Upgrade the package to receive an upgrade receipt
        .upgrade(package_id, move_package_data, assigned("upgrade_ticket"))
        .assign("upgrade_receipt")
        // Commit the upgrade using the receipt
        .move_call(
            iota_sdk_types::Address::FRAMEWORK,
            "package",
            "commit_upgrade",
        )
        .arguments((upgrade_capability, assigned("upgrade_receipt")));

    let tx_data = builder.finish().await?;

    let transaction_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    println!("Transaction sent {}", transaction_response.digest);
    println!("Object changes:");
    for object_change in transaction_response.object_changes.unwrap() {
        println!("{object_change:?}");
    }

    Ok(())
}
