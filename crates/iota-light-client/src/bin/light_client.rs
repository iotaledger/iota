// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand};
use iota_light_client::{
    checkpoint::sync_and_check_checkpoints,
    config::Config,
    package_store::RemotePackageStore,
    verifier::{get_verified_effects_and_events, get_verified_object},
};
use iota_package_resolver::Resolver;
use iota_types::{
    base_types::ObjectID,
    digests::TransactionDigest,
    object::{Data, bounded_visitor::BoundedVisitor},
};
use tracing::debug;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

// A light client for the IOTA blockchain
#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    author,
    version = VERSION,
    propagate_version = true,
)]
struct Args {
    /// Sets a custom config file
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: LightClientCommand,
}

#[derive(Subcommand, Debug)]
pub enum LightClientCommand {
    /// Sync light client
    Sync,
    /// Check a transaction for inclusion
    CheckTransaction {
        /// Transaction digest
        #[arg(value_name = "BASE58")]
        transaction_digest: String,
    },
    /// Check an object for inclusion
    CheckObject {
        /// Object ID
        #[arg(value_name = "HEX")]
        object_id: String,
    },
}

#[tokio::main]
pub async fn main() {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_log_level("info")
        .with_env()
        .init();

    let args = Args::parse();

    let path = args
        .config
        .unwrap_or_else(|| panic!("Need a config file path"));
    let config = Config::load(&path)
        .unwrap_or_else(|e| panic!("Unable to load config from {}: {e}", path.display()));

    let remote_package_store = RemotePackageStore::new(config.clone());
    let resolver = Resolver::new(remote_package_store);

    debug!("IOTA Light Client CLI version: {VERSION}");
    match args.command {
        LightClientCommand::CheckTransaction { transaction_digest } => {
            if config.sync_before_check {
                sync_and_check_checkpoints(&config)
                    .await
                    .expect("Failed to sync checkpoints");
            }

            let (effects, events) = get_verified_effects_and_events(
                &config,
                TransactionDigest::from_str(&transaction_digest).unwrap(),
            )
            .await
            .unwrap();

            let exec_digests = effects.execution_digests();
            println!(
                "Executed Digest: {} Effects: {}",
                exec_digests.transaction, exec_digests.effects
            );

            if events.is_some() {
                for event in events.as_ref().unwrap().data.iter() {
                    let type_layout = resolver
                        .type_layout(event.type_.clone().into())
                        .await
                        .unwrap();

                    let result = BoundedVisitor::deserialize_value(&event.contents, &type_layout)
                        .expect("Cannot deserialize");

                    println!(
                        "Event:\n - Package: {}\n - Module: {}\n - Sender: {}\n - Type: {}\n{}",
                        event.package_id,
                        event.transaction_module,
                        event.sender,
                        event.type_,
                        serde_json::to_string_pretty(&result).unwrap()
                    );
                }
            } else {
                println!("No events found");
            }
        }
        LightClientCommand::CheckObject { object_id } => {
            if config.sync_before_check {
                sync_and_check_checkpoints(&config)
                    .await
                    .expect("Failed to sync checkpoints");
            }

            let object_id = ObjectID::from_str(&object_id).unwrap();
            let object = get_verified_object(&config, object_id).await.unwrap();
            println!("Successfully verified object: {}", object_id);

            if let Data::Move(move_object) = &object.data {
                let object_type = move_object.type_().clone();

                let type_layout = resolver
                    .type_layout(object_type.clone().into())
                    .await
                    .unwrap();

                let result =
                    BoundedVisitor::deserialize_value(move_object.contents(), &type_layout)
                        .expect("Cannot deserialize");

                let (oid, version, hash) = object.compute_object_reference();
                println!(
                    "OID: {}\n - Version: {}\n - Hash: {}\n - Owner: {}\n - Type: {}\n{}",
                    oid,
                    version,
                    hash,
                    object.owner,
                    object_type,
                    serde_json::to_string_pretty(&result).unwrap()
                );
            }
        }
        LightClientCommand::Sync => {
            sync_and_check_checkpoints(&config)
                .await
                .expect("Failed to sync checkpoints");
        }
    }
}
