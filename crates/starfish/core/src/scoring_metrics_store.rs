// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use starfish_config::AuthorityIndex;

use crate::{
    block_header::{BlockRef, Round},
    context::Context,
    metrics::NodeMetrics,
};

/// Per-authority misbehavior counters.
///
/// Two buckets track different lifecycle stages:
/// - `in_memory`: from blocks currently in the DAG cache (volatile, recomputed
///   on each flush from blocks still in cache).
/// - `persisted`: cumulative counts from blocks evicted from cache (restored
///   from storage on restart).
pub(crate) struct MisbehaviorStore {
    in_memory: CommitteeMisbehaviorCounts,
    persisted: CommitteeMisbehaviorCounts,
}

enum BucketSource {
    InMemory,
    Persisted,
}

impl MisbehaviorStore {
    pub(crate) fn new(committee_size: usize) -> Self {
        Self {
            in_memory: CommitteeMisbehaviorCounts::new(committee_size),
            persisted: CommitteeMisbehaviorCounts::new(committee_size),
        }
    }

    /// Restores persisted counts from storage and computes in-memory counts
    /// from the block refs already loaded into the DAG cache.
    pub(crate) fn initialize_misbehavior_counts(
        &self,
        recovered: BTreeMap<AuthorityIndex, MisbehaviorCounts>,
        recent_refs_by_authority: &[BTreeSet<BlockRef>],
        evicted_rounds: &[Round],
        threshold_clock_round: Round,
        context: &Arc<Context>,
    ) {
        let node_metrics = &context.metrics.node_metrics;

        for (authority_index, authority) in context.committee.authorities() {
            let hostname = authority.hostname.as_str();
            let idx = authority_index.value();

            // Restore persisted counts from storage.
            let storage_metrics = recovered.get(&authority_index).cloned().unwrap_or_default();
            match &storage_metrics {
                MisbehaviorCounts::V1(inner) => {
                    self.initialize_faulty_blocks_metrics(
                        inner.faulty_blocks_provable,
                        inner.faulty_blocks_unprovable,
                        hostname,
                        authority_index,
                        node_metrics,
                    );
                    self.update_missing_blocks_and_equivocations(
                        inner.missing_proposals,
                        inner.equivocations,
                        hostname,
                        authority_index,
                        BucketSource::Persisted,
                        node_metrics,
                    );
                }
            }

            // Compute in-memory counts from cached block refs.
            let eviction_round = evicted_rounds[idx];
            if threshold_clock_round > 0 {
                let cached_rounds: Vec<Round> = recent_refs_by_authority[idx]
                    .iter()
                    .map(|r| r.round)
                    .collect();
                let (eq, missing) = calculate_misbehavior_counts_for_range(
                    cached_rounds,
                    eviction_round + 1,
                    threshold_clock_round.saturating_sub(1),
                );
                self.update_missing_blocks_and_equivocations(
                    missing,
                    eq,
                    hostname,
                    authority_index,
                    BucketSource::InMemory,
                    node_metrics,
                );
            }
        }
    }

    fn initialize_faulty_blocks_metrics(
        &self,
        faulty_provable: u64,
        faulty_unprovable: u64,
        hostname: &str,
        authority: AuthorityIndex,
        node_metrics: &NodeMetrics,
    ) {
        self.persisted.update(authority.value(), |c| {
            c.faulty_blocks_provable = faulty_provable;
            c.faulty_blocks_unprovable = faulty_unprovable;
        });
        node_metrics
            .faulty_blocks_provable_by_authority
            .with_label_values(&[hostname, "persisted"])
            .set(faulty_provable as i64);
        node_metrics
            .faulty_blocks_unprovable_by_peer
            .with_label_values(&[hostname, "persisted"])
            .set(faulty_unprovable as i64);
    }

    fn update_missing_blocks_and_equivocations(
        &self,
        missing_blocks: u64,
        equivocations: u64,
        hostname: &str,
        authority: AuthorityIndex,
        bucket: BucketSource,
        node_metrics: &NodeMetrics,
    ) {
        let idx = authority.value();
        match bucket {
            BucketSource::InMemory => {
                self.in_memory.update(idx, |c| {
                    c.equivocations = equivocations;
                    c.missing_proposals = missing_blocks;
                });
                node_metrics
                    .equivocations_by_authority
                    .with_label_values(&[hostname, "in_memory"])
                    .set(equivocations as i64);
                node_metrics
                    .missing_proposals_by_authority
                    .with_label_values(&[hostname, "in_memory"])
                    .set(missing_blocks as i64);
            }
            BucketSource::Persisted => {
                self.persisted.update(idx, |c| {
                    c.equivocations += equivocations;
                    c.missing_proposals += missing_blocks;
                });
                node_metrics
                    .equivocations_by_authority
                    .with_label_values(&[hostname, "persisted"])
                    .add(equivocations as i64);
                node_metrics
                    .missing_proposals_by_authority
                    .with_label_values(&[hostname, "persisted"])
                    .add(missing_blocks as i64);
            }
        }
    }

