// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::Bound::Included, time::Duration};

use bytes::Bytes;
use consensus_config::{AuthorityIndex, Committee};
use iota_common::{misbehavior_counts::MisbehaviorsV1, scoring_metrics::VersionedScoringMetrics};
use iota_macros::fail_point;
use typed_store::{
    Map as _,
    metrics::SamplingInterval,
    reopen,
    rocks::{DBMap, MetricConf, ReadWriteOptions, default_db_options, open_cf_opts},
};

use super::{CommitInfo, Store, WriteBatch};
use crate::{
    block::{BlockAPI as _, BlockDigest, BlockRef, Round, SignedBlock, VerifiedBlock},
    commit::{CommitAPI as _, CommitDigest, CommitIndex, CommitRange, CommitRef, TrustedCommit},
    error::{ConsensusError, ConsensusResult},
    storage::StorageScoringMetrics,
};

/// Persistent storage with RocksDB.
pub(crate) struct RocksDBStore {
    /// Stores SignedBlock by refs.
    blocks: DBMap<(Round, AuthorityIndex, BlockDigest), Bytes>,
    /// A secondary index that orders refs first by authors.
    digests_by_authorities: DBMap<(AuthorityIndex, Round, BlockDigest), ()>,
    /// Maps commit index to Commit.
    commits: DBMap<(CommitIndex, CommitDigest), Bytes>,
    /// Collects votes on commits.
    /// TODO: batch multiple votes into a single row.
    commit_votes: DBMap<(CommitIndex, CommitDigest, BlockRef), ()>,
    /// Stores info related to Commit that helps recovery.
    commit_info: DBMap<(CommitIndex, CommitDigest), CommitInfo>,
    /// Legacy scoring metrics (read-only).
    /// TODO: remove this field after migration is done.
    #[deprecated]
    scoring_metrics: DBMap<AuthorityIndex, StorageScoringMetrics>,
    /// Stores versioned scoring metrics as a single blob under key 0.
    scoring_metrics_v2: DBMap<(), VersionedScoringMetrics>,
}

impl RocksDBStore {
    const BLOCKS_CF: &'static str = "blocks";
    const DIGESTS_BY_AUTHORITIES_CF: &'static str = "digests";
    const COMMITS_CF: &'static str = "commits";
    const COMMIT_VOTES_CF: &'static str = "commit_votes";
    const COMMIT_INFO_CF: &'static str = "commit_info";
    const SCORING_METRICS_CF: &'static str = "scoring_metrics";
    const SCORING_METRICS_V2_CF: &'static str = "scoring_metrics_v2";

    /// Creates a new instance of RocksDB storage.
    pub(crate) fn new(path: &str) -> Self {
        // Consensus data has high write throughput (all transactions) and is rarely
        // read (only during recovery and when helping peers catch up).
        let db_options = default_db_options().optimize_db_for_write_throughput(2);
        let mut metrics_conf = MetricConf::new("consensus");
        metrics_conf.read_sample_interval = SamplingInterval::new(Duration::from_secs(60), 0);
        let cf_options = default_db_options().optimize_for_write_throughput().options;
        let column_family_options = vec![
            (
                Self::BLOCKS_CF,
                default_db_options()
                    .optimize_for_write_throughput_no_deletion()
                    // Using larger block is ok since there is not much point reads on the cf.
                    .set_block_options(512, 128 << 10)
                    .options,
            ),
            (Self::DIGESTS_BY_AUTHORITIES_CF, cf_options.clone()),
            (Self::COMMITS_CF, cf_options.clone()),
            (Self::COMMIT_VOTES_CF, cf_options.clone()),
            (Self::COMMIT_INFO_CF, cf_options.clone()),
            (Self::SCORING_METRICS_CF, cf_options.clone()),
            (Self::SCORING_METRICS_V2_CF, cf_options.clone()),
        ];
        let rocksdb = open_cf_opts(
            path,
            Some(db_options.options),
            metrics_conf,
            &column_family_options,
        )
        .expect("Cannot open database");

        let (
            blocks,
            digests_by_authorities,
            commits,
            commit_votes,
            commit_info,
            scoring_metrics,
            scoring_metrics_v2,
        ) = reopen!(&rocksdb,
            Self::BLOCKS_CF;<(Round, AuthorityIndex, BlockDigest), bytes::Bytes>,
            Self::DIGESTS_BY_AUTHORITIES_CF;<(AuthorityIndex, Round, BlockDigest), ()>,
            Self::COMMITS_CF;<(CommitIndex, CommitDigest), Bytes>,
            Self::COMMIT_VOTES_CF;<(CommitIndex, CommitDigest, BlockRef), ()>,
            Self::COMMIT_INFO_CF;<(CommitIndex, CommitDigest), CommitInfo>,
            Self::SCORING_METRICS_CF;<AuthorityIndex, StorageScoringMetrics>,
            Self::SCORING_METRICS_V2_CF;<(), VersionedScoringMetrics>
        );

        Self {
            blocks,
            digests_by_authorities,
            commits,
            commit_votes,
            commit_info,
            #[allow(deprecated)]
            scoring_metrics,
            scoring_metrics_v2,
        }
    }
}

