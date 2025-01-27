// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::Arc,
};

use anyhow::anyhow;
use async_trait::async_trait;
use clap::Subcommand;
use iota_config::genesis::Genesis;
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use iota_package_resolver::{Package, PackageStore, Result as ResolverResult};
use iota_rest_api::{CheckpointData, Client};
use iota_sdk::IotaClientBuilder;
use iota_types::{
    base_types::ObjectID,
    committee::Committee,
    crypto::AuthorityQuorumSignInfo,
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    message_envelope::Envelope,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSummary, EndOfEpochData},
    object::Object,
};
use move_core_types::account_address::AccountAddress;

pub async fn check_and_sync_checkpoints(config: &Config) -> anyhow::Result<()> {
    sync_checkpoint_list_to_latest(config).await?;

    // Get the local checkpoint list
    let checkpoints_list: CheckpointsList = read_checkpoint_list_from_config(config)?;

    // Load the genesis committee
    let mut genesis_path = config.checkpoint_summary_dir.clone();
    genesis_path.push(&config.genesis_filename);
    let genesis_committee = Genesis::load(&genesis_path)?.committee()?;

    // Check the signatures of all checkpoints
    // And download any missing ones

    let mut prev_committee = genesis_committee;
    for ckp_id in &checkpoints_list.checkpoints {
        // check if there is a file with this name ckp_id.yaml in the
        // checkpoint_summary_dir
        let mut checkpoint_path = config.checkpoint_summary_dir.clone();
        checkpoint_path.push(format!("{}.json", ckp_id));
        // If file exists read the file otherwise download it from the server
        let summary = if checkpoint_path.exists() {
            read_checkpoint(config, *ckp_id)?
        } else {
            // Download the checkpoint from the server
            let summary = download_checkpoint_summary(config, *ckp_id).await?;
            summary.clone().try_into_verified(&prev_committee)?;
            // Write the checkpoint summary to a file
            write_checkpoint(config, &summary)?;
            summary
        };

        // Print the id of the checkpoint and the epoch number
        println!(
            "Epoch: {} Checkpoint ID: {}",
            summary.epoch(),
            summary.digest()
        );

        // Extract the new committee information
        if let Some(EndOfEpochData {
            next_epoch_committee,
            ..
        }) = &summary.end_of_epoch_data
        {
            let next_committee = next_epoch_committee.iter().cloned().collect();
            prev_committee =
                Committee::new(summary.epoch().checked_add(1).unwrap(), next_committee);
        } else {
            return Err(anyhow!(
                "Expected all checkpoints to be end-of-epoch checkpoints"
            ));
        }
    }

    Ok(())
}

pub fn write_checkpoint(
    config: &Config,
    summary: &Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>,
) -> anyhow::Result<()> {
    write_checkpoint_general(config, summary, None)
}

pub fn write_checkpoint_general(
    config: &Config,
    summary: &Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>,
    path: Option<&str>,
) -> anyhow::Result<()> {
    // Write the checkpoint summary to a file
    let mut checkpoint_path = config.checkpoint_summary_dir.clone();
    if let Some(path) = path {
        checkpoint_path.push(path);
    }
    checkpoint_path.push(format!("{}.json", summary.sequence_number));
    let writer = fs::File::create(checkpoint_path.clone())?;
    serde_json::to_writer(&writer, &summary)
        .map_err(|_| anyhow!("Unable to serialize checkpoint summary"))?;
    Ok(())
}

pub fn write_checkpoint_list(
    config: &Config,
    checkpoints_list: &CheckpointsList,
) -> anyhow::Result<()> {
    // Write the checkpoint list to a file
    let mut checkpoints_path = config.checkpoint_summary_dir.clone();
    checkpoints_path.push("checkpoints.yaml");
    let mut writer = fs::File::create(checkpoints_path.clone())?;
    let mut bytes = Vec::new();
    serde_yaml::to_writer(&mut bytes, &checkpoints_list)?;
    writer
        .write_all(&bytes)
        .map_err(|_| anyhow!("Unable to serialize checkpoint list"))
}

pub async fn download_checkpoint_summary(
    config: &Config,
    seq: u64,
) -> anyhow::Result<CertifiedCheckpointSummary> {
    // Download the checkpoint from the server
    let client = Client::new(config.rest_url());
    Ok(client.get_checkpoint_summary(seq).await?)
}

