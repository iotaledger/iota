// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf, str::FromStr};

use clap::Parser;
use iota_json::IotaJsonValue;
use iota_light_client::utils::{
    Config, RemotePackageStore, SCommands, check_and_sync_checkpoints,
    get_verified_effects_and_events, get_verified_object,
};
use iota_package_resolver::Resolver;
use iota_types::{base_types::ObjectID, digests::TransactionDigest, object::Data};

/// A light client for the Iota blockchain
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<SCommands>,
}

#[tokio::main]
pub async fn main() {
    // Command line arguments and config loading
    let args = Args::parse();
    let path = args
        .config
        .unwrap_or_else(|| panic!("Need a config file path"));

    let reader = fs::File::open(path.clone())
        .unwrap_or_else(|_| panic!("Unable to load config from {}", path.display()));
    let config: Config = serde_yaml::from_reader(reader).unwrap();

    // Print config parameters
    println!(
        "Checkpoint Dir: {}",
        config.checkpoint_summary_dir().display()
    );

    let remote_package_store = RemotePackageStore::new(config.clone());
    let resolver = Resolver::new(remote_package_store);

    match args.command {
        Some(SCommands::Transaction { tid }) => {
            let (effects, events) = get_verified_effects_and_events(
                &config,
                TransactionDigest::from_str(&tid).unwrap(),
            )
            .await
            .unwrap();

            let exec_digests = effects.execution_digests();
            println!(
                "Executed TID: {} Effects: {}",
                exec_digests.transaction, exec_digests.effects
            );

            for event in events.as_ref().unwrap().data.iter() {
                let type_layout = resolver
                    .type_layout(event.type_.clone().into())
                    .await
                    .unwrap();

                let json_val =
                    IotaJsonValue::from_bcs_bytes(Some(&type_layout), &event.contents).unwrap();

                println!(
                    "Event:\n - Package: {}\n - Module: {}\n - Sender: {}\n - Type: {}\n{}",
                    event.package_id,
                    event.transaction_module,
                    event.sender,
                    event.type_,
                    serde_json::to_string_pretty(&json_val.to_json_value()).unwrap()
                );
            }
        }
        Some(SCommands::Object { oid }) => {
            let oid = ObjectID::from_str(&oid).unwrap();
            let object = get_verified_object(&config, oid).await.unwrap();

            if let Data::Move(move_object) = &object.data {
                let object_type = move_object.type_().clone();

                let type_layout = resolver
                    .type_layout(object_type.clone().into())
                    .await
                    .unwrap();

                let json_val =
                    IotaJsonValue::from_bcs_bytes(Some(&type_layout), move_object.contents())
                        .unwrap();

                let (oid, version, hash) = object.compute_object_reference();
                println!(
                    "OID: {}\n - Version: {}\n - Hash: {}\n - Owner: {}\n - Type: {}\n{}",
                    oid,
                    version,
                    hash,
                    object.owner,
                    object_type,
                    serde_json::to_string_pretty(&json_val.to_json_value()).unwrap()
                );
            }
        }

        Some(SCommands::Sync {}) => {
            check_and_sync_checkpoints(&config)
                .await
                .expect("Failed to sync checkpoints");
        }
        _ => {}
    };
}

// Make a test namespace
#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use anyhow::anyhow;
    use iota_light_client::utils::extract_verified_effects_and_events;
    use iota_rest_api::CheckpointData;
    use iota_types::{
        committee::Committee,
        crypto::AuthorityQuorumSignInfo,
        message_envelope::Envelope,
        messages_checkpoint::{CheckpointSummary, FullCheckpointContents},
    };

    use super::*;

    async fn read_full_checkpoint(checkpoint_path: &PathBuf) -> anyhow::Result<CheckpointData> {
        Ok(bcs::from_reader(fs::File::open(checkpoint_path)?)?)
    }

    // clippy ignore dead-code
    #[allow(dead_code)]
    async fn write_full_checkpoint(
        checkpoint_path: &Path,
        checkpoint: &CheckpointData,
    ) -> anyhow::Result<()> {
        bcs::serialize_into(&mut fs::File::create(checkpoint_path)?, &checkpoint)?;
        Ok(())
    }

    async fn read_data() -> (Committee, CheckpointData) {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("example_config/534.json");

        let checkpoint: Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>> =
            serde_json::from_reader(&fs::File::open(&d).unwrap())
                .map_err(|_| anyhow!("Unable to parse checkpoint summary file"))
                .unwrap();

        let prev_committee = checkpoint
            .end_of_epoch_data
            .as_ref()
            .ok_or(anyhow!(
                "Expected all checkpoints to be end-of-epoch checkpoints"
            ))
            .unwrap()
            .next_epoch_committee
            .iter()
            .cloned()
            .collect();

        // Make a committee object using this
        let committee = Committee::new(checkpoint.epoch().checked_add(1).unwrap(), prev_committee);

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("example_config/800.chk");

        let full_checkpoint = read_full_checkpoint(&d).await.unwrap();

        (committee, full_checkpoint)
    }

    #[tokio::test]
    async fn test_checkpoint_all_good() {
        let (committee, full_checkpoint) = read_data().await;

        extract_verified_effects_and_events(
            &full_checkpoint,
            &committee,
            TransactionDigest::from_str("DpSZVwqohRzF7ASz7PMM8xL1ZkMnNvWMLJfjadt9ybE9").unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_checkpoint_bad_committee() {
        let (mut committee, full_checkpoint) = read_data().await;

        // Change committee
        committee.epoch += 10;

        assert!(
            extract_verified_effects_and_events(
                &full_checkpoint,
                &committee,
                TransactionDigest::from_str("DpSZVwqohRzF7ASz7PMM8xL1ZkMnNvWMLJfjadt9ybE9")
                    .unwrap(),
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn test_checkpoint_no_transaction() {
        let (committee, full_checkpoint) = read_data().await;

        assert!(
            extract_verified_effects_and_events(
                &full_checkpoint,
                &committee,
                TransactionDigest::from_str("6ciKBJF3gZ2zNNKLqwRMBL4ftZRGL2kHPgKepWP2thbs")
                    .unwrap(),
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn test_checkpoint_bad_contents() {
        let (committee, mut full_checkpoint) = read_data().await;

        // Change contents
        let random_contents = FullCheckpointContents::random_for_testing();
        full_checkpoint.checkpoint_contents = random_contents.checkpoint_contents();

        assert!(
            extract_verified_effects_and_events(
                &full_checkpoint,
                &committee,
                TransactionDigest::from_str("9AoR24Tcmss7K3DgBZYiUZNxHFuk8kdAbZEMcFe9mcAi")
                    .unwrap(),
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn test_checkpoint_bad_events() {
        let (committee, mut full_checkpoint) = read_data().await;

        let event = full_checkpoint.transactions[1]
            .events
            .as_ref()
            .unwrap()
            .data[0]
            .clone();

        for t in &mut full_checkpoint.transactions {
            if let Some(events) = &mut t.events {
                events.data.push(event.clone());
            }
        }

        assert!(
            extract_verified_effects_and_events(
                &full_checkpoint,
                &committee,
                TransactionDigest::from_str("Hj7mZdET3fKqxbSnbdcVbx9N2pqvHtkoKbPy6MEeFsfB")
                    .unwrap(),
            )
            .is_err()
        );
    }
}
