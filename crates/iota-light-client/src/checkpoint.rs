// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use iota_config::genesis::Genesis;
use iota_data_ingestion_core::history::epoch_boundaries::EpochBoundaries;
use iota_json_rpc_types::CheckpointId;
use iota_sdk::IotaClientBuilder;
use iota_types::{
    committee::{Committee, CommitteeChainVerifier},
    messages_checkpoint::{CertifiedCheckpointSummary, VerifiedCheckpoint},
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
    bcs::from_bytes(&buffer).context("Unable to parse checkpoint file")
}

/// Authenticates the committee for `epoch` from the configured genesis.
///
/// Cached summaries and the checkpoint list are untrusted, including files left
/// behind by a failed sync. Verify every preceding epoch's closing checkpoint
/// before using its next committee. This works offline and does not depend on
/// `sync_before_check`; syncing only determines which summaries are available.
pub fn verified_committee_for_epoch(config: &Config, epoch: u64) -> Result<Committee> {
    let mut verifier = CommitteeChainVerifier::new(load_genesis_committee(config)?);
    if epoch == 0 {
        return Ok(verifier.committee().clone());
    }
    let list = read_checkpoint_list(config)
        .context("Checkpoint list not found. Please run the `sync` command first")?;
    let epoch_count = usize::try_from(epoch).context("Epoch does not fit the checkpoint list")?;
    let summaries = list.checkpoints.get(..epoch_count).ok_or_else(|| {
        anyhow::anyhow!("Checkpoint list not synced. Please run the `sync` command first")
    })?;
    for &seq in summaries {
        verify_summary_file(config, seq, &mut verifier)?;
    }
    Ok(verifier.committee().clone())
}

fn load_genesis_committee(config: &Config) -> Result<Committee> {
    Genesis::load(config.genesis_blob_file_path())?
        .committee()
        .context("Failed to load genesis committee")
}

fn verify_summary_file(
    config: &Config,
    seq: u64,
    verifier: &mut CommitteeChainVerifier,
) -> Result<VerifiedCheckpoint> {
    let summary = read_checkpoint_summary(config, seq)
        .with_context(|| format!("Failed to read checkpoint {seq}"))?;
    anyhow::ensure!(
        summary.sequence_number() == seq,
        "Checkpoint file {seq} contains a different sequence number"
    );
    verifier
        .verify_epoch_close(summary)
        .with_context(|| format!("Failed to verify checkpoint {seq}"))
}

pub fn write_checkpoint_list(config: &Config, checkpoints_list: &CheckpointList) -> Result<()> {
    let bytes =
        serde_yaml::to_vec(checkpoints_list).context("Unable to serialize checkpoint list")?;
    write_file_atomically(&config.checkpoints_list_file_path(), &bytes)
}

pub fn write_checkpoint_summary(
    config: &Config,
    summary: &CertifiedCheckpointSummary,
) -> Result<()> {
    let bytes = bcs::to_bytes(summary).context("Unable to serialize checkpoint summary")?;
    write_file_atomically(
        &config.checkpoint_summary_file_path(summary.sequence_number()),
        &bytes,
    )
}

/// Writes through a temporary file in the same directory and renames it over
/// `path`, so that `path` never holds a partial write.
fn write_file_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    let directory = path
        .parent()
        .with_context(|| format!("'{}' has no parent directory", path.display()))?;
    let mut builder = tempfile::Builder::new();
    // The cache holds public data, so keep it readable by other users as a
    // plainly created file would be: a temporary file is owner-only by default,
    // and the rename below keeps the temporary file's access rights. The umask
    // applies to the mode given here, as it does to a plain create.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    let mut file = builder
        .tempfile_in(directory)
        .with_context(|| format!("Unable to create a file in '{}'", directory.display()))?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("Unable to write '{}'", path.display()))?;
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
        info!("Filled epoch: {epoch}, checkpoint: {seq} from checkpoint archive");
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

    let genesis_committee = load_genesis_committee(config)?;

    let missing = missing_summaries(config, &checkpoints_list.checkpoints);
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
    verify_cached_summaries(config, &checkpoints_list.checkpoints, genesis_committee)
}

