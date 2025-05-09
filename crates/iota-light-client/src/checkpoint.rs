// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::sync::atomic::AtomicU64;
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    num::NonZeroUsize,
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use getset::Getters;
use iota_archival::reader::{ArchiveReader, ArchiveReaderMetrics};
use iota_config::{genesis::Genesis, node::ArchiveReaderConfig};
use iota_json_rpc_types::CheckpointId;
use iota_sdk::IotaClientBuilder;
use iota_types::{
    committee::Committee,
    messages_checkpoint::{CertifiedCheckpointSummary, EndOfEpochData, VerifiedCheckpoint},
    storage::{ObjectStore, ReadStore, WriteStore},
};
use prometheus::Registry;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    config::Config, graphql::query_last_checkpoint_of_epoch, object_store::CheckpointStore,
};

// The list of checkpoints at the end of each epoch
#[derive(Debug, Clone, Default, Deserialize, Serialize, Getters)]
#[getset(get = "pub")]
pub struct CheckpointList {
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

pub fn read_checkpoint_summary(config: &Config, seq: u64) -> Result<CertifiedCheckpointSummary> {
    let checkpoint_path = config.checkpoint_summary_file_path(seq);
    let mut reader = fs::File::open(checkpoint_path)?;
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    Ok(bcs::from_bytes(&buffer).expect("Unable to parse checkpoint file"))
}

pub fn write_checkpoint_list(config: &Config, checkpoints_list: &CheckpointList) -> Result<()> {
    let checkpoints_path = config.checkpoints_list_file_path();
    let mut writer = fs::File::create(checkpoints_path)?;
    let bytes = serde_yaml::to_vec(checkpoints_list)?;
    writer
        .write_all(&bytes)
        .context("Unable to serialize checkpoint list")
}

pub fn write_checkpoint_summary(
    config: &Config,
    summary: &CertifiedCheckpointSummary,
) -> Result<()> {
    let path = config.checkpoint_summary_file_path(*summary.sequence_number());
    bcs::serialize_into(
        &mut fs::File::create(&path)
            .context(format!("error writing summary file '{}'", path.display()))?,
        &summary,
    )
    .expect("error serializing to bcs");
    Ok(())
}

/// Downloads the list of end of epoch checkpoints from the archive store or the
/// GraphQL endpoint
pub async fn sync_checkpoint_list_to_latest(config: &Config) -> anyhow::Result<CheckpointList> {
    let checkpoints_from_archive = if config.archive_store_config.is_some() {
        match sync_checkpoint_list_to_latest_from_archive(config).await {
            Ok(list) => list,
            Err(e) => {
                warn!("Failed to sync checkpoint list from archive: {e}");
                CheckpointList::default()
            }
        }
    } else {
        CheckpointList::default()
    };

    let checkpoints_from_graphql = if config.graphql_url.is_some() {
        match sync_checkpoint_list_to_latest_from_graphql(config).await {
            Ok(list) => list,
            Err(e) => {
                warn!("Failed to sync checkpoints from full node: {e}");
                CheckpointList::default()
            }
        }
    } else {
        CheckpointList::default()
    };

    let checkpoint_list =
        merge_checkpoint_lists(&checkpoints_from_archive, &checkpoints_from_graphql);

    if checkpoint_list.is_empty() {
        bail!("Unable to sync from configured sources");
    }

    // Write the fetched checkpoint list to disk
    write_checkpoint_list(config, &checkpoint_list)?;

    Ok(checkpoint_list)
}

/// Merges two checkpoint lists, removing duplicates and ensuring the result is
/// sorted
fn merge_checkpoint_lists(list1: &CheckpointList, list2: &CheckpointList) -> CheckpointList {
    let unique_checkpoints: HashSet<u64> = list1
        .checkpoints
        .iter()
        .chain(list2.checkpoints.iter())
        .copied()
        .collect();

    // Convert to sorted vector
    let mut sorted_checkpoints: Vec<_> = unique_checkpoints.into_iter().collect();
    sorted_checkpoints.sort();

    CheckpointList {
        checkpoints: sorted_checkpoints,
    }
}

/// Syncs the list of end-of-epoch checkpoints from GraphQL.
async fn sync_checkpoint_list_to_latest_from_graphql(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoint list from GraphQL.");

    // Get the local checkpoint list, or create an empty one if it doesn't exist
    let mut checkpoints_list = match read_checkpoint_list(config) {
        Ok(list) => list,
        Err(_) => {
            info!("No existing checkpoint file found. Creating a new checkpoint list.");
            CheckpointList::default()
        }
    };

    // Get the last synced epoch, or fetch the first
    let last_epoch = if !checkpoints_list.is_empty() {
        checkpoints_list.len() as u64 - 1
    } else {
        let first_epoch = 0u64;
        let first_seq = query_last_checkpoint_of_epoch(config, first_epoch).await?;
        checkpoints_list.checkpoints.push(first_seq);
        info!("Synced epoch: {first_epoch}, checkpoint: {first_seq}",);
        first_epoch
    };

    // Download the last synced checkpoint from the node
    let client = IotaClientBuilder::default()
        .build(config.rpc_url.as_str())
        .await?;
    let read_api = client.read_api();

    // Download the latest available checkpoint from the node
    let latest_seq = read_api.get_latest_checkpoint_sequence_number().await?;
    let latest_checkpoint = read_api
        .get_checkpoint(CheckpointId::SequenceNumber(latest_seq))
        .await?;

    // Sequentially record all the missing end of epoch checkpoints numbers
    for target_epoch in (last_epoch + 1)..latest_checkpoint.epoch {
        let target_seq = query_last_checkpoint_of_epoch(config, target_epoch).await?;
        checkpoints_list.checkpoints.push(target_seq);
        info!("Synced epoch: {target_epoch}, checkpoint: {target_seq}");
    }

    Ok(checkpoints_list)
}

/// Syncs the list of end-of-epoch checkpoints from an archive store.
async fn sync_checkpoint_list_to_latest_from_archive(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    info!("Syncing checkpoint list from archive store.");

    let Some(archive_store_config) = &config.archive_store_config else {
        bail!("Archive store config is not provided");
    };

    let config = ArchiveReaderConfig {
        remote_store_config: archive_store_config.clone(),
        download_concurrency: NonZeroUsize::new(5).unwrap(),
        use_for_pruning_watermark: false,
    };

    let metrics = ArchiveReaderMetrics::new(&Registry::default());
    let archive_reader = ArchiveReader::new(config, &metrics)?;
    archive_reader.sync_manifest_once().await?;

    let manifest = archive_reader.get_manifest().await?;
    let checkpoints = manifest.get_all_end_of_epoch_checkpoint_seq_numbers()?;

    Ok(CheckpointList { checkpoints })
}

pub async fn sync_and_verify_checkpoints(config: &Config) -> anyhow::Result<()> {
    let checkpoints_list = sync_checkpoint_list_to_latest(config)
        .await
        .context("Failed to sync checkpoint list")?;

    // Load the genesis committee
    let genesis_committee = Genesis::load(config.genesis_blob_file_path())?
        .committee()
        .context("Failed to load genesis file")?;

    // Create a list of summaries that need to be downloaded
    let mut missing = Vec::new();
    for seq in checkpoints_list.checkpoints.iter().copied() {
        if !config.checkpoint_summary_file_path(seq).exists() {
            // ensure the file is valid and can be parsed
            if read_checkpoint_summary(config, seq).is_err() {
                missing.push(seq);
            }
        }
    }

    if !missing.is_empty() {
        if let Some(archive_store_config) = &config.archive_store_config {
            info!("Downloading missing checkpoints from archive store.");

            // Download summaries from archive store
            let archive_reader_config = ArchiveReaderConfig {
                remote_store_config: archive_store_config.clone(),
                download_concurrency: NonZeroUsize::new(5).unwrap(),
                use_for_pruning_watermark: false,
            };

            let store = CheckpointSummaryFileStore::new(config);
            let counter = Arc::new(AtomicU64::new(0));
            let metrics = ArchiveReaderMetrics::new(&Registry::default());
            let archive_reader = ArchiveReader::new(archive_reader_config, &metrics)?;
            archive_reader.sync_manifest_once().await?;
            archive_reader
                .read_summaries_for_list_no_verify(store.clone(), missing, counter)
                .await?;
        } else if let Some(_checkpoint_store_url) = &config.checkpoint_store_config {
            info!("Downloading missing checkpoints from checkpoint store.");

            let checkpoint_store = CheckpointStore::new(config)?;
            for seq in missing {
                info!("Downloading checkpoint: {seq}");

                let summary = checkpoint_store
                    .fetch_checkpoint_summary(seq)
                    .await
                    .context(format!(
                        "Failed to download checkpoint summary '{seq}' from checkpoint store"
                    ))?;
                write_checkpoint_summary(config, &summary)?;
            }
        } else {
            info!("Downloading missing checkpoints from node.");

            // Download summaries from the full node
            let client = iota_rest_api::Client::new(&config.rpc_url);

            // Download all missing checkpoints
            for seq in missing {
                info!("Downloading checkpoint: {seq}");

                let summary = client
                    .get_checkpoint_summary(seq)
                    .await
                    .context(format!("Failed to download checkpoint summary '{seq}'"))?;

                write_checkpoint_summary(config, &summary)?;
            }
        }
    }

    info!("Verifying checkpoints.");

    // Check the signatures of all checkpoints
    let mut prev_committee = genesis_committee;
    for seq in checkpoints_list.checkpoints {
        // Check if there is a corresponding checkpoint summary file in the checkpoints
        // directory
        let summary_path = config.checkpoint_summary_file_path(seq);

        // If file exists read the file otherwise download it from the server
        let summary = if summary_path.exists() {
            read_checkpoint_summary(config, seq).context("Failed to read checkpoint summary")?
        } else {
            panic!("corrupted checkpoint directory");
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

#[derive(Clone, Debug)]
struct CheckpointSummaryFileStore<'a> {
    config: &'a Config,
}

impl<'a> CheckpointSummaryFileStore<'a> {
    fn new(config: &'a Config) -> Self {
        Self { config }
    }
}

impl WriteStore for CheckpointSummaryFileStore<'_> {
    fn insert_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> iota_types::storage::error::Result<()> {
        let path = self
            .config
            .checkpoint_summary_file_path(*checkpoint.sequence_number());
        info!("Downloading checkpoint summary to '{}'", path.display());
        bcs::serialize_into(
            &mut fs::File::create(&path).expect("error writing file"),
            &checkpoint.clone().into_inner(),
        )
        .expect("error serializing summary checkpoint to bcs");
        Ok(())
    }

    fn update_highest_synced_checkpoint(
        &self,
        _: &iota_types::messages_checkpoint::VerifiedCheckpoint,
    ) -> iota_types::storage::error::Result<()> {
        unimplemented!()
    }

    fn update_highest_verified_checkpoint(
        &self,
        _: &iota_types::messages_checkpoint::VerifiedCheckpoint,
    ) -> iota_types::storage::error::Result<()> {
        unimplemented!()
    }

    fn insert_checkpoint_contents(
        &self,
        _: &iota_types::messages_checkpoint::VerifiedCheckpoint,
        _: iota_types::messages_checkpoint::VerifiedCheckpointContents,
    ) -> iota_types::storage::error::Result<()> {
        unimplemented!()
    }

    fn insert_committee(&self, _: Committee) -> iota_types::storage::error::Result<()> {
        unimplemented!()
    }
}

impl ReadStore for CheckpointSummaryFileStore<'_> {
    fn get_committee(
        &self,
        _: iota_types::committee::EpochId,
    ) -> iota_types::storage::error::Result<Option<Arc<Committee>>> {
        unimplemented!()
    }

    fn get_latest_checkpoint(&self) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        unimplemented!()
    }

