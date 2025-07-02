// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use iota_json_rpc_types::CheckpointId;
use iota_light_client::{
    Proof, ProofTargets,
    checkpoint::sync_and_verify_checkpoints,
    config::Config,
    construct_proof,
    object_store::CheckpointStore,
    package_store::RemotePackageStore,
    proof,
    verifier::{get_verified_effects_and_events, get_verified_object},
};
use iota_package_resolver::Resolver;
use iota_rest_api::CheckpointData;
use iota_types::{
    base_types::ObjectID,
    committee::Committee,
    digests::{CheckpointDigest, TransactionDigest},
    event::EventID,
    object::{Data, bounded_visitor::BoundedVisitor},
};
use tracing::{debug, info};

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
    /// Check an object for inclusion
    CheckObject {
        /// Object ID
        #[arg(value_name = "HEX")]
        object_id: ObjectID,
    },
    /// Check a transaction for inclusion
    CheckTransaction {
        /// Transaction digest
        #[arg(value_name = "BASE58")]
        transaction_digest: TransactionDigest,
    },
    /// Construct a proof for events and objects, and write it to a file
    ConstructProof {
        /// Event IDs that should be included in the proof
        #[arg(
            name = "events",
            long,
            value_parser = parse_event_id,
            num_args(1..),
        )]
        event_ids: Vec<EventID>,
        /// Object IDs that should be included in the proof
        #[arg(name = "objects", long, num_args(1..))]
        object_ids: Vec<ObjectID>,
        /// The checkpoint sequence number or checkpoint digest
        #[arg(name = "checkpoint", long, value_parser = parse_checkpoint_id)]
        checkpoint_id: CheckpointId,
        /// The path to the file the proof is written to
        #[arg(name = "output", long, value_name = "PATH")]
        output_file: PathBuf,
    },
    /// Sync the light client
    Sync,
    /// Verify a proof stored in a file against a committee
    VerifyProof {
        /// The checkpoint sequence number or checkpoint digest of an
        /// end-of-epoch checkpoint of the committee to verify the proof
        /// against
        #[arg(name = "checkpoint", long, value_parser = parse_checkpoint_id)]
        checkpoint_id: CheckpointId,
        /// The path to the file the proof is read from
        #[arg(name = "input", long, value_name = "PATH")]
        input_file: PathBuf,
    },
}

fn parse_event_id(s: &str) -> Result<EventID> {
    s.to_string().try_into()
}

fn parse_checkpoint_id(s: &str) -> Result<CheckpointId> {
    if let Ok(seq) = s.parse::<u64>() {
        return Ok(seq.into());
    } else if let Ok(digest) = s.parse::<CheckpointDigest>() {
        return Ok(digest.into());
    } else {
        bail!("invalid checkpoint id");
    }
}

#[tokio::main]
pub async fn main() -> Result<()> {
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
        LightClientCommand::Sync => {
            sync_and_verify_checkpoints(&config)
                .await
                .context("Failed to sync checkpoints")?;
        }
        LightClientCommand::ConstructProof {
            event_ids,
            object_ids,
            checkpoint_id: checkpoint,
            output_file,
        } => {
            let CheckpointId::SequenceNumber(seq) = checkpoint else {
                todo!("convert digest to sequence number");
            };

            let committee: Option<Committee> = None;
            let mut events = Vec::new();
            for event in event_ids {
                info!("Fetching event {event:?}");
            }
            let objects = Vec::new();
            for object in object_ids {
                info!("Fetching object {object}");
            }

            let data = download_checkpoints_from_checkpoint_store(&config, seq).await?;
            let targets = ProofTargets {
                committee,
                events,
                objects,
            };

            let proof = construct_proof(targets, &data)?;

            let file = std::fs::File::create(output_file)?;
            serde_json::to_writer_pretty(file, &proof)?;
        }
        LightClientCommand::VerifyProof {
            checkpoint_id,
            input_file,
        } => {
            let CheckpointId::SequenceNumber(seq) = checkpoint_id else {
                todo!("convert digest to sequence number");
            };
            let data = download_checkpoints_from_checkpoint_store(&config, seq).await?;
            let summary = data.checkpoint_summary;
            let Some(end_of_epoch_data) = &summary.end_of_epoch_data else {
                bail!("not an end-of-epoch checkpoint");
            };

            let next_committee = end_of_epoch_data
                .next_epoch_committee
                .iter()
                .cloned()
                .collect();
            let committee = Committee::new(summary.epoch().checked_add(1).unwrap(), next_committee);

            let file = std::fs::File::open(input_file)?;
            let proof: Proof = serde_json::from_reader(file)?;

            proof::verify_proof(&committee, &proof)?;
        }
    }

    Ok(())
}

pub async fn download_checkpoints_from_checkpoint_store(
    config: &Config,
    seq: u64,
) -> Result<CheckpointData> {
    let checkpoint_store = CheckpointStore::new(config)?;
    info!("Downloading checkpoint: {seq}.chk");

    let data = checkpoint_store
        .fetch_full_checkpoint(seq)
        .await
        .context(format!(
            "Failed to download checkpoint '{seq}' from checkpoint store"
        ))?;

    Ok(data)
}
