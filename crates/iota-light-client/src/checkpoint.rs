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
    crypto::AuthorityQuorumSignInfo,
    message_envelope::Envelope,
    messages_checkpoint::{CheckpointSummary, EndOfEpochData},
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    config::Config, graphql::query_last_checkpoint_of_epoch, object_store::IotaObjectStore,
    utils::download_checkpoint_summary_from_object_store_with_fallback,
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
}

pub fn read_checkpoint_list(config: &Config) -> Result<CheckpointList> {
    let checkpoints_path = config.checkpoint_list_path();
    let reader = fs::File::open(checkpoints_path)?;
    Ok(serde_yaml::from_reader(reader)?)
}

pub fn write_checkpoint_list(config: &Config, checkpoints_list: &CheckpointList) -> Result<()> {
    let checkpoints_path = config.checkpoint_list_path();
    let mut writer = fs::File::create(checkpoints_path)?;
    let bytes = serde_yaml::to_vec(checkpoints_list)?;
    writer
        .write_all(&bytes)
        .map_err(|e| anyhow!("Unable to serialize checkpoint list: {}", e))
}

pub fn read_checkpoint(
    config: &Config,
    seq: u64,
) -> Result<Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>> {
    read_checkpoint_general(config, seq, None)
}

fn read_checkpoint_general(
    config: &Config,
    seq: u64,
    path: Option<&str>,
) -> Result<Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>> {
    let checkpoint_path = config.checkpoint_path(seq, path);
    let mut reader = fs::File::open(checkpoint_path)?;
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    bcs::from_bytes(&buffer).map_err(|_| anyhow!("Unable to parse checkpoint file"))
}

pub fn write_checkpoint(
    config: &Config,
    summary: &Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>,
) -> Result<()> {
    write_checkpoint_general(config, summary, None)
}