    fn get_highest_verified_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        unimplemented!()
    }

    fn get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        unimplemented!()
    }

    fn get_lowest_available_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<iota_types::messages_checkpoint::CheckpointSequenceNumber>
    {
        unimplemented!()
    }

    fn get_checkpoint_by_digest(
        &self,
        _: &iota_types::digests::CheckpointDigest,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        unimplemented!()
    }

    fn get_checkpoint_by_sequence_number(
        &self,
        _: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        unimplemented!()
    }

    fn get_checkpoint_contents_by_digest(
        &self,
        _: &iota_types::digests::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        unimplemented!()
    }

    fn get_checkpoint_contents_by_sequence_number(
        &self,
        _: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        unimplemented!()
    }

    fn get_transaction(
        &self,
        _: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<Arc<iota_types::transaction::VerifiedTransaction>>>
    {
        unimplemented!()
    }

    fn get_transaction_effects(
        &self,
        _: &iota_types::digests::TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEffects>> {
        unimplemented!()
    }

    fn get_events(
        &self,
        _: &iota_types::digests::TransactionEventsDigest,
    ) -> iota_types::storage::error::Result<Option<iota_types::effects::TransactionEvents>> {
        unimplemented!()
    }

    fn get_full_checkpoint_contents_by_sequence_number(
        &self,
        _: iota_types::messages_checkpoint::CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        unimplemented!()
    }

    fn get_full_checkpoint_contents(
        &self,
        _: &iota_types::digests::CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::FullCheckpointContents>,
    > {
        unimplemented!()
    }
}

impl ObjectStore for CheckpointSummaryFileStore<'_> {
    fn get_object(
        &self,
        _: &iota_types::base_types::ObjectID,
    ) -> iota_types::storage::error::Result<Option<iota_types::object::Object>> {
        unimplemented!()
    }

    fn get_object_by_key(
        &self,
        _: &iota_types::base_types::ObjectID,
        _: iota_types::base_types::VersionNumber,
    ) -> iota_types::storage::error::Result<Option<iota_types::object::Object>> {
        unimplemented!()
    }
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
            rpc_url: "http://localhost:9000".parse().unwrap(),
            graphql_url: None,
            checkpoints_dir: temp_dir.path().to_path_buf(),
            sync_before_check: false,
            genesis_blob_download_url: None,
            checkpoint_store_config: None,
            archive_store_config: None,
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