/// Run binary search to for each end of epoch checkpoint that is missing
/// between the latest on the list and the latest checkpoint.
pub async fn sync_checkpoint_list_to_latest(config: &Config) -> anyhow::Result<()> {
    // Get the local checkpoint list
    let mut checkpoints_list: CheckpointsList = read_checkpoint_list_from_config(config)?;
    let mut last_epoch = 0;
    let mut last_checkpoint_seq = 0;

    if let Some(latest_in_list) = checkpoints_list.checkpoints.last() {
        // Download the latest in list checkpoint
        let summary = download_checkpoint_summary(config, *latest_in_list).await?;
        last_epoch = summary.epoch();
        last_checkpoint_seq = summary.sequence_number;
    }

    // Download the very latest checkpoint
    let client = Client::new(config.rest_url());
    let latest = client.get_latest_checkpoint().await?;

    println!("last_epoch: {last_epoch}, latest_epoch: {}", latest.epoch());

    // Check if the local node has enough data
    if last_epoch >= latest.epoch() {
        return Err(anyhow!(
            "Local node does not have enough data: last_epoch: {last_epoch} >= latest_epoch: {}",
            latest.epoch()
        ));
    }

    println!("Do a binary search to find missing checkpoints...");

    // Binary search to find missing checkpoints
    while last_epoch + 1 < latest.epoch() {
        let mut start = last_checkpoint_seq;
        let mut end = latest.sequence_number;

        let target_epoch = last_epoch + 1;
        // Print target
        println!("Target Epoch: {}", target_epoch);
        let mut found_summary = None;

        while start < end {
            let mid = (start + end) / 2;
            let summary = download_checkpoint_summary(config, mid).await?;

            // print summary epoch and seq
            println!(
                "Epoch: {} Seq: {}: {}",
                summary.epoch(),
                summary.sequence_number,
                summary.end_of_epoch_data.is_some()
            );

            if summary.epoch() == target_epoch && summary.end_of_epoch_data.is_some() {
                found_summary = Some(summary);
                break;
            }

            if summary.epoch() <= target_epoch {
                start = mid + 1;
            } else {
                end = mid;
            }
        }

        if let Some(summary) = found_summary {
            // Note: Do not write summary to file, since we must only persist
            //       checkpoints that have been verified by the previous committee

            // Add to the list
            checkpoints_list.checkpoints.push(summary.sequence_number);
            write_checkpoint_list(config, &checkpoints_list)?;

            // Update
            last_epoch = summary.epoch();
            last_checkpoint_seq = summary.sequence_number;
        }
    }

    Ok(())
}

pub async fn get_full_checkpoint(config: &Config, seq: u64) -> anyhow::Result<CheckpointData> {
    // Downloading the checkpoint from the server
    let client: Client = Client::new(config.rest_url());
    let full_checkpoint = client.get_full_checkpoint(seq).await?;

    Ok(full_checkpoint)
}

pub fn extract_verified_effects_and_events(
    checkpoint: &CheckpointData,
    committee: &Committee,
    tid: TransactionDigest,
) -> anyhow::Result<(TransactionEffects, Option<TransactionEvents>)> {
    let summary = &checkpoint.checkpoint_summary;

    // Verify the checkpoint summary using the committee
    summary.verify_with_contents(committee, Some(&checkpoint.checkpoint_contents))?;

    // Check the validity of the transaction
    let contents = &checkpoint.checkpoint_contents;
    let (matching_tx, _) = checkpoint
        .transactions
        .iter()
        .zip(contents.iter())
        // Note that we get the digest of the effects to ensure this is
        // indeed the correct effects that are authenticated in the contents.
        .find(|(tx, digest)| {
            tx.effects.execution_digests() == **digest && digest.transaction == tid
        })
        .ok_or(anyhow!("Transaction not found in checkpoint contents"))?;

    // Check the events are all correct.
    let events_digest = matching_tx.events.as_ref().map(|events| events.digest());
    anyhow::ensure!(
        events_digest.as_ref() == matching_tx.effects.events_digest(),
        "Events digest does not match"
    );

    // Since we do not check objects we do not return them
    Ok((matching_tx.effects.clone(), matching_tx.events.clone()))
}

pub async fn get_verified_effects_and_events(
    config: &Config,
    tid: TransactionDigest,
) -> anyhow::Result<(TransactionEffects, Option<TransactionEvents>)> {
    let iota_mainnet: Arc<iota_sdk::IotaClient> = Arc::new(
        IotaClientBuilder::default()
            .build(config.full_node_url.as_str())
            .await
            .unwrap(),
    );
    let read_api = iota_mainnet.read_api();

    // Lookup the transaction id and get the checkpoint sequence number
    let options = IotaTransactionBlockResponseOptions::new();
    let seq = read_api
        .get_transaction_with_options(tid, options)
        .await?
        .checkpoint
        .ok_or(anyhow!("Transaction not found"))?;

    // Download the full checkpoint for this sequence number
    let full_check_point = get_full_checkpoint(config, seq).await?;

    // Load the list of stored checkpoints
    let checkpoints_list: CheckpointsList = read_checkpoint_list_from_config(config)?;

    // find the stored checkpoint before the seq checkpoint
    let prev_ckp_id = checkpoints_list
        .checkpoints
        .iter()
        .filter(|ckp_id| **ckp_id < seq)
        .last();

    let committee = if let Some(prev_ckp_id) = prev_ckp_id {
        // Read it from the store
        let prev_ckp = read_checkpoint(config, *prev_ckp_id)?;

        // Check we have the right checkpoint
        anyhow::ensure!(
            prev_ckp.epoch().checked_add(1).unwrap() == full_check_point.checkpoint_summary.epoch(),
            "Checkpoint sequence number does not match. Need to Sync."
        );

        // Get the committee from the previous checkpoint
        let current_committee = prev_ckp
            .end_of_epoch_data
            .as_ref()
            .ok_or(anyhow!(
                "Expected all checkpoints to be end-of-epoch checkpoints"
            ))?
            .next_epoch_committee
            .iter()
            .cloned()
            .collect();

        // Make a committee object using this
        Committee::new(prev_ckp.epoch().checked_add(1).unwrap(), current_committee)
    } else {
        // Since we did not find a small committee checkpoint we use the genesis
        let mut genesis_path = config.checkpoint_summary_dir.clone();
        genesis_path.push(&config.genesis_filename);
        Genesis::load(&genesis_path)?.committee()?
    };

    extract_verified_effects_and_events(&full_check_point, &committee, tid)
}

