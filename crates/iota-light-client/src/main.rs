// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use iota_light_client::{
    checkpoint::sync_and_verify_checkpoints,
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

#[derive(Parser, Debug)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    author,
    version = VERSION,
    propagate_version = true,
)]
struct Args {
    /// Uses a specific config file, otherwise defaults to the mainnet config
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
        transaction_digest: TransactionDigest,
    },
    /// Check an object for inclusion
    CheckObject {
        /// Object ID
        #[arg(value_name = "HEX")]
        object_id: ObjectID,
    },
}

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_log_level("info")
        .with_env()
        .init();

    let args = Args::parse();

    let config = if let Some(path) = args.config {
        Config::load(&path).await.context(format!(
            "Failed to load custom config '{}'.",
            path.display()
        ))?
    } else {
        Config::get_mainnet_config()
    };

    config.setup().await?;

    let remote_package_store = RemotePackageStore::new(config.clone());
    let resolver = Resolver::new(remote_package_store);

    debug!("IOTA Light Client CLI version: {VERSION}");

    match args.command {
        LightClientCommand::CheckTransaction { transaction_digest } => {
            if config.sync_before_check {
                sync_and_verify_checkpoints(&config)
                    .await
                    .context("Failed to sync checkpoints")?;
            }

            let (effects, events) =
                get_verified_effects_and_events(&config, transaction_digest).await?;

            let exec_digests = effects.execution_digests();
            println!(
                "Executed Digest: {} Effects: {}",
                exec_digests.transaction, exec_digests.effects
            );

            if let Some(events) = &events {
                for event in &events.data {
                    let type_layout = resolver.type_layout(event.type_.clone().into()).await?;

                    let result = BoundedVisitor::deserialize_value(&event.contents, &type_layout)
                        .context("Failed to deserialize event")?;

                    println!(
                        "Event:\n - Package: {}\n - Module: {}\n - Sender: {}\n - Type: {}\n{}",
                        event.package_id,
                        event.transaction_module,
                        event.sender,
                        event.type_,
                        serde_json::to_string(&result).expect("json deserialization error")
                    );
                }
            } else {
                println!("No events found");
            }
        }
        LightClientCommand::CheckObject { object_id } => {
            if config.sync_before_check {
                sync_and_verify_checkpoints(&config)
                    .await
                    .context("Failed to sync checkpoints")?;
            }

            let object = get_verified_object(&config, object_id).await?;
            println!("Successfully verified object: {object_id}");

            if let Data::Move(move_object) = &object.data {
                let object_type = move_object.type_().clone();

                let type_layout = resolver.type_layout(object_type.clone().into()).await?;

                let result =
                    BoundedVisitor::deserialize_value(move_object.contents(), &type_layout)
                        .context("Failed to deserialize object")?;

                let (object_id, version, hash) = object.compute_object_reference();
                println!(
                    "ObjectID: {object_id}\n - Version: {version}\n - Hash: {hash}\n - Owner: {}\n - Type: {object_type}\n{}",
                    object.owner,
                    serde_json::to_string(&result).expect("json deserialization error")
                );
            }
        }
        LightClientCommand::Sync => {
            sync_and_verify_checkpoints(&config)
                .await
                .context("Failed to sync checkpoints")?;
        }
    }

    Ok(())
}
