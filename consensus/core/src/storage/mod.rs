// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
pub(crate) mod mem_store;
pub(crate) mod rocksdb_store;

#[cfg(test)]
mod store_tests;

use consensus_config::AuthorityIndex;
use iota_common::{misbehavior_counts::MisbehaviorsV1, scoring_metrics::VersionedScoringMetrics};
use iota_protocol_config::ProtocolConfig;
use serde::{Deserialize, Serialize};

use crate::{
    CommitIndex,
    block::{BlockRef, Round, VerifiedBlock},
    commit::{CommitInfo, CommitRange, CommitRef, TrustedCommit},
    error::ConsensusResult,
};

/// A common interface for consensus storage.
pub(crate) trait Store: Send + Sync {
    /// Writes blocks, consensus commits and other data to store atomically.
    fn write(&self, write_batch: WriteBatch) -> ConsensusResult<()>;

    /// Reads blocks for the given refs.
    fn read_blocks(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<Option<VerifiedBlock>>>;

    /// Checks if blocks exist in the store.
    fn contains_blocks(&self, refs: &[BlockRef]) -> ConsensusResult<Vec<bool>>;

    /// Checks whether there is any block at the given slot
    #[allow(dead_code)]
    fn contains_block_at_slot(&self, slot: crate::block::Slot) -> ConsensusResult<bool>;

    /// Reads blocks for an authority, from start_round.
    fn scan_blocks_by_author(
        &self,
        authority: AuthorityIndex,
        start_round: Round,
    ) -> ConsensusResult<Vec<VerifiedBlock>>;

    // The method reads and returns all metrics stored. Used for restoring the
    // scoring metrics in case of DagState initialization from storage
    fn scan_scoring_metrics(
        &self,
    ) -> ConsensusResult<Vec<(AuthorityIndex, VersionedStorageScoringMetrics)>>;

    // The method returns the last `num_of_rounds` rounds blocks by author in round
    // ascending order. When a `before_round` is defined then the blocks of
    // round `<=before_round` are returned. If not then the max value for round
    // will be used as cut off.
    #[allow(dead_code)]
    fn scan_last_blocks_by_author(
        &self,
        author: AuthorityIndex,
        num_of_rounds: u64,
        before_round: Option<Round>,
    ) -> ConsensusResult<Vec<VerifiedBlock>>;

    /// Reads the last commit.
    fn read_last_commit(&self) -> ConsensusResult<Option<TrustedCommit>>;

    /// Reads all commits from start (inclusive) until end (inclusive).
    fn scan_commits(&self, range: CommitRange) -> ConsensusResult<Vec<TrustedCommit>>;

    /// Reads all blocks voting on a particular commit.
    fn read_commit_votes(&self, commit_index: CommitIndex) -> ConsensusResult<Vec<BlockRef>>;

    /// Reads the last commit info, written atomically with the last commit.
    fn read_last_commit_info(&self) -> ConsensusResult<Option<(CommitRef, CommitInfo)>>;
}

/// Represents data to be written to the store together atomically.
#[derive(Debug, Default)]
pub(crate) struct WriteBatch {
    pub(crate) blocks: Vec<VerifiedBlock>,
    pub(crate) commits: Vec<TrustedCommit>,
    pub(crate) commit_info: Vec<(CommitRef, CommitInfo)>,
    pub(crate) scoring_metrics: Vec<(AuthorityIndex, VersionedStorageScoringMetrics)>,
}

impl WriteBatch {
    pub(crate) fn new(
        blocks: Vec<VerifiedBlock>,
        commits: Vec<TrustedCommit>,
        commit_info: Vec<(CommitRef, CommitInfo)>,
        scoring_metrics: Vec<(AuthorityIndex, VersionedStorageScoringMetrics)>,
    ) -> Self {
        WriteBatch {
            blocks,
            commits,
            commit_info,
            scoring_metrics,
        }
    }

    // Test setters.

    #[cfg(test)]
    pub(crate) fn blocks(mut self, blocks: Vec<VerifiedBlock>) -> Self {
        self.blocks = blocks;
        self
    }

    #[cfg(test)]
    pub(crate) fn commits(mut self, commits: Vec<TrustedCommit>) -> Self {
        self.commits = commits;
        self
    }

    #[cfg(test)]
    pub(crate) fn commit_info(mut self, commit_info: Vec<(CommitRef, CommitInfo)>) -> Self {
        self.commit_info = commit_info;
        self
    }

    #[cfg(test)]
    pub(crate) fn scoring_metrics(
        mut self,
        scoring_metrics: Vec<(AuthorityIndex, VersionedStorageScoringMetrics)>,
    ) -> Self {
        self.scoring_metrics = scoring_metrics;
        self
    }
}

// Re-exportMisbehaviorsV1<u64> as StorageScoringMetrics for storage use.
pub(crate) type StorageScoringMetricsV1 = MisbehaviorsV1<u64>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum VersionedStorageScoringMetrics {
    V1(StorageScoringMetricsV1),
}

impl VersionedStorageScoringMetrics {
    pub fn new_zeroed(protocol_config: &ProtocolConfig) -> Self {
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => {
                VersionedStorageScoringMetrics::V1(StorageScoringMetricsV1::new_zeroed())
            }
            _ => panic!("Unsupported scorer version"),
        }
    }

    pub fn new_from(scoring_metrics: &VersionedScoringMetrics, authority_index: usize) -> Self {
        match scoring_metrics {
            VersionedScoringMetrics::V1(misbehavior_vectors) => {
                let inner = misbehavior_vectors.misbehaviors_from_authority(authority_index);
                VersionedStorageScoringMetrics::V1(inner)
            }
        }
    }

    /// Returns an iterator over references to the metric values.
    pub fn iterate_over_metrics(&self) -> std::vec::IntoIter<&u64> {
        match self {
            VersionedStorageScoringMetrics::V1(inner) => inner.iter(),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_v1_for_test(
        faulty_blocks_provable: u64,
        faulty_blocks_unprovable: u64,
        missing_proposals: u64,
        equivocations: u64,
    ) -> Self {
        VersionedStorageScoringMetrics::V1(StorageScoringMetricsV1::new(
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        ))
    }
}
