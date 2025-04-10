// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    sync::Arc,
};

use anyhow::{Result, anyhow, bail};
use getset::Getters;
use iota_archival::read_manifest;
use iota_config::genesis::Genesis;
use iota_rest_api::Client;
use iota_sdk::IotaClientBuilder;
use iota_storage::object_store::{ObjectStoreGetExt, http::HttpDownloaderBuilder};
use iota_types::{
    committee::Committee,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::{CertifiedCheckpointSummary, EndOfEpochData},
};
use object_store::path::Path;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    config::Config,
    graphql::query_last_checkpoint_of_epoch,
    object_store::{IotaObjectStore, ObjectStoreExt},
};

// The list of checkpoints at the end of each epoch
#[derive(Debug, Clone, Default, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CheckpointList {
    // List of end of epoch checkpoints
    checkpoints: Vec<u64>,
}

impl CheckpointList {
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }
}

pub fn read_checkpoint_list(config: &Config) -> Result<CheckpointList> {
    let checkpoints_path = config.checkpoints_list_file_path();
    let reader = fs::File::open(checkpoints_path)?;
    Ok(serde_yaml::from_reader(reader)?)
}

pub fn write_checkpoint_list(config: &Config, checkpoints_list: &CheckpointList) -> Result<()> {
    let checkpoints_path = config.checkpoints_list_file_path();
    let mut writer = fs::File::create(checkpoints_path)?;
    let bytes = serde_yaml::to_vec(checkpoints_list)?;
    writer
        .write_all(&bytes)
        .map_err(|e| anyhow!("Unable to serialize checkpoint list: {}", e))
}

pub fn read_checkpoint_summary(config: &Config, seq: u64) -> Result<CertifiedCheckpointSummary> {
    read_checkpoint_summary_general(config, seq, None)
}

fn read_checkpoint_summary_general(
    config: &Config,
    seq: u64,
    path: Option<&str>,
) -> Result<CertifiedCheckpointSummary> {
    let checkpoint_path = config.checkpoint_summary_file_path(seq, path);
    let mut reader = fs::File::open(checkpoint_path)?;
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    bcs::from_bytes(&buffer).map_err(|_| anyhow!("Unable to parse checkpoint file"))
}

pub fn write_checkpoint_summary(
    config: &Config,
    summary: &CertifiedCheckpointSummary,
) -> Result<()> {
    write_checkpoint_summary_general(config, summary, None)
}

fn write_checkpoint_summary_general(
    config: &Config,
    summary: &CertifiedCheckpointSummary,
    path: Option<&str>,
) -> Result<()> {
    let checkpoint_path = config.checkpoint_summary_file_path(*summary.sequence_number(), path);
    let mut writer = fs::File::create(checkpoint_path)?;
    let bytes =
        bcs::to_bytes(summary).map_err(|_| anyhow!("Unable to serialize checkpoint summary"))?;
    writer.write_all(&bytes)?;
    Ok(())
}

/// Downloads the list of end of epoch checkpoints from the archive store or the
/// GraphQL endpoint
pub async fn sync_checkpoint_list_to_latest(config: &Config) -> anyhow::Result<CheckpointList> {
    // Check if we have any source configured
    if config.graphql_url.is_none() && config.archive_store_config.is_none() {
        bail!(
            "No checkpoint sources configured - both GraphQL URL and Archive Store config are missing"
        );
    }

    // Try getting checkpoints from object store or full node (fallback).
    // In both cases we need a GraphQL endpoint to be configured
    let graphql_list = if config.graphql_url.is_some() {
        if config.object_store_url.is_some() {
            match sync_checkpoint_list_to_latest_using_object_store(config).await {
                Ok(list) => list,
                Err(e) => {
                    warn!("Failed to sync checkpoints from object store: {e}");
                    CheckpointList::default()
                }
            }
        } else {
            // Fall back to the full node REST, RPC and GraphQL endpoints
            match sync_checkpoint_list_to_latest_using_full_node(config).await {
                Ok(list) => list,
                Err(e) => {
                    warn!("Failed to sync checkpoints from full node: {e}");
                    CheckpointList::default()
                }
            }
        }
    } else {
        CheckpointList::default()
    };

    // Try getting checkpoints from archive store if configured
    let archive_list = if config.archive_store_config.is_some() {
        match sync_checkpoint_list_to_latest_using_archive(config).await {
            Ok(list) => list,
            Err(e) => {
                warn!("Failed to sync checkpoints from archive store: {e}");
                CheckpointList::default()
            }
        }
    } else {
        CheckpointList::default()
    };

    // Verify we have at least some checkpoints
    if graphql_list.checkpoints.is_empty() && archive_list.checkpoints.is_empty() {
        bail!("Could not retrieve any checkpoints from configured sources");
    }

    let merged_checkpoints = merge_checkpoint_lists(&graphql_list, &archive_list);
    let checkpoints_list = CheckpointList {
        checkpoints: merged_checkpoints,
    };

    // Write the fetched checkpoint list to disk
    write_checkpoint_list(config, &checkpoints_list)?;

    Ok(checkpoints_list)
}

