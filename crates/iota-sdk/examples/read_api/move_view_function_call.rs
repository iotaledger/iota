// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to publish a package containing a `#[view]`
//! function and call it with the `iota_view` JSON-RPC method.
//!
//! A function can only be called through `iota_view` if it is declared with
//! the `#[view]` attribute and is recorded in its module's on-chain view
//! functions metadata. The metadata is only recorded on networks with view
//! function support enabled; at the time of writing this is not yet the case
//! on testnet, so run this example against a local network (see the faucet
//! URL notes in `utils.rs`) or devnet instead.
//!
//! cargo run --example move_view_function_call

#[path = "../utils.rs"]
mod utils;

use std::path::PathBuf;

use iota_json::IotaJsonValue;
use iota_json_rpc_api::WriteApiClient;
use iota_move_build::{BuildConfig, ProtocolBuildConfig};
use iota_sdk::rpc_types::ObjectChange;
use iota_sdk_types::Owner;
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
        "../../examples/move/view_functions",
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

    let view_call_results = client
        .http()
        .view_function_call(
            format!("{package_id}::counter::value"),
            None,
            vec![IotaJsonValue::new(serde_json::json!(counter_id))?],
        )
        .await?;
    println!("{view_call_results:?}");

    Ok(())
}
