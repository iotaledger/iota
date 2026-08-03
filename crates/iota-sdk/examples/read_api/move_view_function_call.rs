// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to publish a package containing `#[view]`
//! functions and call them with the `iota_view` JSON-RPC method, including a
//! generic view called with a type argument.
//!
//! A function can only be called through `iota_view` if it is declared with
//! the `#[view]` attribute and is recorded in its module's on-chain view
//! functions metadata, which is recorded on devnet or a local network, not yet
//! on testnet or mainnet.
//!
//! By default it runs against devnet. Pass `--localnet` to fund a fresh wallet
//! from the local faucet, or `--devnet` / `--testnet` to use the configured
//! wallet (assumed funded — the public faucets have no HTTP API):
//!
//! cargo run --example move_view_function_call -- --localnet

#[path = "../utils.rs"]
mod utils;

use std::{path::PathBuf, str::FromStr};

use iota_json::IotaJsonValue;
use iota_json_rpc_api::WriteApiClient;
use iota_move_build::{BuildConfig, ProtocolBuildConfig};
use iota_sdk::{
    rpc_types::{IotaTransactionBlockEffectsAPI, IotaTypeTag, ObjectChange},
    types::{
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{TransactionData, TransactionDataAPI},
    },
};
use iota_sdk_types::{Argument, Command, Identifier, Owner, TypeTag};
use move_package::BuildConfig as MoveBuildConfig;
use utils::{setup_for_write_with_network, sign_and_execute_transaction};

use crate::utils::Network;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let network = match std::env::args().nth(1).as_deref() {
        None | Some("--devnet") => Network::Devnet,
        Some("--localnet") => Network::Localnet,
        Some("--testnet") => Network::Testnet,
        Some(other) => {
            anyhow::bail!("unknown flag {other}; use --localnet, --devnet, or --testnet")
        }
    };
    let (client, sender, _) = setup_for_write_with_network(network).await?;

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
        anyhow::bail!(
            "publishing the package failed: {:?}",
            publish_response.effects.map(|e| e.into_status())
        );
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

    let view_call_results = client
        .http()
        .view_function_call(
            format!("{package_id}::counter::value"),
            None,
            vec![IotaJsonValue::new(serde_json::json!(counter_id))?],
        )
        .await?;
    println!("{view_call_results:?}");

    // The `vault` module is generic over the type `T` it stores. Build a shared
    // `Vault<Coin<IOTA>>` so that a generic view has something to read.
    //
    // `create` takes the item by value, so the stored coin cannot be an existing
    // shared object. Instead, split a coin off the gas payment and hand that
    // split result straight to `create` in a single transaction.
    let gas_price = client.read_api().get_reference_gas_price().await?;
    let gas_coin = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .expect("missing gas coin");

    let mut ptb = ProgrammableTransactionBuilder::new();
    // Split 1000 NANOS off the gas coin to store in the vault.
    let split_amount = ptb.pure(1_000u64)?;
    ptb.command(Command::new_split_coins(Argument::Gas, vec![split_amount]));
    // unlock_at: a Unix timestamp (ms); unused by the `item` view.
    let unlock_at = ptb.pure(0u64)?;
    // beneficiary: the only address allowed to unlock the vault.
    let beneficiary = ptb.pure(sender)?;
    ptb.programmable_move_call(
        package_id,
        Identifier::new("vault")?,
        Identifier::new("create")?,
        // `T = Coin<IOTA>`, the type argument the view must also be called with.
        vec![TypeTag::from_str("0x2::coin::Coin<0x2::iota::IOTA>")?],
        // The split coin (result of the first command), then the two pure args.
        vec![Argument::Result(0), unlock_at, beneficiary],
    );
    let create_vault_tx = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        ptb.finish(),
        gas_budget,
        gas_price,
    );
    let create_vault_response =
        sign_and_execute_transaction(&client, &sender, create_vault_tx).await?;
    let vault_id = create_vault_response
        .object_changes
        .unwrap()
        .iter()
        .find_map(|change| match change {
            ObjectChange::Created {
                object_id,
                owner: Owner::Shared(_),
                ..
            } => Some(*object_id),
            _ => None,
        })
        .expect("missing shared vault object");

    // Call the generic `vault::item` view, filling in the type argument
    // (`Coin<IOTA>`) and the object argument (the vault's ID).
    let vault_view_results = client
        .http()
        .view_function_call(
            format!("{package_id}::vault::item"),
            Some(vec![IotaTypeTag::new(
                "0x2::coin::Coin<0x2::iota::IOTA>".to_string(),
            )]),
            vec![IotaJsonValue::new(serde_json::json!(vault_id))?],
        )
        .await?;
    println!("{vault_view_results:?}");

    Ok(())
}