pub async fn get_verified_object(config: &Config, id: ObjectID) -> anyhow::Result<Object> {
    let client: Client = Client::new(config.rest_url());
    let object = client.get_object(id).await?;

    // Need to authenticate this object
    let (effects, _) = get_verified_effects_and_events(config, object.previous_transaction).await?;

    // check that this object ID, version and hash is in the effects
    effects
        .all_changed_objects()
        .iter()
        .find(|object_ref| object_ref.0 == object.compute_object_reference())
        .ok_or(anyhow!("Object not found"))?;

    Ok(object)
}

pub struct RemotePackageStore {
    config: Config,
}

impl RemotePackageStore {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl PackageStore for RemotePackageStore {
    /// Read package contents. Fails if `id` is not an object, not a package, or
    /// is malformed in some way.
    async fn fetch(&self, id: AccountAddress) -> ResolverResult<Arc<Package>> {
        let object = get_verified_object(&self.config, id.into()).await.unwrap();
        let package = Package::read_from_object(&object).unwrap();
        Ok(Arc::new(package))
    }
}

#[derive(Subcommand, Debug)]
pub enum SCommands {
    /// Sync all end-of-epoch checkpoints
    Sync {},

    /// Checks a specific transaction using the light client
    Transaction {
        /// Transaction hash
        #[arg(short, long, value_name = "TID")]
        tid: String,
    },

    /// Checks a specific object using the light client
    Object {
        /// Transaction hash
        #[arg(short, long, value_name = "OID")]
        oid: String,
    },
}

// The config file for the light client including the root of trust genesis
// digest
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Config {
    /// Full node url
    full_node_url: String,

    /// Checkpoint summary directory
    checkpoint_summary_dir: PathBuf,

    //  Genesis file name
    genesis_filename: PathBuf,
}

impl Config {
    pub fn rest_url(&self) -> &str {
        &self.full_node_url
    }

    pub fn full_node_url(&self) -> &str {
        &self.full_node_url
    }

    pub fn checkpoint_summary_dir(&self) -> &PathBuf {
        &self.checkpoint_summary_dir
    }
}

// The list of checkpoints at the end of each epoch
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CheckpointsList {
    // List of end of epoch checkpoints
    pub checkpoints: Vec<u64>,
}

pub fn read_checkpoint_list(file_path: PathBuf) -> anyhow::Result<CheckpointsList> {
    // Read the resulting file and parse the yaml checkpoint list
    let reader = fs::File::open(&file_path)?;
    Ok(serde_yaml::from_reader(reader)?)
}

pub fn read_checkpoint_list_from_config(config: &Config) -> anyhow::Result<CheckpointsList> {
    let mut checkpoints_path = config.checkpoint_summary_dir.clone();
    checkpoints_path.push("checkpoints.yaml");
    read_checkpoint_list(checkpoints_path)
}

pub fn read_checkpoint(
    config: &Config,
    seq: u64,
) -> anyhow::Result<Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>> {
    read_checkpoint_general(config, seq, None)
}

pub fn read_checkpoint_general(
    config: &Config,
    seq: u64,
    path: Option<&str>,
) -> anyhow::Result<Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>> {
    // Read the resulting file and parse the yaml checkpoint list
    let mut checkpoint_path = config.checkpoint_summary_dir.clone();
    if let Some(path) = path {
        checkpoint_path.push(path);
    }
    checkpoint_path.push(format!("{}.chk", seq));
    let mut reader = fs::File::open(checkpoint_path.clone())?;
    let metadata = fs::metadata(&checkpoint_path)?;
    let mut buffer = vec![0; metadata.len() as usize];
    reader.read_exact(&mut buffer)?;
    bcs::from_bytes(&buffer).map_err(|_| anyhow!("Unable to parse checkpoint file"))
}
