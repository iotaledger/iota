// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to publish a package containing a `#[view]`
//! function and call it using the transaction builder and dev-inspect.
//!
//! A function can only be called through `iota_view` if it is declared with
//! the `#[view]` attribute and is recorded in its module's on-chain view
//! functions metadata. This requires the network to have view function
//! support enabled; at the time of writing this is not yet the case on
//! testnet, so run this example against a local network (see the faucet URL
//! notes in `utils.rs`) or devnet instead.
//!
//! cargo run --example move_view_function_call

#[path = "../utils.rs"]
mod utils;

use std::path::PathBuf;

use iota_json::IotaJsonValue;
use iota_json_rpc_types::{DevInspectResults, IotaTypeTag};
use iota_move_build::{BuildConfig, ProtocolBuildConfig};
use iota_sdk::{IotaClient, rpc_types::ObjectChange};
use iota_sdk_types::{Address, ObjectId, Owner};
use move_package::BuildConfig as MoveBuildConfig;
use utils::{setup_for_write, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (client, sender, _) = setup_for_write().await?;

    let gas_coin_object_id = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data[0]
        .coin_object_id;
    let gas_budget = 50_000_000;

    let package_path = [
        env!("CARGO_MANIFEST_DIR"),
        "../../examples/move/view_function_example",
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
        // Compile the `#[view]` attribute into the module's runtime metadata.
        protocol_build_config: ProtocolBuildConfig {
            allow_view_function: true,
        },
    };
    let compiled_package = build_config.build(&package_path)?;

    let tx_data = client
        .transaction_builder()
        .publish(
            sender,
            compiled_package.get_package_bytes(false),
            compiled_package.get_dependency_storage_package_ids(),
            gas_coin_object_id,
            gas_budget,
        )
        .await?;
    let publish_response = sign_and_execute_transaction(&client, &sender, tx_data).await?;

    let object_changes = publish_response.object_changes.unwrap();
    let package_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .expect("missing published package");
    let counter_id = object_changes
        .iter()
        .find_map(|change| match change {
            ObjectChange::Created {
                object_id,
                owner: Owner::Shared(_),
                ..
            } => Some(*object_id),
            _ => None,
        })
        .expect("missing shared counter object");

    let view_call_results = move_view_function_dev_inspect(
        sender,
        &client,
        package_id,
        "counter",
        "value",
        vec![],
        vec![IotaJsonValue::new(serde_json::json!(counter_id))?],
    )
    .await?
    .results;
    println!("{view_call_results:?}");

    Ok(())
}

async fn move_view_function_dev_inspect(
    sender: Address,
    client: &IotaClient,
    package_id: ObjectId,
    module_name: &str,
    function_name: &str,
    type_args: Vec<IotaTypeTag>,
    args: Vec<IotaJsonValue>,
) -> Result<DevInspectResults, anyhow::Error> {
    let pt = client
        .transaction_builder()
        .move_view_call_tx_kind(package_id, module_name, function_name, type_args, args)
        .await?;

    Ok(client
        .read_api()
        .dev_inspect_transaction_block(sender, pt, None, None, None)
        .await?)
}
