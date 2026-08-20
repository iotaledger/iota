// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to call a `#[view]` function over gRPC, using the
//! `TransactionExecutionService.ViewFunctionCalls` endpoint via the high-level
//! `iota_grpc_client::Client::view_function_call` helper.
//!
//! A function can only be called this way if it is declared with the `#[view]`
//! attribute and recorded in its module's on-chain view functions metadata.
//!
//! Arguments are passed as JSON and encoded by the node against the parameter's
//! Move type. Integers go over the wire as strings, because a protobuf
//! `Value` number is a double and cannot represent every integer exactly.
//!
//! It runs against a local network that has the gRPC API enabled. Start one
//! first, for example:
//!
//! iota-localnet start --force-regenesis --with-faucet --with-grpc
//!
//! then run:
//!
//! cargo run --example grpc_view_function_call

#[path = "../utils.rs"]
mod utils;

use std::path::PathBuf;

use iota_grpc_client::{Client as GrpcClient, read_mask_fields::ViewFunctionCallReadMask};
use iota_move_build::{BuildConfig, ProtocolBuildConfig};
use iota_sdk::rpc_types::ObjectChange;
use iota_sdk_types::Owner;
use move_package::BuildConfig as MoveBuildConfig;
use utils::{Network, setup_for_write_with_network, sign_and_execute_transaction};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Publish the `view_functions` package on the local network over JSON-RPC.
    // See the `move_view_function_call` example for a walk-through of this part.
    let (client, sender, _) = setup_for_write_with_network(Network::Localnet).await?;

    let gas_coin_object_id = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data[0]
        .coin_object_id;
    let gas_budget = 500_000_000;

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
            max_move_package_size: None,
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
    if publish_response.status_ok() != Some(true) {
        anyhow::bail!("publishing the package failed");
    }

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

    // Call `counter::value(counter)` over gRPC. The shared counter is passed by
    // its object id as a JSON string.
    let grpc = GrpcClient::new_localnet()?;
    let outputs = grpc
        .view_function_call(
            &format!("{package_id}::counter::value"),
            &[],
            &[serde_json::json!(counter_id)],
            ViewFunctionCallReadMask::default(),
        )
        .await?;

    // The call ran either way; `execution_error` says whether it aborted.
    match outputs.body().return_values() {
        Some(values) => {
            for output in &values.outputs {
                println!("counter::value returned: {:?}", output.json);
            }
        }
        None => println!("view call aborted: {:?}", outputs.body().execution_error()),
    }

    Ok(())
}
