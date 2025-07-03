// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, path::PathBuf};

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use iota_config::genesis::Genesis;
use iota_json_rpc_types::CheckpointId;
use iota_light_client::{
    Proof, ProofTargets,
    checkpoint::{read_checkpoint_list, read_checkpoint_summary, sync_and_verify_checkpoints},
    config::Config,
    construct_proof, graphql,
    object_store::CheckpointStore,
    package_store::RemotePackageStore,
    proof,
    verifier::{get_verified_effects_and_events, get_verified_object},
};
use iota_package_resolver::Resolver;
use iota_rest_api::CheckpointData;
use iota_sdk::IotaClientBuilder;
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
        /// Events that should be included in the proof
        #[arg(
            name = "events",
            long,
            value_parser = parse_event_id,
            num_args(0..),
        )]
        event_ids: Vec<EventID>,
        /// Objects that should be included in the proof
        #[arg(name = "objects", long, num_args(0..))]
        object_ids: Vec<ObjectID>,
        /// Whether to include the next committee in the proof
        #[arg(long, default_value_t = false)]
        include_committee: bool,
        /// The checkpoint given by either its sequence number or its digest
        #[arg(name = "checkpoint", long, value_parser = parse_checkpoint_id)]
        checkpoint_id: CheckpointId,
        /// The path to the file the proof is written to
        #[arg(name = "proof", value_name = "PATH")]
        proof_file: PathBuf,
    },
    /// Sync the light client
    Sync,
    /// Verify a proof stored in a file
    VerifyProof {
        /// The path to the file the proof is read from
        #[arg(name = "proof", value_name = "PATH")]
        proof_file: PathBuf,
    },
}

fn parse_event_id(s: &str) -> Result<EventID> {
    s.to_string().try_into()
}

fn parse_checkpoint_id(s: &str) -> Result<CheckpointId> {
    if let Ok(seq) = s.parse::<u64>() {
        Ok(seq.into())
    } else if let Ok(digest) = s.parse::<CheckpointDigest>() {
        Ok(digest.into())
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
            include_committee,
            checkpoint_id,
            proof_file: output_file,
        } => {
            ensure!(
                !event_ids.is_empty() || !object_ids.is_empty() || include_committee,
                "missing proof targets"
            );

            let client = IotaClientBuilder::default()
                .build(config.rpc_url.as_str())
                .await?;
            let read_api = client.read_api();

            // determine the checkpoint sequence number
            let seq = match checkpoint_id {
                CheckpointId::SequenceNumber(seq) => seq,
                CheckpointId::Digest(_) => {
                    let checkpoint = read_api.get_checkpoint(checkpoint_id).await?;
                    checkpoint.sequence_number
                }
            };

            // download the full checkpoint to scan transactions
            let checkpoint = download_checkpoints_from_checkpoint_store(&config, seq).await?;

            // add event and object targets
            let mut event_ids_map: HashSet<EventID> = event_ids.iter().cloned().collect();
            let mut object_ids_map: HashSet<ObjectID> = object_ids.iter().cloned().collect();
            let mut committee: Option<Committee> = None;
            let mut events = Vec::new();
            let mut objects = Vec::new();
            for tx in &checkpoint.transactions {
                // add event ID targets
                if let Some(tx_events) = &tx.events {
                    let tx_digest = *tx.transaction.digest();
                    // TODO: make sure this is the correct way to get the event sequence number
                    for (event_seq, event) in tx_events.data.iter().cloned().enumerate() {
                        let event_id = (tx_digest, event_seq as u64).into();
                        if event_ids.contains(&event_id) {
                            event_ids_map.remove(&event_id);
                            events.push((event_id, event));
                        }
                    }
                }
                // add object ID targets
                for obj in &tx.output_objects {
                    if object_ids.contains(&obj.id()) {
                        let obj_ref = obj.compute_object_reference();
                        object_ids_map.remove(&obj_ref.0);
                        objects.push((obj_ref, obj.clone()));
                    }
                }
            }
            ensure!(
                event_ids_map.is_empty(),
                "not all provided events could be added to the proof for checkpoint {seq}"
            );
            ensure!(
                object_ids_map.is_empty(),
                "not all provided objects could be added to the proof for checkpoint {seq}"
            );

            // add the committee of the next epoch as a proof target
            if include_committee {
                // fetch the end-of-epoch checkpoint; this will fail if the epoch has not ended
                // yet
                let epoch = checkpoint.checkpoint_summary.epoch;
                let end_of_epoch_sequence_number =
                    graphql::query_last_checkpoint_of_epoch(&config, epoch).await?;
                let end_of_epoch_checkpoint = download_checkpoints_from_checkpoint_store(
                    &config,
                    end_of_epoch_sequence_number,
                )
                .await?;
                let next_committee = end_of_epoch_checkpoint
                    .checkpoint_summary
                    .into_data()
                    .end_of_epoch_data
                    .unwrap()
                    .next_epoch_committee;

                committee.replace(Committee::new(
                    epoch + 1,
                    next_committee.into_iter().collect(),
                ));
            }

            let targets = ProofTargets {
                committee,
                events,
                objects,
            };

            let proof = construct_proof(targets, &checkpoint)?;

            let file = std::fs::File::create(&output_file)?;
            serde_json::to_writer_pretty(file, &proof)?;

            println!("Created proof and written to {}.", output_file.display());
        }
        LightClientCommand::VerifyProof {
            proof_file: input_file,
        } => {
            if config.sync_before_check {
                sync_and_verify_checkpoints(&config)
                    .await
                    .context("Failed to sync checkpoints")?;
            }

            let file = std::fs::File::open(&input_file)?;
            let proof: Proof = serde_json::from_reader(file)?;

            let seq = proof.checkpoint_summary.sequence_number;

            // read the summary from the local checkpoints dir
            let summary = read_checkpoint_summary(&config, seq)?.into_data();

            let checkpoint_list = read_checkpoint_list(&config)?;
            let Some(index) = checkpoint_list.get_index(seq) else {
                bail!("invalid checkpoint list");
            };

            let current_committee = if index == 0 {
                Genesis::load(config.genesis_blob_file_path())?.committee()?
            } else {
                let Some(prev_seq) = checkpoint_list.get(index - 1) else {
                    bail!("invalid checkpoint list");
                };
                let prev_summary = read_checkpoint_summary(&config, prev_seq)?.into_data();
                let authorities = prev_summary
                    .end_of_epoch_data
                    .as_ref()
                    .unwrap()
                    .next_epoch_committee
                    .iter()
                    .cloned()
                    .collect();

                Committee::new(summary.epoch, authorities)
            };

            proof::verify_proof(&current_committee, &proof)?;

            println!("The proof stored in {} is valid.", input_file.display())
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
