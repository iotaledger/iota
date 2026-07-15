// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    io::{Read, Write},
};

use anyhow::{Context, Result, bail};
use iota_config::genesis::Genesis;
use iota_data_ingestion_core::history::epoch_boundaries::EpochBoundaries;
use iota_json_rpc_types::CheckpointId;
use iota_sdk::IotaClientBuilder;
use iota_sdk_types::{
    CheckpointContentsDigest, CheckpointDigest, TransactionDigest, checkpoint::CheckpointContents,
};
use iota_types::{
    committee::CommitteeChainVerifier, messages_checkpoint::CertifiedCheckpointSummary,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    config::Config, graphql::query_last_checkpoint_of_epoch, object_store::CheckpointStore,
};

// The list of checkpoints at the end of each epoch
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CheckpointList {
    pub checkpoints: Vec<u64>,
}

impl CheckpointList {
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }

    pub fn get_sequence_number_by_epoch(&self, epoch: u64) -> Option<u64> {
        self.checkpoints.get(epoch as usize).copied()
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
    let path = config.checkpoint_summary_file_path(summary.sequence_number());
    bcs::serialize_into(
        &mut fs::File::create(&path)
            .context(format!("error writing summary file '{}'", path.display()))?,
        &summary,
    )
    .expect("error serializing to bcs");
    Ok(())
}

/// Downloads the list of end-of-epoch checkpoints, using GraphQL first and
/// falling back to the checkpoint archive for any epochs GraphQL cannot serve.
pub async fn sync_checkpoint_list_to_latest(config: &Config) -> anyhow::Result<CheckpointList> {
    let mut checkpoint_list = read_checkpoint_list(config).unwrap_or_default();
    let target_epoch = latest_epoch_from_rpc(config).await?;

    if config.graphql_url.is_some() {
        if let Err(e) = extend_from_graphql(config, &mut checkpoint_list, target_epoch).await {
            warn!("GraphQL checkpoint list sync stopped early, falling back to archive: {e}");
        }
    }

    if (checkpoint_list.len() as u64) < target_epoch && config.checkpoint_store_config.is_some() {
        if let Err(e) =
            extend_from_checkpoint_archive(config, &mut checkpoint_list, target_epoch).await
        {
            warn!("Checkpoint archive fallback failed: {e}");
        }
    }

    if checkpoint_list.is_empty() {
        bail!("Unable to sync from configured sources");
    }

    // Write the fetched checkpoint list to disk
    write_checkpoint_list(config, &checkpoint_list)?;

    Ok(checkpoint_list)
}

/// Syncs the list of end-of-epoch checkpoints from GraphQL only.
pub async fn sync_checkpoint_list_to_latest_from_graphql(
    config: &Config,
) -> anyhow::Result<CheckpointList> {
    let mut checkpoint_list = read_checkpoint_list(config).unwrap_or_default();
    let target_epoch = latest_epoch_from_rpc(config).await?;
    extend_from_graphql(config, &mut checkpoint_list, target_epoch).await?;
    Ok(checkpoint_list)
}

/// Returns the epoch of the latest checkpoint known to the RPC node. Every
/// epoch below this one has a recorded end-of-epoch checkpoint.
async fn latest_epoch_from_rpc(config: &Config) -> anyhow::Result<u64> {
    let client = IotaClientBuilder::default()
        .build(config.rpc_url.as_str())
        .await?;
    let read_api = client.read_api();
    let latest_seq = read_api.get_latest_checkpoint_sequence_number().await?;
    let latest_checkpoint = read_api
        .get_checkpoint(CheckpointId::SequenceNumber(latest_seq))
        .await?;
    Ok(latest_checkpoint.epoch)
}

/// Appends end-of-epoch checkpoints from GraphQL for every epoch from the
/// current list length up to (but excluding) `target_epoch`. Returns an error
/// at the first epoch GraphQL cannot serve, keeping the epochs synced before
/// it.
async fn extend_from_graphql(
    config: &Config,
    checkpoint_list: &mut CheckpointList,
    target_epoch: u64,
) -> anyhow::Result<()> {
    info!("Syncing checkpoint list from GraphQL.");
    for epoch in (checkpoint_list.len() as u64)..target_epoch {
        let seq = query_last_checkpoint_of_epoch(config, epoch).await?;
        checkpoint_list.checkpoints.push(seq);
        info!("Synced epoch: {epoch}, checkpoint: {seq}");
    }
    Ok(())
}