/// Merges two checkpoint lists, removing duplicates and ensuring the result is
/// sorted
fn merge_checkpoint_lists(list1: &CheckpointList, list2: &CheckpointList) -> Vec<u64> {
    // Combine both lists into a HashSet to remove duplicates
    let unique_checkpoints: HashSet<u64> = list1
        .checkpoints
        .iter()
        .chain(list2.checkpoints.iter())
        .copied()
        .collect();

    // Convert to sorted vector
    let mut sorted_checkpoints: Vec<_> = unique_checkpoints.into_iter().collect();
    sorted_checkpoints.sort();

    sorted_checkpoints
}

/// Downloads the list of end of epoch checkpoints from the full node's RPC and
/// GraphQL endpoints
async fn sync_checkpoint_list_to_latest_using_full_node(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoints from full node");

    // Get the local checkpoint list, or create an empty one if it doesn't exist
    let mut checkpoints_list = match read_checkpoint_list(config) {
        Ok(list) => list,
        Err(e) => {
            info!("Could not read existing checkpoint list, starting with empty list: {e}");
            CheckpointList::default()
        }
    };

    // Get the last synced checkpoint sequence number, or fetch the first
    let last_seq = if let Some(last_seq) = checkpoints_list.checkpoints.last() {
        *last_seq
    } else {
        let last_seq = query_last_checkpoint_of_epoch(config, 0).await?;
        checkpoints_list.checkpoints.push(last_seq);
        info!("Synced epoch: 0, checkpoint: {last_seq}",);
        last_seq
    };

    // Download the checkpoint from the node
    let rest_client = Client::new(config.full_node_url.as_str());

    // Download the latest in list checkpoint
    let last_sum = rest_client.get_checkpoint_summary(last_seq).await?;

    // Download the very latest checkpoint
    let client = IotaClientBuilder::default()
        .build(config.full_node_url.as_str())
        .await?;

    let latest_seq = client
        .read_api()
        .get_latest_checkpoint_sequence_number()
        .await?;
    let latest_sum = rest_client.get_checkpoint_summary(latest_seq).await?;

    // Sequentially record all the missing end of epoch checkpoints numbers
    for target_epoch in (last_sum.epoch() + 1)..latest_sum.epoch() {
        let target_seq = query_last_checkpoint_of_epoch(config, target_epoch).await?;
        checkpoints_list.checkpoints.push(target_seq);
        info!("Synced epoch: {target_epoch}, checkpoint: {target_seq}");
    }

    Ok(checkpoints_list)
}

/// Downloads the list of end of epoch checkpoints from the archive store
/// (archiving node)
async fn sync_checkpoint_list_to_latest_using_archive(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoints from archive store");

    let Some(archive_store_config) = &config.archive_store_config else {
        bail!("Archive store config is not provided");
    };

    let archive_store: Arc<dyn ObjectStoreGetExt> = if archive_store_config.no_sign_request {
        archive_store_config.make_http()?
    } else {
        Arc::new(archive_store_config.make()?)
    };

    let manifest = read_manifest(archive_store).await?;
    let checkpoints = manifest.get_all_end_of_epoch_checkpoint_seq_numbers()?;

    Ok(CheckpointList { checkpoints })
}

/// Downloads the list of end of epoch checkpoints from the object store
async fn sync_checkpoint_list_to_latest_using_object_store(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoints from object store");

    // Get the local checkpoint list, or create an empty one if it doesn't exist
    let mut checkpoints_list = match read_checkpoint_list(config) {
        Ok(list) => list,
        Err(e) => {
            info!("Could not read existing checkpoint list, starting with empty list: {e}");
            CheckpointList::default()
        }
    };

    // Get the last synced checkpoint sequence number, or fetch the first
    let last_seq = if let Some(last_seq) = checkpoints_list.checkpoints.last() {
        *last_seq
    } else {
        // TODO try to fetch the first checkpoint from the object store instead of
        // the full node which might no longer have it
        let last_seq = query_last_checkpoint_of_epoch(config, 0).await?;
        checkpoints_list.checkpoints.push(last_seq);
        info!("Synced epoch: 0, checkpoint: {last_seq}",);
        last_seq
    };

    let object_store = IotaObjectStore::new(config)?;

    let last_sum = object_store.get_checkpoint_summary(last_seq).await?;

    let client = IotaClientBuilder::default()
        .build(config.full_node_url.as_str())
        .await?;

    let latest_seq = client
        .read_api()
        .get_latest_checkpoint_sequence_number()
        .await?;
    let latest_sum = object_store.get_checkpoint_summary(latest_seq).await?;

    // Sequentially record all the missing end of epoch checkpoints numbers
    for target_epoch in (last_sum.epoch() + 1)..latest_sum.epoch() {
        let target_seq = query_last_checkpoint_of_epoch(config, target_epoch).await?;
        checkpoints_list.checkpoints.push(target_seq);
        info!("Synced epoch: {target_epoch}, checkpoint: {target_seq}");
    }

    Ok(checkpoints_list)
}