fn write_checkpoint_general(
    config: &Config,
    summary: &Envelope<CheckpointSummary, AuthorityQuorumSignInfo<true>>,
    path: Option<&str>,
) -> Result<()> {
    let checkpoint_path = config.checkpoint_path(*summary.sequence_number(), path);
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

    // Try getting checkpoints from GraphQL if URL is configured
    let graphql_list = if config.graphql_url.is_some() {
        // match sync_checkpoint_list_to_latest_using_object_store(config).await {
        match sync_checkpoint_list_to_latest_using_fullnode(config).await {
            Ok(list) => list,
            Err(e) => {
                info!("Failed to get checkpoints from GraphQL: {}", e);
                CheckpointList::default()
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
                info!("Failed to get checkpoints from archive: {}", e);
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
    Ok(CheckpointList {
        checkpoints: merged_checkpoints,
    })
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

/// Downloads the list of end of epoch checkpoints from the full node
async fn sync_checkpoint_list_to_latest_using_fullnode(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoints from full node");
    // Download the checkpoint from the server
    let rest_client = Client::new(config.full_node_url.as_str());

    // Get the local checkpoint list
    let mut checkpoints_list: CheckpointList = read_checkpoint_list(config)?;
    let latest_in_list = if let Some(latest_in_list) = checkpoints_list.checkpoints.last() {
        *latest_in_list
    } else {
        let last_checkpoint_in_first_epoch = query_last_checkpoint_of_epoch(config, 0).await?;
        checkpoints_list
            .checkpoints
            .push(last_checkpoint_in_first_epoch);
        write_checkpoint_list(config, &checkpoints_list)?;
        println!(
            "Last Epoch: {} Last Checkpoint: {}",
            0, last_checkpoint_in_first_epoch
        );
        last_checkpoint_in_first_epoch
    };

    // Download the latest in list checkpoint
    let summary = rest_client.get_checkpoint_summary(latest_in_list).await?;
    let mut last_epoch = summary.epoch();

    // Download the very latest checkpoint
    let client = IotaClientBuilder::default()
        .build(config.full_node_url.as_str())
        .await
        .expect("Cannot connect to full node");

    let latest_seq = client
        .read_api()
        .get_latest_checkpoint_sequence_number()
        .await?;
    let latest = rest_client.get_checkpoint_summary(latest_seq).await?;

    // Sequentially record all the missing end of epoch checkpoints numbers
    while last_epoch + 1 < latest.epoch() {
        let target_epoch = last_epoch + 1;
        let target_last_checkpoint_number =
            query_last_checkpoint_of_epoch(config, target_epoch).await?;

        // Add to the list
        checkpoints_list
            .checkpoints
            .push(target_last_checkpoint_number);
        write_checkpoint_list(config, &checkpoints_list)?;

        // Update
        last_epoch = target_epoch;

        println!(
            "Last Epoch: {} Last Checkpoint: {}",
            target_epoch, target_last_checkpoint_number
        );
    }

    Ok(checkpoints_list)
}

/// Downloads the list of end of epoch checkpoints from the archive store
async fn sync_checkpoint_list_to_latest_using_archive(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoints from archive store");
    let Some(archive_store_config) = &config.archive_store_config else {
        return Err(anyhow!("Archive store config is not provided"));
    };
    let remote_object_store: Arc<dyn ObjectStoreGetExt> = if archive_store_config.no_sign_request {
        archive_store_config.make_http()?
    } else {
        Arc::new(archive_store_config.make()?)
    };
    let manifest = read_manifest(remote_object_store).await?;
    let checkpoints = manifest.get_all_end_of_epoch_checkpoint_seq_numbers()?;
    // write_checkpoint_list(config, &CheckpointsList { checkpoints })?;
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
            CheckpointList {
                checkpoints: vec![],
            }
        }
    };

    // If list is empty, we can't proceed with the normal algorithm
    // as we need a starting checkpoint
    if checkpoints_list.checkpoints.is_empty() {
        return Err(anyhow!(
            "Empty checkpoint list and no initial checkpoint to start from"
        ));
    }

    let latest_in_list = checkpoints_list.checkpoints.last().unwrap();

    // Create object store
    let object_store = IotaObjectStore::new(config)?;

    // Download the latest in list checkpoint
    let summary = object_store
        .download_checkpoint_summary(*latest_in_list)
        .await?;
    let mut last_epoch = summary.epoch();

    // Download the very latest checkpoint
    let client = IotaClientBuilder::default()
        .build(config.full_node_url.as_str())
        .await
        .expect("Cannot connect to full node");

    let latest_seq = client
        .read_api()
        .get_latest_checkpoint_sequence_number()
        .await?;
    let latest = object_store.download_checkpoint_summary(latest_seq).await?;

    // Sequentially record all the missing end of epoch checkpoints numbers
    while last_epoch + 1 < latest.epoch() {
        let target_epoch = last_epoch + 1;
        let target_last_checkpoint_number =
            query_last_checkpoint_of_epoch(config, target_epoch).await?;

        // Add to the list
        checkpoints_list
            .checkpoints
            .push(target_last_checkpoint_number);

        // Update
        last_epoch = target_epoch;

        info!(
            "Last Epoch: {} Last Checkpoint: {}",
            target_epoch, target_last_checkpoint_number
        );
    }

    Ok(checkpoints_list)
}

pub async fn check_and_sync_checkpoints(config: &Config) -> anyhow::Result<()> {
    let checkpoints_list = sync_checkpoint_list_to_latest(config)
        .await
        .map_err(|e| anyhow!(format!("Cannot refresh list: {e}")))?;

    // Write the fetched checkpoint list to disk
    write_checkpoint_list(config, &checkpoints_list)?;

    // Load the genesis committee
    let mut genesis_path = config.checkpoints_sync_dir.clone();
    genesis_path.push(&config.genesis_filename);
    let genesis_committee = Genesis::load(&genesis_path)?
        .committee()
        .map_err(|e| anyhow!(format!("Cannot load Genesis: {e}")))?;

    // Check the signatures of all checkpoints
    // And download any missing ones

    let mut prev_committee = genesis_committee;
    for ckp_id in &checkpoints_list.checkpoints {
        // check if there is a file with this name ckp_id.yaml in the
        // checkpoint_summary_dir
        let mut checkpoint_path = config.checkpoints_sync_dir.clone();
        checkpoint_path.push(format!("{}.yaml", ckp_id));

        // If file exists read the file otherwise download it from the server
        let summary = if checkpoint_path.exists() {
            read_checkpoint(config, *ckp_id)
                .map_err(|e| anyhow!(format!("Cannot read checkpoint: {e}")))?
        } else {
            let summary =
                download_checkpoint_summary_from_object_store_with_fallback(config, *ckp_id)
                    .await?;
            summary.clone().try_into_verified(&prev_committee)?;
            // Write the checkpoint summary to a file
            write_checkpoint(config, &summary)?;
            summary
        };

        // Print the id of the checkpoint and the epoch number
        info!(
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

#[cfg(test)]
mod tests {
    use iota_types::{
        gas::GasCostSummary, messages_checkpoint::CheckpointContents,
        supported_protocol_versions::ProtocolConfig,
    };
    use roaring::RoaringBitmap;
    use tempfile::TempDir;

    use super::*;

    fn create_test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            checkpoints_sync_dir: temp_dir.path().to_path_buf(),
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

        write_checkpoint(&config, &test_summary).unwrap();
        let read_summary = read_checkpoint(&config, 0).unwrap();

        assert_eq!(
            test_summary.sequence_number(),
            read_summary.sequence_number()
        );
    }
}