/// Fills the tail of `checkpoint_list` from the checkpoint archive's recorded
/// epoch boundaries, up to (but excluding) `target_epoch`.
async fn extend_from_checkpoint_archive(
    config: &Config,
    checkpoint_list: &mut CheckpointList,
    target_epoch: u64,
) -> anyhow::Result<()> {
    info!("Filling checkpoint list from checkpoint archive.");
    let checkpoint_store = CheckpointStore::new(config)?;
    let boundaries = checkpoint_store.end_of_epoch_checkpoints().await?;
    fill_list_from_boundaries(checkpoint_list, &boundaries, target_epoch);
    Ok(())
}

/// Appends end-of-epoch checkpoints from `boundaries` for every epoch from the
/// current list length up to (but excluding) `target_epoch`, stopping at the
/// first epoch the archive does not have.
fn fill_list_from_boundaries(
    checkpoint_list: &mut CheckpointList,
    boundaries: &EpochBoundaries,
    target_epoch: u64,
) {
    for epoch in (checkpoint_list.len() as u64)..target_epoch {
        let Some(seq) = boundaries.get(epoch) else {
            break;
        };
        checkpoint_list.checkpoints.push(seq);
        info!("Filled epoch: {epoch}, checkpoint: {seq} from archive");
    }
}

pub async fn download_summaries_from_checkpoint_store(
    config: &Config,
    checkpoints: Vec<u64>,
) -> anyhow::Result<()> {
    info!("Downloading summaries from checkpoint store.");

    let checkpoint_store = CheckpointStore::new(config)?;
    for seq in checkpoints {
        info!("Downloading summary: {seq}.sum");

        let summary = checkpoint_store
            .fetch_checkpoint_summary(seq)
            .await
            .context(format!(
                "Failed to download checkpoint summary '{seq}' from checkpoint store"
            ))?;
        write_checkpoint_summary(config, &summary)?;
    }

    Ok(())
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
        if config.checkpoint_store_config.is_some() {
            download_summaries_from_checkpoint_store(config, missing).await?;
        } else {
            anyhow::bail!(
                "No download source configured for missing checkpoint summaries. \
                 Configure `checkpoint_store_config`."
            );
        }
    }

    info!("Verifying summaries.");

    // Walk the committee chain over the end-of-epoch checkpoints, anchored at
    // the genesis committee.
    let mut chain_verifier = CommitteeChainVerifier::new(genesis_committee);
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

        let verified = chain_verifier
            .verify_epoch_close(summary)
            .with_context(|| format!("Failed to verify checkpoint {seq}"))?;

        info!(
            "Verified epoch: {}, checkpoint: {seq}, checkpoint digest: {}",
            verified.epoch(),
            verified.digest()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{
        checkpoint::{CheckpointContents, CheckpointSummary},
        gas::GasCostSummary,
    };
    use iota_types::{
        crypto::AuthorityQuorumSignInfo,
        message_envelope::Envelope,
        messages_checkpoint::{CheckpointContentsExt, CheckpointSummaryExt},
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
        let summary = CheckpointSummary::new_with_protocol_config(
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

    #[test]
    fn fill_list_from_boundaries_appends_missing_tail() {
        let boundaries = EpochBoundaries::from_iter([(0, 10), (1, 20), (2, 30)]);
        // The list already covers epoch 0; fill epochs 1 and 2 up to target 3.
        let mut list = CheckpointList {
            checkpoints: vec![10],
        };
        fill_list_from_boundaries(&mut list, &boundaries, 3);
        assert_eq!(list.checkpoints, vec![10, 20, 30]);
    }

    #[test]
    fn fill_list_from_boundaries_stops_where_archive_ends() {
        // The archive only reaches epoch 1, but the target is epoch 4.
        let boundaries = EpochBoundaries::from_iter([(0, 10), (1, 20)]);
        let mut list = CheckpointList::default();
        fill_list_from_boundaries(&mut list, &boundaries, 4);
        assert_eq!(list.checkpoints, vec![10, 20]);
    }

    #[test]
    fn fill_list_from_boundaries_is_noop_when_already_complete() {
        let boundaries = EpochBoundaries::from_iter([(0, 10), (1, 20)]);
        let mut list = CheckpointList {
            checkpoints: vec![10, 20],
        };
        fill_list_from_boundaries(&mut list, &boundaries, 2);
        assert_eq!(list.checkpoints, vec![10, 20]);
    }
}