    /// Updates misbehavior counts for one authority during flush.
    /// Must be called before cache eviction is applied, while evicted block
    /// refs are still in `recent_refs`.
    /// Returns `Some(MisbehaviorCounts)` if the eviction boundary advanced
    /// (meaning new data needs to be written to storage).
    pub(crate) fn update_misbehavior_counts_on_eviction(
        &self,
        authority_index: AuthorityIndex,
        hostname: &str,
        recent_refs: &BTreeSet<BlockRef>,
        eviction_round: Round,
        last_eviction_round: Round,
        threshold_clock_round: Round,
        context: &Arc<Context>,
    ) -> Option<MisbehaviorCounts> {
        let node_metrics = &context.metrics.node_metrics;

        if threshold_clock_round == 0 || authority_index.value() >= context.committee.size() {
            return None;
        }

        // Recompute in-memory window from blocks still in cache.
        let in_memory_block_rounds: Vec<Round> = recent_refs
            .iter()
            .map(|b| b.round)
            .filter(|&r| r > eviction_round && r < threshold_clock_round)
            .collect();
        let (in_memory_eq, in_memory_missing) = calculate_misbehavior_counts_for_range(
            in_memory_block_rounds,
            eviction_round + 1,
            threshold_clock_round.saturating_sub(1),
        );
        self.update_missing_blocks_and_equivocations(
            in_memory_missing,
            in_memory_eq,
            hostname,
            authority_index,
            BucketSource::InMemory,
            node_metrics,
        );

        if eviction_round == last_eviction_round {
            return None;
        }

        // Accumulate newly-evicted rounds into persisted.
        let evicted_block_rounds: Vec<Round> = recent_refs
            .iter()
            .map(|b| b.round)
            .filter(|&r| r <= eviction_round)
            .collect();
        let (evicted_eq, evicted_missing) = calculate_misbehavior_counts_for_range(
            evicted_block_rounds,
            last_eviction_round + 1,
            eviction_round,
        );
        self.update_missing_blocks_and_equivocations(
            evicted_missing,
            evicted_eq,
            hostname,
            authority_index,
            BucketSource::Persisted,
            node_metrics,
        );

        Some(self.persisted.to_storage(authority_index.value()))
    }
}

/// Per-authority misbehavior counters, one `Mutex<MisbehaviorCountsV1>`
/// per authority. Uses per-authority Mutex for interior mutability and to
/// support future concurrent writes from block validation threads.
struct CommitteeMisbehaviorCounts {
    authorities: Vec<Mutex<MisbehaviorCountsV1>>,
}

impl CommitteeMisbehaviorCounts {
    fn new(committee_size: usize) -> Self {
        Self {
            authorities: (0..committee_size)
                .map(|_| Mutex::new(MisbehaviorCountsV1::default()))
                .collect(),
        }
    }

    fn get(&self, authority: usize) -> MisbehaviorCountsV1 {
        self.authorities[authority].lock().unwrap().clone()
    }

    fn update(&self, authority: usize, f: impl FnOnce(&mut MisbehaviorCountsV1)) {
        f(&mut self.authorities[authority].lock().unwrap());
    }

    fn to_storage(&self, authority: usize) -> MisbehaviorCounts {
        MisbehaviorCounts::V1(self.get(authority))
    }

    #[cfg(test)]
    fn collect<F: Fn(&MisbehaviorCountsV1) -> u64>(&self, field: F) -> Vec<u64> {
        self.authorities
            .iter()
            .map(|m| field(&m.lock().unwrap()))
            .collect()
    }
}

/// Given block rounds for one authority in [start, end], returns
/// (equivocations, missing_proposals).
fn calculate_misbehavior_counts_for_range(
    mut block_rounds: Vec<Round>,
    start: Round,
    end: Round,
) -> (u64, u64) {
    block_rounds.retain(|&round| round >= start && round <= end);
    block_rounds.sort();
    let number_of_blocks = block_rounds.len();
    block_rounds.dedup();
    let unique_block_rounds = block_rounds.len();
    let number_of_equivocations = number_of_blocks.saturating_sub(unique_block_rounds) as u64;
    let number_of_missing_blocks =
        (end + 1).saturating_sub(start + unique_block_rounds as u32) as u64;
    (number_of_equivocations, number_of_missing_blocks)
}

