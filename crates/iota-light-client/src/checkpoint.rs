// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    num::NonZeroUsize,
};

use anyhow::{Context, Result, bail};
use getset::Getters;
use iota_archival::reader::{ArchiveReader, ArchiveReaderMetrics};
use iota_config::{genesis::Genesis, node::ArchiveReaderConfig};
use iota_json_rpc_types::CheckpointId;
use iota_sdk::IotaClientBuilder;
use iota_types::{
    committee::Committee,
    messages_checkpoint::{CertifiedCheckpointSummary, EndOfEpochData},
};
use prometheus::Registry;
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
        .context("Unable to serialize checkpoint list")
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
    Ok(bcs::from_bytes(&buffer).expect("Unable to parse checkpoint file"))
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
    let bytes = bcs::to_bytes(summary).expect("unable to serialize checkpoint summary");
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

    let checkpoints_from_object_store = if config.object_store_url.is_some() {
        // TODO blocked by https://github.com/iotaledger/iota/issues/4908
        warn!("Syncing from a checkpoint object store is not supported yet.");
        CheckpointList::default()
        // match sync_checkpoint_list_to_latest_using_full_node_and_object_store(config).await {
        //     Ok(list) => list,
        //     Err(e) => {
        //         warn!("Failed to sync checkpoints from checkpoint object
        // store: {e}");         CheckpointList::default()
        //     }
        // }
    } else {
        CheckpointList::default()
    };

    // Try getting checkpoints from archive store if configured
    let checkpoints_from_archive_store = if config.archive_store_config.is_some() {
        match sync_checkpoint_list_to_latest_using_archive_store_only(config).await {
            Ok(list) => list,
            Err(e) => {
                warn!("Failed to sync checkpoints from archive store: {e}");
                CheckpointList::default()
            }
        }
    } else {
        CheckpointList::default()
    };

    let merged_checkpoints = merge_checkpoint_lists(
        &checkpoints_from_object_store,
        &checkpoints_from_archive_store,
    );

    // Try to sync from the full node if there are still no checkpoints
    let checkpoints_list = if merged_checkpoints.is_empty() {
        match sync_checkpoint_list_to_latest_using_full_node(config).await {
            Ok(list) => list,
            Err(e) => {
                warn!("Failed to sync checkpoints from full node: {e}");
                CheckpointList::default()
            }
        }
    } else {
        CheckpointList {
            checkpoints: merged_checkpoints,
        }
    };

    if checkpoints_list.is_empty() {
        bail!("Could not retrieve any checkpoints from configured sources");
    }

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