fn missing_summaries(config: &Config, checkpoints: &[u64]) -> Vec<u64> {
    checkpoints
        .iter()
        .copied()
        .filter(|&seq| read_checkpoint_summary(config, seq).is_err())
        .collect()
}

/// A summary that cannot be read or verified is removed, so that the next sync
/// downloads it again.
fn verify_cached_summaries(
    config: &Config,
    checkpoints: &[u64],
    genesis_committee: Committee,
) -> Result<()> {
    let mut chain_verifier = CommitteeChainVerifier::new(genesis_committee);
    for &seq in checkpoints {
        let verified = match verify_summary_file(config, seq, &mut chain_verifier) {
            Ok(verified) => verified,
            Err(error) => {
                remove_unusable_summary(config, seq);
                return Err(error);
            }
        };
        info!(
            "Verified epoch: {}, checkpoint: {seq}, checkpoint digest: {}",
            verified.epoch(),
            verified.digest()
        );
    }
    Ok(())
}

fn remove_unusable_summary(config: &Config, seq: u64) {
    let path = config.checkpoint_summary_file_path(seq);
    match fs::remove_file(&path) {
        Ok(()) => warn!(
            "Removed unusable checkpoint summary '{}'; the next sync downloads it again",
            path.display()
        ),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => warn!(
            "Failed to remove unusable checkpoint summary '{}': {error}",
            path.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{CheckpointContents, CheckpointSummary, EndOfEpochData, GasCostSummary};
    use iota_types::{
        crypto::{AuthorityKeyPair, AuthorityQuorumSignInfo},
        message_envelope::Envelope,
        messages_checkpoint::{CheckpointContentsExt, CheckpointSummaryExt},
        supported_protocol_versions::ProtocolConfig,
        utils::make_committee_key,
    };
    use rand::{SeedableRng, rngs::StdRng};
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

    fn summary(seq: u64, end_of_epoch_data: Option<EndOfEpochData>) -> CheckpointSummary {
        CheckpointSummary::new_with_protocol_config(
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            0,
            seq,
            0,
            &CheckpointContents::new_with_digests_only_for_tests(vec![]),
            None,
            GasCostSummary::default(),
            end_of_epoch_data,
            0,
            Vec::new(),
            Vec::new(),
            Default::default(),
        )
    }

    fn unsigned_summary(seq: u64) -> CertifiedCheckpointSummary {
        let info = AuthorityQuorumSignInfo::<true> {
            epoch: 0,
            signature: Default::default(),
            signers_map: RoaringBitmap::new(),
        };
        Envelope::new_from_data_and_sig(summary(seq, None), info)
    }

    fn closing_summary(
        seq: u64,
        committee: &Committee,
        keypairs: &[AuthorityKeyPair],
    ) -> CertifiedCheckpointSummary {
        let summary = summary(
            seq,
            Some(EndOfEpochData {
                next_epoch_committee: committee.committee_members(),
                next_epoch_protocol_version: 1,
                epoch_commitments: vec![],
                epoch_supply_change: 0,
            }),
        );
        CertifiedCheckpointSummary::new_from_keypairs_for_testing(summary, keypairs, committee)
    }

    #[test]
    fn test_checkpoint_read_write() {
        let (config, _temp_dir) = create_test_config();
        let test_summary = unsigned_summary(0);

        write_checkpoint_summary(&config, &test_summary).unwrap();
        let read_summary = read_checkpoint_summary(&config, 0).unwrap();

        assert_eq!(
            test_summary.sequence_number(),
            read_summary.sequence_number()
        );
    }

    #[test]
    fn write_replaces_the_file_and_leaves_no_temporary_file() {
        let (config, temp_dir) = create_test_config();
        write_checkpoint_summary(&config, &unsigned_summary(10)).unwrap();
        write_checkpoint_summary(&config, &unsigned_summary(10)).unwrap();
        write_checkpoint_list(&config, &CheckpointList::default()).unwrap();

        assert_eq!(fs::read_dir(temp_dir.path()).unwrap().count(), 2);
        assert_eq!(
            read_checkpoint_summary(&config, 10)
                .unwrap()
                .sequence_number(),
            10
        );
    }

    #[test]
    fn unparsable_summary_counts_as_missing() {
        let (config, _temp_dir) = create_test_config();
        write_checkpoint_summary(&config, &unsigned_summary(10)).unwrap();
        fs::write(config.checkpoint_summary_file_path(20), b"not a summary").unwrap();

        assert_eq!(missing_summaries(&config, &[10, 20, 30]), vec![20, 30]);
    }

    #[test]
    fn verified_summary_is_kept() {
        let (config, _temp_dir) = create_test_config();
        let (committee, keypairs) = Committee::new_simple_test_committee();
        write_checkpoint_summary(&config, &closing_summary(10, &committee, &keypairs)).unwrap();

        verify_cached_summaries(&config, &[10], committee).unwrap();

        assert!(config.checkpoint_summary_file_path(10).exists());
    }

    #[test]
    fn unreadable_summary_is_removed() {
        let (config, _temp_dir) = create_test_config();
        let path = config.checkpoint_summary_file_path(10);
        fs::write(&path, b"not a summary").unwrap();
        let (committee, _) = Committee::new_simple_test_committee();

        assert!(verify_cached_summaries(&config, &[10], committee).is_err());

        assert!(!path.exists());
    }

    #[test]
    fn misnamed_summary_is_removed() {
        let (config, _temp_dir) = create_test_config();
        let (committee, keypairs) = Committee::new_simple_test_committee();
        let path = config.checkpoint_summary_file_path(10);
        fs::write(
            &path,
            bcs::to_bytes(&closing_summary(99, &committee, &keypairs)).unwrap(),
        )
        .unwrap();

        assert!(verify_cached_summaries(&config, &[10], committee).is_err());

        assert!(!path.exists());
    }

    #[test]
    fn only_the_failing_summary_is_removed() {
        let (config, _temp_dir) = create_test_config();
        let (committee, keypairs) = Committee::new_simple_test_committee();
        write_checkpoint_summary(&config, &closing_summary(10, &committee, &keypairs)).unwrap();
        fs::write(config.checkpoint_summary_file_path(20), b"not a summary").unwrap();

        assert!(verify_cached_summaries(&config, &[10, 20], committee).is_err());

        assert!(config.checkpoint_summary_file_path(10).exists());
        assert!(!config.checkpoint_summary_file_path(20).exists());
    }

    #[test]
    fn walk_stops_at_the_first_failing_summary() {
        let (config, _temp_dir) = create_test_config();
        let (committee, _) = Committee::new_simple_test_committee();
        fs::write(config.checkpoint_summary_file_path(10), b"not a summary").unwrap();
        fs::write(config.checkpoint_summary_file_path(20), b"not a summary").unwrap();

        assert!(verify_cached_summaries(&config, &[10, 20], committee).is_err());

        assert!(!config.checkpoint_summary_file_path(10).exists());
        assert!(config.checkpoint_summary_file_path(20).exists());
    }

    #[test]
    fn summary_signed_by_another_committee_is_removed() {
        let (config, _temp_dir) = create_test_config();
        let (committee, _) = Committee::new_simple_test_committee();
        // Any seed but the `[0; 32]` of `new_simple_test_committee`, so no member is shared.
        let (other_keypairs, other_committee) = make_committee_key(&mut StdRng::from_seed([1; 32]));
        write_checkpoint_summary(
            &config,
            &closing_summary(10, &other_committee, &other_keypairs),
        )
        .unwrap();

        assert!(verify_cached_summaries(&config, &[10], committee).is_err());

        assert!(!config.checkpoint_summary_file_path(10).exists());
    }

    #[test]
    fn non_closing_summary_is_removed() {
        let (config, _temp_dir) = create_test_config();
        write_checkpoint_summary(&config, &unsigned_summary(10)).unwrap();
        let (committee, _) = Committee::new_simple_test_committee();

        assert!(verify_cached_summaries(&config, &[10], committee).is_err());

        assert!(!config.checkpoint_summary_file_path(10).exists());
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