/// Versioned envelope for persisted scoring metrics. New versions are added as
/// enum variants so existing RocksDB data deserializes without migration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum MisbehaviorCounts {
    V1(MisbehaviorCountsV1),
}

impl Default for MisbehaviorCounts {
    fn default() -> Self {
        Self::V1(MisbehaviorCountsV1::default())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct MisbehaviorCountsV1 {
    pub(crate) faulty_blocks_provable: u64,
    pub(crate) faulty_blocks_unprovable: u64,
    pub(crate) missing_proposals: u64,
    pub(crate) equivocations: u64,
}

#[cfg(test)]
impl MisbehaviorStore {
    pub(crate) fn dummy_for_test(committee_size: usize) -> Self {
        Self::new(committee_size)
    }

    pub(crate) fn persisted_missing_proposals(&self) -> Vec<u64> {
        self.persisted.collect(|c| c.missing_proposals)
    }

    pub(crate) fn persisted_equivocations(&self) -> Vec<u64> {
        self.persisted.collect(|c| c.equivocations)
    }

    pub(crate) fn in_memory_missing_proposals(&self) -> Vec<u64> {
        self.in_memory.collect(|c| c.missing_proposals)
    }

    pub(crate) fn in_memory_equivocations(&self) -> Vec<u64> {
        self.in_memory.collect(|c| c.equivocations)
    }
}

#[cfg(test)]
impl MisbehaviorCounts {
    pub(crate) fn new_v1_for_test(
        faulty_blocks_provable: u64,
        faulty_blocks_unprovable: u64,
        missing_proposals: u64,
        equivocations: u64,
    ) -> Self {
        Self::V1(MisbehaviorCountsV1 {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use starfish_config::Parameters;

    use super::*;
    use crate::{
        context::Context,
        dag_state::{DagState, DataSource},
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
    };

    #[test]
    fn test_calculate_misbehavior_counts_for_range_basic() {
        // No blocks in range → all missing, no equivocations
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![], 1, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 5);

        // All rounds present, no equivocations
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![1, 2, 3, 4, 5], 1, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);

        // One equivocation (duplicate round 3)
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![1, 2, 3, 3, 4, 5], 1, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 0);

        // One missing (round 3) + one equivocation (round 2)
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![1, 2, 2, 4, 5], 1, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 1);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_filters_out_of_range() {
        // Rounds outside [2, 4] are filtered
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![1, 2, 3, 4, 5], 2, 4);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_empty_range() {
        // start > end → no missing, no equivocations (saturating_sub handles it)
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![], 5, 3);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_unsorted_input() {
        // Unsorted with one equivocation (round 3 appears twice) and one missing (round 5)
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![4, 1, 3, 2, 3], 1, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 1);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_multiple_equivocations() {
        // Round 2 appears 3 times (2 equivocations), round 4 appears twice (1 equivocation)
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![1, 2, 2, 2, 3, 4, 4], 1, 4);
        assert_eq!(eq, 3);
        assert_eq!(missing, 0);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_single_round() {
        // Single-round range with block present
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![5], 5, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);

        // Single-round range with no block
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![], 5, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 1);

        // Single-round range with equivocation
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![5, 5], 5, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 0);
    }

    #[tokio::test]
    async fn test_update_misbehavior_counts_on_eviction_edge_cases() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(4);
        let authority_index = AuthorityIndex::new_for_test(0);
        let hostname = "test_host";
        let recent_refs = BTreeSet::new();

        // threshold_clock_round=0 → always returns None
        let result = store.update_misbehavior_counts_on_eviction(
            authority_index,
            hostname,
            &recent_refs,
            0,
            0,
            0,
            &context,
        );
        assert!(result.is_none());

        // No eviction (eviction_round == last_eviction_round) → None
        let result = store.update_misbehavior_counts_on_eviction(
            authority_index,
            hostname,
            &recent_refs,
            5,
            5,
            5,
            &context,
        );
        assert!(result.is_none());

        // Eviction happened with empty refs → missing proposals accumulated
        let result = store.update_misbehavior_counts_on_eviction(
            authority_index,
            hostname,
            &recent_refs,
            3,
            0,
            2,
            &context,
        );
        assert_eq!(result, Some(MisbehaviorCounts::new_v1_for_test(0, 0, 3, 0)));

        // Out-of-bounds authority → None
        let oob = AuthorityIndex::new_for_test(4);
        let result = store.update_misbehavior_counts_on_eviction(
            oob,
            hostname,
            &recent_refs,
            2,
            1,
            3,
            &context,
        );
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_metrics_flush_and_recovery() {
        let committee_size = 4;
        let (context, _) = Context::new_for_test(committee_size);
        let context = context.with_parameters(Parameters {
            dag_state_cached_rounds: 5,
            ..Default::default()
        });
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Build a 20-round DAG:
        // - Rounds 6-8: authority 0 skips proposals
        // - Round 11: authority 1 equivocates (1 extra block)
        // - Round 13: authority 2 equivocates (2 extra blocks)
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=5).build();
        dag_builder
            .layers(6..=8)
            .authorities(vec![AuthorityIndex::new_for_test(0)])
            .skip_block()
            .build();
        dag_builder.layers(9..=10).build();
        dag_builder
            .layers(11..=11)
            .authorities(vec![AuthorityIndex::new_for_test(1)])
            .equivocate(1)
            .build();
        dag_builder.layers(12..=12).build();
        dag_builder
            .layers(13..=13)
            .authorities(vec![AuthorityIndex::new_for_test(2)])
            .equivocate(2)
            .build();
        dag_builder.layers(14..=20).build();

        let mut commits = dag_builder
            .get_sub_dag_and_commits(1..=20)
            .into_iter()
            .map(|(_subdag, commit)| commit)
            .collect::<Vec<_>>();

        // Accept blocks+commits for first 10 rounds
        let temp_commits = commits.split_off(9);
        dag_state.accept_block_headers(dag_builder.block_headers(1..=10), DataSource::Test);
        for commit in commits {
            dag_state.add_commit(commit);
        }

        // Metrics should be zero before flush
        let scoring = dag_state.misbehavior_store();
        assert_eq!(scoring.persisted_equivocations(), vec![0; committee_size]);
        assert_eq!(
            scoring.persisted_missing_proposals(),
            vec![0; committee_size]
        );

        // Flush — this triggers misbehavior_counts_to_write
        dag_state.flush();

        // After flush: authority 0 should have missing proposals in the in-memory
        // window (cached rounds 6-8 where it didn't propose).
        // No equivocations yet (those are in rounds 11+, not accepted yet).
        let scoring = dag_state.misbehavior_store();
        assert_eq!(scoring.persisted_equivocations(), vec![0; committee_size]);
        assert!(scoring.in_memory_missing_proposals()[0] > 0);

        // Drop and recover from storage
        let persisted_missing_before = scoring.persisted_missing_proposals();
        let in_memory_missing_before_drop = scoring.in_memory_missing_proposals();
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Persisted metrics should be restored, and in-memory recomputed from
        // the cached block refs loaded during recovery.
        let scoring = dag_state.misbehavior_store();
        assert_eq!(
            scoring.persisted_missing_proposals(),
            persisted_missing_before
        );
        // In-memory is recomputed from cached refs on init (not zero).
        let in_memory_after_recovery = scoring.in_memory_missing_proposals();
        assert_eq!(
            in_memory_after_recovery[0], in_memory_missing_before_drop[0],
            "In-memory should be recomputed from cached refs on startup"
        );

        // Accept rounds 11-20 and flush
        dag_state.accept_block_headers(dag_builder.block_headers(11..=20), DataSource::Test);
        for commit in temp_commits {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Now equivocations should be tracked
        let scoring = dag_state.misbehavior_store();
        let total_eq: u64 = scoring.persisted_equivocations().iter().sum::<u64>()
            + scoring.in_memory_equivocations().iter().sum::<u64>();
        assert!(total_eq > 0, "Should have detected equivocations");
    }

    #[tokio::test]
    async fn test_no_double_counting_on_restart() {
        let committee_size = 4;
        let (context, _) = Context::new_for_test(committee_size);
        let context = context.with_parameters(Parameters {
            dag_state_cached_rounds: 5,
            ..Default::default()
        });
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Build a DAG where authority 0 skips rounds 3-4
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=2).build();
        dag_builder
            .layers(3..=4)
            .authorities(vec![AuthorityIndex::new_for_test(0)])
            .skip_block()
            .build();
        dag_builder.layers(5..=10).build();

        let commits = dag_builder
            .get_sub_dag_and_commits(1..=10)
            .into_iter()
            .map(|(_subdag, commit)| commit)
            .collect::<Vec<_>>();

        dag_state.accept_block_headers(dag_builder.block_headers(1..=10), DataSource::Test);
        for commit in commits {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Record persisted counts after first flush
        let persisted_after_first_flush =
            dag_state.misbehavior_store().persisted_missing_proposals();
        drop(dag_state);

        // Recover from storage and flush again without new blocks
        let mut dag_state = DagState::new(context.clone(), store.clone());
        dag_state.flush();

        // Persisted counts must NOT have doubled
        let persisted_after_restart_flush =
            dag_state.misbehavior_store().persisted_missing_proposals();
        assert_eq!(
            persisted_after_first_flush, persisted_after_restart_flush,
            "Persisted counts should not double on restart + flush"
        );
    }
}