impl Store for RocksDBStore {
    fn write(&self, write_batch: WriteBatch) -> ConsensusResult<()> {
        fail_point!("consensus-store-before-write");

        let mut batch = self.blocks.batch();
        for block in write_batch.blocks {
            let block_ref = block.reference();
            batch
                .insert_batch(
                    &self.blocks,
                    [(
                        (block_ref.round, block_ref.author, block_ref.digest),
                        block.serialized(),
                    )],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
            batch
                .insert_batch(
                    &self.digests_by_authorities,
                    [((block_ref.author, block_ref.round, block_ref.digest), ())],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
            for vote in block.commit_votes() {
                batch
                    .insert_batch(
                        &self.commit_votes,
                        [((vote.index, vote.digest, block_ref), ())],
                    )
                    .map_err(ConsensusError::RocksDBFailure)?;
            }
        }

        for commit in write_batch.commits {
            batch
                .insert_batch(
                    &self.commits,
                    [((commit.index(), commit.digest()), commit.serialized())],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
        }

        for (commit_ref, commit_info) in write_batch.commit_info {
            batch
                .insert_batch(
                    &self.commit_info,
                    [((commit_ref.index, commit_ref.digest), commit_info)],
                )
                .map_err(ConsensusError::RocksDBFailure)?;
        }
        if let Some(metrics) = &write_batch.scoring_metrics {
            batch
                .insert_batch(&self.scoring_metrics_v2, [(&(), metrics)])
                .map_err(ConsensusError::RocksDBFailure)?;
        }

        batch.write()?;
        fail_point!("consensus-store-after-write");
        Ok(())
    }

    fn read_blocks(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<Option<VerifiedBlock>>> {
        let keys = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();
        let serialized = self.blocks.multi_get(keys)?;
        let mut blocks = vec![];
        for (key, serialized) in refs.iter().zip(serialized) {
            if let Some(serialized) = serialized {
                let signed_block: SignedBlock =
                    bcs::from_bytes(&serialized).map_err(ConsensusError::MalformedBlock)?;
                // Only accepted blocks should have been written to storage.
                let block = VerifiedBlock::new_verified(signed_block, serialized);
                // Makes sure block data is not corrupted, by comparing digests.
                assert_eq!(*key, block.reference());
                blocks.push(Some(block));
            } else {
                blocks.push(None);
            }
        }
        Ok(blocks)
    }

    fn contains_blocks(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<bool>> {
        let refs = refs
            .iter()
            .map(|r| (r.round, r.author, r.digest))
            .collect::<Vec<_>>();
        let exist = self.blocks.multi_contains_keys(refs)?;
        Ok(exist)
    }

    fn contains_block_at_slot(&self, slot: crate::block::Slot) -> ConsensusResult<bool> {
        let found = self
            .digests_by_authorities
            .safe_range_iter((
                Included((slot.authority, slot.round, BlockDigest::MIN)),
                Included((slot.authority, slot.round, BlockDigest::MAX)),
            ))
            .next()
            .is_some();
        Ok(found)
    }

    fn scan_blocks_by_author(
        &self,
        author: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<VerifiedBlock>> {
        let mut refs = vec![];
        for kv in self.digests_by_authorities.safe_range_iter((
            Included((author, start_round, BlockDigest::MIN)),
            Included((author, Round::MAX, BlockDigest::MAX)),
        )) {
            let ((author, round, digest), _) = kv?;
            refs.push(BlockRef::new(round, author, digest));
        }
        let results = self.read_blocks(refs.as_slice())?;
        let mut blocks = Vec::with_capacity(refs.len());
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            blocks.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {r:?} not found!")),
            );
        }
        Ok(blocks)
    }

    // Reads scoring metrics from the v2 CF (single blob under key 0). If not found,
    // falls back to the legacy per-authority `StorageScoringMetrics` CF, and
    // reconstructs a single `VersionedScoringMetrics` blob. The legacy migration
    // logic should be deleted after `StorageScoringMetrics` is removed.
    fn scan_scoring_metrics(
        &self,
        committee: &Committee,
    ) -> ConsensusResult<Option<VersionedScoringMetrics>> {
        // Try to read the single blob from the v2 CF first.
        if let Some(metrics) = self.scoring_metrics_v2.get(&())? {
            return Ok(Some(metrics));
        }

        // Fall back to v1 (per-authority) CF.
        let mut legacy_entries = vec![];
        #[allow(deprecated)]
        for kv in self.scoring_metrics.safe_iter() {
            legacy_entries.push(kv?);
        }

        if legacy_entries.is_empty() {
            return Ok(None);
        }

        // If v1 (per-authority) CF is not empty, return the data migrated to v2 format.
        // Note that we do not write the migrated data to the v2 CF here, since the data
        // will be added to the v2 CF after the first write.
        let scoring_metrics_v2 = migrate_stored_metrics(legacy_entries, committee);

        Ok(Some(scoring_metrics_v2))
    }

    // The method returns the last `num_of_rounds` rounds blocks by author in round
    // ascending order. When a `before_round` is defined then the blocks of
    // round `<=before_round` are returned. If not then the max value for round
    // will be used as cut off.
    fn scan_last_blocks_by_author(
        &self,
        author: AuthorityIndex,
        num_of_rounds: u64,
        before_round: Option<Round>,
    ) -> ConsensusResult<Vec<VerifiedBlock>> {
        let before_round = before_round.unwrap_or(Round::MAX);
        let mut refs = std::collections::VecDeque::new();
        for kv in self
            .digests_by_authorities
            .reversed_safe_iter_with_bounds(
                Some((author, Round::MIN, BlockDigest::MIN)),
                Some((author, before_round, BlockDigest::MAX)),
            )?
            .take(num_of_rounds as usize)
        {
            let ((author, round, digest), _) = kv?;
            refs.push_front(BlockRef::new(round, author, digest));
        }
        let results = self.read_blocks(refs.as_slices().0)?;
        let mut blocks = vec![];
        for (r, block) in refs.into_iter().zip(results.into_iter()) {
            blocks.push(
                block.unwrap_or_else(|| panic!("Storage inconsistency: block {r:?} not found!")),
            );
        }
        Ok(blocks)
    }

    fn read_last_commit(&self) -> ConsensusResult<Option<TrustedCommit>> {
        let Some(result) = self
            .commits
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
        else {
            return Ok(None);
        };
        let ((_index, digest), serialized) = result?;
        let commit = TrustedCommit::new_trusted(
            bcs::from_bytes(&serialized).map_err(ConsensusError::MalformedCommit)?,
            serialized,
        );
        assert_eq!(commit.digest(), digest);
        Ok(Some(commit))
    }

    fn scan_commits(&self, range: CommitRange) -> ConsensusResult<Vec<TrustedCommit>> {
        let mut commits = vec![];
        for result in self.commits.safe_range_iter((
            Included((range.start(), CommitDigest::MIN)),
            Included((range.end(), CommitDigest::MAX)),
        )) {
            let ((_index, digest), serialized) = result?;
            let commit = TrustedCommit::new_trusted(
                bcs::from_bytes(&serialized).map_err(ConsensusError::MalformedCommit)?,
                serialized,
            );
            assert_eq!(commit.digest(), digest);
            commits.push(commit);
        }
        Ok(commits)
    }

    fn read_commit_votes(&self, commit_index: CommitIndex) -> ConsensusResult<Vec<BlockRef>> {
        let mut votes = Vec::new();
        for vote in self.commit_votes.safe_range_iter((
            Included((commit_index, CommitDigest::MIN, BlockRef::MIN)),
            Included((commit_index, CommitDigest::MAX, BlockRef::MAX)),
        )) {
            let ((_, _, block_ref), _) = vote?;
            votes.push(block_ref);
        }
        Ok(votes)
    }

    fn read_last_commit_info(&self) -> ConsensusResult<Option<(CommitRef, CommitInfo)>> {
        let Some(result) = self
            .commit_info
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
        else {
            return Ok(None);
        };
        let (key, commit_info) = result.map_err(ConsensusError::RocksDBFailure)?;
        Ok(Some((CommitRef::new(key.0, key.1), commit_info)))
    }
}

// TODO: delete this function after the migration is done and the legacy
// `StorageScoringMetrics` field is removed.
fn migrate_stored_metrics(
    legacy_entries: Vec<(AuthorityIndex, StorageScoringMetrics)>,
    committee: &Committee,
) -> VersionedScoringMetrics {
    let committee_size = committee.size();

    // Reconstruct vectors from per-authority entries.
    let mut faulty_blocks_provable = vec![0u64; committee_size];
    let mut faulty_blocks_unprovable = vec![0u64; committee_size];
    let mut missing_proposals = vec![0u64; committee_size];
    let mut equivocations = vec![0u64; committee_size];

    for (authority_index, old) in &legacy_entries {
        let idx = authority_index.value();
        faulty_blocks_provable[idx] = old.faulty_blocks_provable;
        faulty_blocks_unprovable[idx] = old.faulty_blocks_unprovable;
        missing_proposals[idx] = old.missing_proposals;
        equivocations[idx] = old.equivocations;
    }

    let blob = VersionedScoringMetrics::V1(
        MisbehaviorsV1::new(
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        )
        .as_atomic(),
    );

    tracing::info!(
        "Migrating {} legacy scoring metrics entries to single-blob format",
        legacy_entries.len()
    );

    blob
}

// The following method is only used for testing and simulates the existence of
// legacy-format scoring metrics in the old `StorageScoringMetrics` field. It
// should be removed after the migration is done and the legacy field is
// deleted.
#[cfg(test)]
impl RocksDBStore {
    /// Writes legacy-format scoring metrics directly to the old CF, simulating
    /// data written by a pre-upgrade node.
    pub(crate) fn write_legacy_scoring_metrics(
        &self,
        entries: Vec<(AuthorityIndex, [u64; 4])>,
    ) -> ConsensusResult<()> {
        #[allow(deprecated)]
        let mut batch = self.scoring_metrics.batch();
        for (authority, [fbp, fbu, missing, equiv]) in entries {
            let legacy = StorageScoringMetrics {
                faulty_blocks_provable: fbp,
                faulty_blocks_unprovable: fbu,
                missing_proposals: missing,
                equivocations: equiv,
            };
            #[allow(deprecated)]
            batch
                .insert_batch(&self.scoring_metrics, [(authority, legacy)])
                .map_err(ConsensusError::RocksDBFailure)?;
        }
        batch.write()?;
        Ok(())
    }
}

/// Tests all scan_scoring_metrics scenarios: empty store, legacy-only,
/// legacy updates, v2-only, v2 updates, and v2 taking priority over legacy.
#[tokio::test]
async fn scan_scoring_metrics_legacy_migration() {
    use consensus_config::{AuthorityIndex, Stake, local_committee_and_keys};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let store = RocksDBStore::new(temp_dir.path().to_str().unwrap());
    let epoch = 100;
    let authority_stakes = (1..=2).map(|s| s as Stake).collect();
    let (committee, _) = local_committee_and_keys(epoch, authority_stakes);

    // Case 1: Empty store — no legacy, no v2. Should return None.
    let result = store.scan_scoring_metrics(&committee).unwrap();
    assert!(result.is_none());

    // Case 2: Only legacy data — falls back to legacy CF and returns migrated data.
    // Array order: [faulty_blocks_provable, faulty_blocks_unprovable,
    // missing_proposals, equivocations].
    store
        .write_legacy_scoring_metrics(vec![
            (AuthorityIndex::new_for_test(0), [10, 20, 30, 40]),
            (AuthorityIndex::new_for_test(1), [0, 0, 0, 0]),
        ])
        .unwrap();

    let scanned = store.scan_scoring_metrics(&committee).unwrap().unwrap();
    assert_eq!(scanned.load_faulty_blocks_provable(), vec![10, 0]);
    assert_eq!(scanned.load_faulty_blocks_unprovable(), vec![20, 0]);
    assert_eq!(scanned.load_missing_proposals(), vec![30, 0]);
    assert_eq!(scanned.load_equivocations(), vec![40, 0]);

    // Case 3: Legacy data updated — scan does not cache to v2, so the updated
    // legacy values are returned.
    store
        .write_legacy_scoring_metrics(vec![
            (AuthorityIndex::new_for_test(0), [50, 60, 70, 80]),
            (AuthorityIndex::new_for_test(1), [0, 0, 0, 0]),
        ])
        .unwrap();

    let scanned = store.scan_scoring_metrics(&committee).unwrap().unwrap();
    assert_eq!(scanned.load_faulty_blocks_provable(), vec![50, 0]);
    assert_eq!(scanned.load_faulty_blocks_unprovable(), vec![60, 0]);
    assert_eq!(scanned.load_missing_proposals(), vec![70, 0]);
    assert_eq!(scanned.load_equivocations(), vec![80, 0]);

    // Case 4: Write v2 data via WriteBatch. V2 should now take priority over
    // legacy.
    let v2_blob = VersionedScoringMetrics::V1(
        MisbehaviorsV1::new(vec![1, 2], vec![3, 4], vec![5, 6], vec![7, 8]).as_atomic(),
    );
    store
        .write(WriteBatch::default().scoring_metrics(v2_blob.snapshot()))
        .unwrap();

    let scanned = store.scan_scoring_metrics(&committee).unwrap().unwrap();
    assert_eq!(scanned.load_faulty_blocks_provable(), vec![1, 2]);
    assert_eq!(scanned.load_faulty_blocks_unprovable(), vec![3, 4]);
    assert_eq!(scanned.load_missing_proposals(), vec![5, 6]);
    assert_eq!(scanned.load_equivocations(), vec![7, 8]);

    // Case 5: Update legacy while v2 exists — v2 still takes priority,
    // legacy changes are ignored.
    store
        .write_legacy_scoring_metrics(vec![
            (AuthorityIndex::new_for_test(0), [99, 99, 99, 99]),
            (AuthorityIndex::new_for_test(1), [99, 99, 99, 99]),
        ])
        .unwrap();

    let scanned = store.scan_scoring_metrics(&committee).unwrap().unwrap();
    assert_eq!(scanned.load_faulty_blocks_provable(), vec![1, 2]);
    assert_eq!(scanned.load_faulty_blocks_unprovable(), vec![3, 4]);
    assert_eq!(scanned.load_missing_proposals(), vec![5, 6]);
    assert_eq!(scanned.load_equivocations(), vec![7, 8]);

    // Case 6: Update v2 data — scan returns the updated v2 values.
    let v2_updated = VersionedScoringMetrics::V1(
        MisbehaviorsV1::new(vec![11, 22], vec![33, 44], vec![55, 66], vec![77, 88]).as_atomic(),
    );
    store
        .write(WriteBatch::default().scoring_metrics(v2_updated.snapshot()))
        .unwrap();

    let scanned = store.scan_scoring_metrics(&committee).unwrap().unwrap();
    assert_eq!(scanned.load_faulty_blocks_provable(), vec![11, 22]);
    assert_eq!(scanned.load_faulty_blocks_unprovable(), vec![33, 44]);
    assert_eq!(scanned.load_missing_proposals(), vec![55, 66]);
    assert_eq!(scanned.load_equivocations(), vec![77, 88]);
}