pub async fn sync_and_check_checkpoints(config: &Config) -> anyhow::Result<()> {
    let checkpoints_list = sync_checkpoint_list_to_latest(config)
        .await
        .map_err(|e| anyhow!(format!("Failed to sync checkpoint list: {e}")))?;

    // Load the genesis committee
    let genesis_committee = Genesis::load(config.genesis_blob_file_path())?
        .committee()
        .map_err(|e| anyhow!(format!("Failed to load genesis file: {e}")))?;

    // Check the signatures of all checkpoints and download any missing ones
    let mut prev_committee = genesis_committee;
    for seq in checkpoints_list.checkpoints {
        // Check if there is a corresponding checkpoint summary file in the checkpoints
        // directory
        let summary_path = config.checkpoint_summary_file_path(seq, None);

        // If file exists read the file otherwise download it from the server
        let summary = if summary_path.exists() {
            read_checkpoint_summary(config, seq)
                .map_err(|e| anyhow!(format!("Failed to read checkpoint summary: {e}")))?
        } else {
            let summary = if let Some(archive_store_config) = &config.archive_store_config {
                // Try downloading it from the archive
                let archive_store: Arc<dyn ObjectStoreGetExt> =
                    if archive_store_config.no_sign_request {
                        archive_store_config.make_http()?
                    } else {
                        Arc::new(archive_store_config.make()?)
                    };
                let checkpoint_summary_file_path = Path::from(format!("{seq}.chk"));
                let bytes = archive_store
                    .get_bytes(&checkpoint_summary_file_path)
                    .await?;
                let (_, full_checkpoint) = bcs::from_bytes::<(u8, CheckpointData)>(&bytes)?;
                full_checkpoint.checkpoint_summary
            } else if config.object_store_url.is_some() {
                // Try downloading the checkpoint summary from the object store
                IotaObjectStore::new(config)?
                    .get_checkpoint_summary(seq)
                    .await
                    .map_err(|e| {
                        anyhow!(format!(
                            "Failed to download checkpoint summary from object store: {e}"
                        ))
                    })?
            } else {
                // Try downloading it from the node via REST API
                let client = Client::new(config.full_node_url.as_str());
                client.get_checkpoint_summary(seq).await.map_err(|e| {
                    anyhow!("Failed to download checkpoint summary from full node: {e}")
                })?
            };

            // Write the checkpoint summary to a file
            write_checkpoint_summary(config, &summary)?;
            summary
        };

        // Verify the checkpoint
        summary.clone().try_into_verified(&prev_committee)?;

        info!(
            "Verified epoch: {}, checkpoint: {seq}, checkpoint digest: {}",
            summary.epoch(),
            summary.digest()
        );

        // Extract the next committee information
        if let Some(EndOfEpochData {
            next_epoch_committee,
            ..
        }) = &summary.end_of_epoch_data
        {
            let next_committee = next_epoch_committee.iter().cloned().collect();
            prev_committee =
                Committee::new(summary.epoch().checked_add(1).unwrap(), next_committee);
        } else {
            bail!("Expected all checkpoints to be end-of-epoch checkpoints");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_types::{
        crypto::AuthorityQuorumSignInfo,
        gas::GasCostSummary,
        message_envelope::Envelope,
        messages_checkpoint::{CheckpointContents, CheckpointSummary},
        supported_protocol_versions::ProtocolConfig,
    };
    use roaring::RoaringBitmap;
    use tempfile::TempDir;

    use super::*;

    fn create_test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            checkpoints_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        (config, temp_dir)
    }

    #[test]
    fn test_checkpoint_list_read_write() {
        let (config, _temp_dir) = create_test_config();
        let test_list = CheckpointList {
            checkpoints: vec![1, 2, 3],
        };

        write_checkpoint_list(&config, &test_list).unwrap();
        let read_list = read_checkpoint_list(&config).unwrap();

        assert_eq!(test_list.checkpoints, read_list.checkpoints);
    }

    #[test]
    fn test_checkpoint_read_write() {
        let (config, _temp_dir) = create_test_config();
        let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
        let summary = CheckpointSummary::new(
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            0,
            0,
            0,
            &contents,
            None,
            GasCostSummary::default(),
            None,
            0,
            Vec::new(),
        );
        let info = AuthorityQuorumSignInfo::<true> {
            epoch: 0,
            signature: Default::default(),
            signers_map: RoaringBitmap::new(),
        };
        let test_summary = Envelope::new_from_data_and_sig(summary, info);

        write_checkpoint_summary(&config, &test_summary).unwrap();
        let read_summary = read_checkpoint_summary(&config, 0).unwrap();

        assert_eq!(
            test_summary.sequence_number(),
            read_summary.sequence_number()
        );
    }
}