// TODO deprecate
/// Syncs the list of end-of-epoch checkpoints from the full node's REST, RPC
/// and GraphQL endpoints alone.
///
/// No object store or archive store required, but only works with non-pruning
/// nodes.
async fn sync_checkpoint_list_to_latest_using_full_node(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoints from full node");

    // Get the local checkpoint list, or create an empty one if it doesn't exist
    let mut checkpoints_list = match read_checkpoint_list(config) {
        Ok(list) => list,
        Err(_) => {
            info!("No existing checkpoint file found. Creating a new list.");
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

    // Download the last synced checkpoint from the node
    let client = IotaClientBuilder::default()
        .build(config.rpc_url.as_str())
        .await?;
    let read_api = client.read_api();
    let last_chk = read_api
        .get_checkpoint(CheckpointId::SequenceNumber(last_seq))
        .await?;

    // Download the latest available checkpoint from the node
    let latest_seq = read_api.get_latest_checkpoint_sequence_number().await?;
    let latest_chk = read_api
        .get_checkpoint(CheckpointId::SequenceNumber(latest_seq))
        .await?;

    // Sequentially record all the missing end of epoch checkpoints numbers
    for target_epoch in (last_chk.epoch + 1)..latest_chk.epoch {
        let target_seq = query_last_checkpoint_of_epoch(config, target_epoch).await?;
        checkpoints_list.checkpoints.push(target_seq);
        info!("Synced epoch: {target_epoch}, checkpoint: {target_seq}");
    }

    Ok(checkpoints_list)
}

/// Syncs the list of end-of-epoch checkpoints from an archive store.
///
/// Does not require a full node.
async fn sync_checkpoint_list_to_latest_using_archive_store_only(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoints from archive store");

    let Some(archive_store_config) = &config.archive_store_config else {
        bail!("Archive store config is not provided");
    };

    // TODO add to config
    let num_parallel_downloads = 5;

    // set up download of checkpoint summaries
    let config = ArchiveReaderConfig {
        remote_store_config: archive_store_config.clone(),
        download_concurrency: NonZeroUsize::new(num_parallel_downloads).unwrap(),
        use_for_pruning_watermark: false,
    };

    let metrics = ArchiveReaderMetrics::new(&Registry::default());
    let archive_reader = ArchiveReader::new(config, &metrics)?;
    archive_reader.sync_manifest_once().await?;

    let manifest = archive_reader.get_manifest().await?;
    let checkpoints = manifest.get_all_end_of_epoch_checkpoint_seq_numbers()?;

    Ok(CheckpointList { checkpoints })
}

// TODO blocked by https://github.com/iotaledger/iota/issues/4908
/// Downloads the list of end-of-epoch checkpoints from an object store.
///
/// Requires full node's RPC, GraphQL endpoints and an checkpoint object store.
async fn _sync_checkpoint_list_to_latest_using_full_node_and_object_store(
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
        .build(config.rpc_url.as_str())
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
        .context("Failed to sync checkpoint list")?;

    // Create a list of summaries that can be skipped
    let mut skiplist = Vec::new();
    for seq in checkpoints_list.checkpoints.iter().copied() {
        if config.checkpoint_summary_file_path(seq, None).exists() {
            skiplist.push(seq);
        }
    }

    // Load the genesis committee
    let genesis_committee = Genesis::load(config.genesis_blob_file_path())?
        .committee()
        .context("Failed to load genesis file")?;

    if let Some(_checkpoint_store_url) = &config.object_store_url {
        // Download summaries from checkpoint object store
        // TODO blocked by https://github.com/iotaledger/iota/issues/4908
        warn!("Syncing from a checkpoint object store is not supported yet.");
    }

    if let Some(archive_store_config) = &config.archive_store_config {
        // Download summaries from archive store
        let archive_reader_config = ArchiveReaderConfig {
            remote_store_config: archive_store_config.clone(),
            download_concurrency: NonZeroUsize::new(5).unwrap(),
            use_for_pruning_watermark: false,
        };

        let metrics = ArchiveReaderMetrics::new(&Registry::default());
        let archive_reader = ArchiveReader::new(archive_reader_config, &metrics)?;
        archive_reader.sync_manifest_once().await?;
        archive_reader
            .download_summaries_for_list_no_verify(
                checkpoints_list.checkpoints.clone(),
                skiplist,
                &config.checkpoints_dir,
            )
            .await?;
    } else {
        // Download summaries from the full node
        let client = iota_rest_api::Client::new(format!("{}/rest", config.rpc_url));

        // We only need the first 2 end-of-epoch checkpoints for the tests
        for seq in checkpoints_list.checkpoints.iter().copied() {
            if skiplist.contains(&seq) {
                continue;
            }

            info!("Downloading summary checkpoint: {seq}");

            let summary = client
                .get_checkpoint_summary(seq)
                .await
                .context(format!("Failed to download checkpoint summary '{seq}'"))?;
            let path = format!("{}/{seq}.sum", config.checkpoints_dir.display());
            bcs::serialize_into(
                &mut std::fs::File::create(&path)
                    .context(format!("error creating summary file '{path}'"))?,
                &summary,
            )
            .expect("error serializing to bcs");
        }
    }

    // Check the signatures of all checkpoints and download any missing ones
    let mut prev_committee = genesis_committee;
    for seq in checkpoints_list.checkpoints {
        // Check if there is a corresponding checkpoint summary file in the checkpoints
        // directory
        let summary_path = config.checkpoint_summary_file_path(seq, None);

        // If file exists read the file otherwise download it from the server
        let summary = if summary_path.exists() {
            read_checkpoint_summary(config, seq).context("Failed to read checkpoint summary")?
        } else {
            bail!("we assume for now that everything could be downloaded");
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
