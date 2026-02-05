// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::Ordering},
};

use iota_common::scoring_metrics::{VersionedScoringMetrics, VersionedStorageScoringMetrics};
use iota_protocol_config::ProtocolConfig;
use itertools::izip;
use starfish_config::AuthorityIndex;

use crate::{BlockRef, context::Context, metrics::NodeMetrics};

/// Struct that holds the scoring metrics for all authorities in the committee,
/// both cached and uncached. It also holds a shared reference to the current
/// local metrics count used by Scorer.
pub(crate) struct ScoringMetricsStore {
    pub current_local_metrics_count: Arc<VersionedScoringMetrics>,
    pub cached_metrics: VersionedScoringMetrics,
    pub uncached_metrics: VersionedScoringMetrics,
}

impl ScoringMetricsStore {
    pub(crate) fn new(
        committee_size: usize,
        current_local_metrics_count: Arc<VersionedScoringMetrics>,
        protocol_config: &ProtocolConfig,
    ) -> Self {
        Self {
            current_local_metrics_count,
            cached_metrics: VersionedScoringMetrics::new(committee_size, protocol_config),
            uncached_metrics: VersionedScoringMetrics::new(committee_size, protocol_config),
        }
    }

    // Initializes the scoring metrics store according to the
    // recovered_scoring_metrics and headers_in_cache_by_authority.
    pub(crate) fn initialize_scoring_metrics(
        &self,
        mut recovered_scoring_metrics: Vec<(AuthorityIndex, VersionedStorageScoringMetrics)>,
        headers_in_cache_by_authority: &Vec<BTreeSet<BlockRef>>,
        threshold_clock_round: u32,
        eviction_rounds: &Vec<u32>,
        context: &Arc<Context>,
    ) {
        // It is possible that the vector recovered_scoring_metrics does not have a
        // component for every authority. A perfectly functioning validator, for
        // example, will never have its metrics updated, so no metric will ever be
        // stored. For this reason, we manually "fill" this vector.
        if recovered_scoring_metrics.len() < context.committee.size() {
            for (i, _) in context.committee.authorities() {
                if !recovered_scoring_metrics
                    .iter()
                    .any(|(index, _)| *index == i)
                {
                    // We add a component with zeroed metrics for the authority with index i.
                    // This will ensure that every authority has its metrics initialized.
                    // They are initialized as zero because if an authority does not have any
                    // recovered metrics, it means that it never misbehaved in a way that was
                    // detected by the node.
                    recovered_scoring_metrics.insert(
                        i.value(),
                        (
                            i,
                            VersionedStorageScoringMetrics::new_zeroed(&context.protocol_config),
                        ),
                    );
                }
            }
        }

        match &context.protocol_config.scorer_version_as_option() {
            None | Some(1) => self.initialize_scoring_metrics_v1(
                recovered_scoring_metrics,
                headers_in_cache_by_authority,
                threshold_clock_round,
                eviction_rounds,
                context,
            ),
            _ => panic!("Unsupported scorer version"),
        }
    }

    // Initializes the scoring metrics store according to the
    // recovered_scoring_metrics and headers_in_cache_by_authority, for scorer
    // version 1. This function should only be called if the scorer version in the
    // protocol config is 1 or None (i.e., defaulting to version 1), otherwise it
    // will panic.
    pub(crate) fn initialize_scoring_metrics_v1(
        &self,
        recovered_scoring_metrics: Vec<(AuthorityIndex, VersionedStorageScoringMetrics)>,
        headers_in_cache_by_authority: &Vec<BTreeSet<BlockRef>>,
        threshold_clock_round: u32,
        eviction_rounds: &Vec<u32>,
        context: &Arc<Context>,
    ) {
        let hostnames = context
            .committee
            .authorities()
            .map(|(_, x)| x.hostname.as_str())
            .collect::<Vec<_>>();

        for ((authority_index, metrics), hostname, headers_in_cache, &eviction_round) in izip!(
            recovered_scoring_metrics,
            hostnames,
            headers_in_cache_by_authority,
            eviction_rounds
        ) {
            // Initialize the uncached scoring metrics according to
            // recovered_scoring_metrics
            let VersionedStorageScoringMetrics::V1(inner) = &metrics;
            self.initialize_faulty_blocks_metrics(
                *inner.faulty_blocks_provable(),
                *inner.faulty_blocks_unprovable(),
                hostname,
                authority_index,
                &context.metrics.node_metrics,
            );
            self.update_missing_blocks_and_equivocations(
                *inner.missing_proposals(),
                *inner.equivocations(),
                hostname,
                authority_index,
                StoreType::Uncached,
                &context.metrics.node_metrics,
            );

            // Initialize the cached scoring metrics according to headers_in_cache.
            let headers_rounds_in_cache = headers_in_cache
                .iter()
                .map(|block_ref| block_ref.round)
                .collect();
            let (cached_equivocations, missing_headers_in_cached_rounds) =
                calculate_scoring_metrics_for_range(
                    headers_rounds_in_cache,
                    eviction_round + 1,
                    threshold_clock_round - 1,
                );
            self.update_missing_blocks_and_equivocations(
                missing_headers_in_cached_rounds,
                cached_equivocations,
                hostname,
                authority_index,
                StoreType::Cached,
                &context.metrics.node_metrics,
            );
        }
    }

    // Auxiliary function to initialize scoring metrics relative to faulty blocks.
    // The `authority` parameter should be a valid index, otherwise the function
    // will panic. This check is not performed here, as it is assumed that the
    // caller has already checked it.
    pub(crate) fn initialize_faulty_blocks_metrics(
        &self,
        faulty_blocks_provable: u64,
        faulty_blocks_unprovable: u64,
        hostname: &str,
        authority_index: AuthorityIndex,
        node_metrics: &NodeMetrics,
    ) {
        node_metrics
            .faulty_blocks_provable_by_authority
            .with_label_values(&[hostname, "loaded from storage", "loaded from storage"])
            .inc_by(faulty_blocks_provable);
        node_metrics
            .faulty_blocks_unprovable_by_authority
            .with_label_values(&[hostname, "loaded from storage", "loaded from storage"])
            .inc_by(faulty_blocks_unprovable);
        self.uncached_metrics
            .store_faulty_blocks_provable(authority_index.value(), faulty_blocks_provable);
        self.uncached_metrics
            .store_faulty_blocks_unprovable(authority_index.value(), faulty_blocks_unprovable);
    }

    // Auxiliary function to update scoring metrics relative to missing blocks
    // and equivocations. The `authority` parameter should be a valid index,
    // otherwise the function will panic. This check is not performed here, as
    // it is assumed that the caller has already checked it.
    pub(crate) fn update_missing_blocks_and_equivocations(
        &self,
        missing_blocks: u64,
        equivocations: u64,
        hostname: &str,
        authority: AuthorityIndex,
        metric_type: StoreType,
        node_metrics: &NodeMetrics,
    ) {
        match metric_type {
            StoreType::Cached => {
                self.cached_metrics
                    .store_equivocations(authority.value(), equivocations);
                self.cached_metrics
                    .store_missing_proposals(authority.value(), missing_blocks);
                node_metrics
                    .equivocations_in_cache_by_authority
                    .with_label_values(&[hostname])
                    .set(equivocations as i64);
                node_metrics
                    .missing_proposals_in_cache_by_authority
                    .with_label_values(&[hostname])
                    .set(missing_blocks as i64);
            }

            StoreType::Uncached => {
                self.uncached_metrics
                    .increment_equivocations(authority.value(), equivocations);
                self.uncached_metrics
                    .increment_missing_proposals(authority.value(), missing_blocks);
                node_metrics
                    .uncached_equivocations_by_authority
                    .with_label_values(&[hostname])
                    .inc_by(equivocations);
                node_metrics
                    .uncached_missing_proposals_by_authority
                    .with_label_values(&[hostname])
                    .inc_by(missing_blocks);
            }
        }
    }
    // Updates the authority's scoring metrics according to the recent changes in
    // the DAG state, i.e., recent evictions and additions to cache. It also
    // updates the current local metrics count used by Scorer. It returns metrics
    // changes that should be updated in disk storage.
    pub(crate) fn update_scoring_metrics_on_eviction(
        &self,
        authority_index: AuthorityIndex,
        hostname: &str,
        recent_refs: &BTreeSet<BlockRef>,
        eviction_round: u32,
        last_eviction_round: u32,
        threshold_clock_round: u32,
        context: &Arc<Context>,
    ) -> Option<VersionedStorageScoringMetrics> {
        let committee_size = context.committee.size();
        let node_metrics = &context.metrics.node_metrics;
        // threshold_clock_round should be always at least 1. Analogously,
        // authority_index should be a valid index.
        if threshold_clock_round == 0 || authority_index.value() >= committee_size {
            return None;
        }

        // Get the blocks rounds that were not evicted.
        let cached_block_rounds = recent_refs
            .iter()
            .map(|block| block.round)
            .filter(|&round| round > eviction_round && round < threshold_clock_round)
            .collect::<Vec<u32>>();

        // Update metrics according to the blocks from rounds still in cache.
        let (cached_equivocations, missing_blocks_in_cached_rounds) =
            calculate_scoring_metrics_for_range(
                cached_block_rounds,
                eviction_round + 1,
                threshold_clock_round.saturating_sub(1),
            );

        self.update_missing_blocks_and_equivocations(
            missing_blocks_in_cached_rounds,
            cached_equivocations,
            hostname,
            authority_index,
            StoreType::Cached,
            node_metrics,
        );

        // If no eviction happened, we do not update the metrics on storage.
        if eviction_round == last_eviction_round {
            return None;
        }

        // Get the evicted blocks rounds.
        let evicted_block_rounds = recent_refs
            .iter()
            .map(|block| block.round)
            .filter(|&round| round <= eviction_round)
            .collect::<Vec<u32>>();

        // Update metrics according to the blocks from evicted rounds.
        let (evicted_equivocations, missing_blocks_in_evicted_rounds) =
            calculate_scoring_metrics_for_range(
                evicted_block_rounds,
                last_eviction_round + 1,
                eviction_round,
            );

        self.update_missing_blocks_and_equivocations(
            missing_blocks_in_evicted_rounds,
            evicted_equivocations,
            hostname,
            authority_index,
            StoreType::Uncached,
            node_metrics,
        );

        // Update current local metrics count.
        self.update_current_local_metrics_count(authority_index);

        Some(VersionedStorageScoringMetrics::new_from(
            &self.uncached_metrics,
            authority_index.value(),
        ))
    }

    // The `authority_index` should be a valid index, otherwise the function will
    // panic. This check is not performed here, as it is assumed that the caller has
    // already checked it.
    pub(crate) fn update_current_local_metrics_count(&self, authority_index: AuthorityIndex) {
        let uncached_metrics = &self
            .uncached_metrics
            .iterate_over_metrics()
            .map(|metric_vec| metric_vec[authority_index].load(Ordering::Relaxed))
            .collect::<Vec<u64>>();
        self.current_local_metrics_count
            .iterate_over_metrics()
            .zip(uncached_metrics)
            .for_each(|(local_metric_vec, uncached_metric)| {
                local_metric_vec[authority_index].store(*uncached_metric, Ordering::Relaxed);
            });
    }
}

// Given the set of blocks issued by an authority in rounds in the inclusive
// range [start, end], this function calculates and returns the number of
// equivocations and missing blocks in that range . The function should receive
// the vector with the rounds of such blocks and the range start and end points.
fn calculate_scoring_metrics_for_range(
    mut block_rounds: Vec<u32>,
    start: u32,
    end: u32,
) -> (u64, u64) {
    // Filter out rounds that are not in the range [start, end].
    block_rounds.retain(|&round| round >= start && round <= end);
    let number_of_blocks = block_rounds.len();
    block_rounds.dedup();
    let unique_block_rounds = block_rounds.len();
    // We use saturating_sub to avoid unexpected underflows, but the subtractions
    // below should never result in negative values by construction:
    // 1) unique_block_rounds <= number_of_blocks
    // 2) end - start + 1 >= unique_block_rounds
    let number_of_equivocations = number_of_blocks.saturating_sub(unique_block_rounds) as u64;
    let number_of_missing_blocks =
        (end + 1).saturating_sub(start + unique_block_rounds as u32) as u64;

    (number_of_equivocations, number_of_missing_blocks)
}

pub(crate) enum StoreType {
    Cached,
    Uncached,
}

#[cfg(test)]
impl ScoringMetricsStore {
    // Creates a dummy scoring metrics store for testing purposes (i.e., without any
    // connection to a Scorer)
    pub(crate) fn dummy_for_test(committee_size: usize, protocol_config: &ProtocolConfig) -> Self {
        let current_local_metrics_count = Arc::new(VersionedScoringMetrics::new(
            committee_size,
            protocol_config,
        ));
        ScoringMetricsStore::new(committee_size, current_local_metrics_count, protocol_config)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Arc, vec};

    use iota_common::scoring_metrics::VersionedStorageScoringMetrics;
    use starfish_config::{AuthorityIndex, Parameters};

    use crate::{
        context::Context,
        dag_state::{DagState, DataSource},
        scoring_metrics_store::ScoringMetricsStore,
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
    };

    impl ScoringMetricsStore {
        pub(crate) fn uncached_missing_proposals_by_authority(&self) -> Vec<u64> {
            self.uncached_metrics.load_missing_proposals()
        }

        pub(crate) fn equivocations_in_cache_by_authority(&self) -> Vec<u64> {
            self.cached_metrics.load_equivocations()
        }

        pub(crate) fn missing_proposals_in_cache_by_authority(&self) -> Vec<u64> {
            self.cached_metrics.load_missing_proposals()
        }

        pub(crate) fn uncached_equivocations_by_authority(&self) -> Vec<u64> {
            self.uncached_metrics.load_equivocations()
        }
    }

    fn get_uncached_missing_proposals(context: &Arc<Context>) -> Vec<u64> {
        let mut metrics = Vec::new();
        for authority in context.committee.authorities() {
            let hostname = authority.1.hostname.as_str();
            metrics.push(
                context
                    .metrics
                    .node_metrics
                    .uncached_missing_proposals_by_authority
                    .get_metric_with_label_values(&[hostname])
                    .unwrap()
                    .get(),
            )
        }
        metrics
    }

    fn get_missing_proposals_in_cache(context: &Arc<Context>) -> Vec<u64> {
        let mut metrics = Vec::new();
        for authority in context.committee.authorities() {
            let hostname = authority.1.hostname.as_str();
            metrics.push(
                context
                    .metrics
                    .node_metrics
                    .missing_proposals_in_cache_by_authority
                    .get_metric_with_label_values(&[hostname])
                    .unwrap()
                    .get()
                    .unsigned_abs(),
            )
        }
        metrics
    }

    fn get_uncached_equivocations(context: &Arc<Context>) -> Vec<u64> {
        let mut metrics = Vec::new();
        for authority in context.committee.authorities() {
            let hostname = authority.1.hostname.as_str();
            metrics.push(
                context
                    .metrics
                    .node_metrics
                    .uncached_equivocations_by_authority
                    .get_metric_with_label_values(&[hostname])
                    .unwrap()
                    .get(),
            )
        }
        metrics
    }

    fn get_equivocations_in_cache(context: &Arc<Context>) -> Vec<u64> {
        let mut metrics = Vec::new();
        for authority in context.committee.authorities() {
            let hostname = authority.1.hostname.as_str();
            metrics.push(
                context
                    .metrics
                    .node_metrics
                    .equivocations_in_cache_by_authority
                    .get_metric_with_label_values(&[hostname])
                    .unwrap()
                    .get()
                    .unsigned_abs(),
            )
        }
        metrics
    }

    #[tokio::test]
    async fn test_update_scoring_metrics_on_eviction_edge_cases() {
        let context = Arc::new(Context::new_for_test(4).0);
        let scoring_metrics_store = context.scoring_metrics_store.clone();
        let authority_index = AuthorityIndex::new_for_test(0);
        let hostname = "test_host";
        let recent_refs_by_authority = BTreeSet::new();
        // Test different unexpected combinations of eviction_round, last_evicted_round,
        // and threshold_clock_round. Since recent_refs_by_authority is empty, the
        // function should never panic or return more than zero equivocations.
        // Each of the cases below have a small explanation of why they are unexpected
        // and why they are supposed to return what they return.

        // Unexpected because: threshold_clock_round = last_evicted_round means that a
        // round with blocks from less than 2f+1 stake was evicted.
        // Return: None, because nothing is currently being evicted.
        let last_evicted_round = 5;
        let eviction_round = 5;
        let threshold_clock_round = 5;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert!(stored_metrics.is_none());

        // Unexpected because: threshold_clock_round = 0 means that genesis is missing.
        // Return: None, because nothing is currently being evicted.
        let last_evicted_round = 0;
        let eviction_round = 0;
        let threshold_clock_round = 0;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert!(stored_metrics.is_none());

        // Unexpected because: threshold_clock_round < eviction_round means that a round
        // with blocks from less than 2f+1 stake in being evicted.
        // Return: 3 missing proposals, from rounds 1 to 3(eviction_round).
        let last_evicted_round = 0;
        let eviction_round = 3;
        let threshold_clock_round = 2;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            stored_metrics,
            Some(VersionedStorageScoringMetrics::new_v1_for_test(
                0, // faulty_blocks_provable
                0, // faulty_blocks_unprovable
                3, // missing_proposals
                0, // equivocations
            ))
        );

        // Unexpected because: eviction_round < last_evicted_round means that blocks
        // below or in last_evicted_round were accepted.
        // Return: metrics won't be updated here, so it should return the same as in the
        // last step.
        let last_evicted_round = 1;
        let eviction_round = 0;
        let threshold_clock_round = 2;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            stored_metrics,
            Some(VersionedStorageScoringMetrics::new_v1_for_test(
                0, // faulty_blocks_provable
                0, // faulty_blocks_unprovable
                3, // missing_proposals
                0, // equivocations
            ))
        );

        // Unexpected because: threshold_clock_round < eviction_round <
        // last_evicted_round and threshold_clock_round. Return: metrics won't be
        // updated here, so it should return the same as in the last step.
        let last_evicted_round = 2;
        let eviction_round = 0;
        let threshold_clock_round = 1;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            stored_metrics,
            Some(VersionedStorageScoringMetrics::new_v1_for_test(
                0, // faulty_blocks_provable
                0, // faulty_blocks_unprovable
                3, // missing_proposals
                0, // equivocations
            ))
        );

        // Unexpected because: threshold_clock_round < last_evicted_round means that a
        // round with blocks from less than 2f+1 stake was evicted.
        // Return: None, because nothing is currently being evicted.
        let last_evicted_round = 1;
        let eviction_round = 2;
        let threshold_clock_round = 0;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert!(stored_metrics.is_none());

        let last_evicted_round = 2;
        let eviction_round = 1;
        let threshold_clock_round = 0;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert!(stored_metrics.is_none());

        // The function should not panic if the authority index is out of bounds.
        // Unexpected because: threshold_clock_round = last_evicted_round means that a
        // round with blocks from less than 2f+1 stake was evicted.
        // Return: None, because nothing is currently being evicted.
        let out_of_bounds_authority_index = AuthorityIndex::new_for_test(4);
        let last_evicted_round = 1;
        let eviction_round = 2;
        let threshold_clock_round = 3;
        let stored_metrics = scoring_metrics_store.update_scoring_metrics_on_eviction(
            out_of_bounds_authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert!(stored_metrics.is_none());
    }

    #[tokio::test]
    async fn test_metrics_flush_and_recovery() {
        //   telemetry_subscribers::init_for_testing();
        let committee_size = 4;
        let (context, _) = Context::new_for_test(committee_size);
        let context = context.with_parameters(Parameters {
            dag_state_cached_rounds: 5,
            ..Default::default()
        });
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        let hostnames: Vec<&str> = context
            .committee
            .authorities()
            .map(|a| a.1.hostname.as_str())
            .collect();
        let scoring_metrics = &context.scoring_metrics_store;
        let node_metrics = &context.metrics.node_metrics;

        // Initialize the DAG builder with 20 layers. Blocks in the DAG will reference
        // all blocks from the previous round.
        // - Rounds 1 to 5 will have unique blocks from all authorities.
        // - Rounds 6 to 8 will have unique blocks from all authorities, except 0, who
        //   will not propose any block.
        // - Rounds 9 to 10 will have unique blocks from all authorities.
        // - Rounds 11 to 20 will have unique blocks from all authorities, except:
        //      - Authority 1, who will produce 1 equivocating blocks at round 11 (i.e.,
        //        1+1 blocks)
        //      - Authority 2, who will produce 2 equivocating blocks at round 13 (i.e.,
        //        1+2 blocks)
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

        // Add the blocks and commits from first 10 rounds to the dag state. Since
        // authority 0 skipped a leader round, we use the 9 first items of the commits
        // vector
        let mut temp_commits = commits.split_off(9);
        dag_state.accept_block_headers(dag_builder.block_headers(1..=10), DataSource::Test);
        for commit in commits.clone() {
            dag_state.add_commit(commit);
        }

        assert_eq!(dag_state.last_commit_index(), 9);
        assert_eq!(dag_state.last_committed_rounds(), [9, 9, 10, 9]);
        assert_eq!(dag_state.evicted_rounds(), [0, 0, 0, 0]);

        // Checks that metrics are still all zeroed, since even though we accepted
        // blocks to the dag state, the metrics updates are done when the dag state is
        // flushed.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Flush the dag state
        dag_state.flush();

        // Check that metrics were updated correctly after flushing.
        //
        // Equivocations:
        // - We only accepted blocks from rounds <= 10, thus, no equivocations were
        //   accepted yet. Equivocations metrics, then, should be still all zeroed.
        //
        // Missing proposals:
        // - The last committed round is 10, so the eviction round should be 5 for
        //   authority 2 (leader of round 10) and 4 for all other authorities.
        // - The threshold_clock_round should be 11, since we already accepted all
        //   blocks from epoch 10.
        // - Then, finally, we should have counted:
        //      - 0 uncached missing proposals for authority 0;
        //      - 3 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        assert_eq!(dag_state.evicted_rounds(), [4, 4, 5, 4]);
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
            ]
        );

        // Clear and check all metrics
        scoring_metrics.uncached_metrics.reset();
        scoring_metrics.cached_metrics.reset();
        node_metrics
            .uncached_missing_proposals_by_authority
            .with_label_values(&[hostnames[0]])
            .reset();
        node_metrics
            .missing_proposals_in_cache_by_authority
            .with_label_values(&[hostnames[0]])
            .set(0);
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        assert_eq!(dag_state.last_commit_index(), 9);
        assert_eq!(dag_state.last_committed_rounds(), [9, 9, 10, 9]);

        // Metrics should have been initialized as before the recovery.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
            ]
        );

        // Add blocks and commits from rounds 11 and 12 to the dag state.
        let second_temp_commits = temp_commits.split_off(2);
        dag_state.accept_block_headers(dag_builder.block_headers(11..=12), DataSource::Test);
        for commit in temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Flush the dag state
        dag_state.flush();

        assert_eq!(dag_state.last_commit_index(), 11);
        assert_eq!(dag_state.last_committed_rounds(), [12, 11, 11, 11]);
        assert_eq!(dag_state.evicted_rounds(), [7, 6, 6, 6]);

        // Check that metrics were updated correctly after flushing.
        //
        // Missing proposals:
        // - The last commit round is 12, so the eviction round should be 7 for
        //   authority 0 (leader of round 12) and 6 for all other authorities. Then, we
        //   should have counted:
        //      - 2 uncached missing proposals for authority 0;
        //      - 1 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        //
        // Equivocations:
        // - We only removed from cache blocks from rounds <= 7, thus, no equivocations
        //   should be uncached. Then, we should have counted:
        //      - 0 uncached equivocations;
        //      - 1 equivocation in cache for authority 1;
        //      - 0 equivocations in cache for authorities 0, 2 and 3;

        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
            ]
        );

        // Accept all the rest of blocks and commits.
        dag_state.accept_block_headers(dag_builder.block_headers(13..=20), DataSource::Test);
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Clear and check all metrics
        scoring_metrics.uncached_metrics.reset();
        scoring_metrics.cached_metrics.reset();
        node_metrics
            .uncached_missing_proposals_by_authority
            .with_label_values(&[hostnames[0]])
            .reset();
        node_metrics
            .missing_proposals_in_cache_by_authority
            .with_label_values(&[hostnames[0]])
            .set(0);
        node_metrics
            .equivocations_in_cache_by_authority
            .with_label_values(&[hostnames[1]])
            .set(0);

        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        assert_eq!(dag_state.last_commit_index(), 11);
        assert_eq!(dag_state.last_committed_rounds(), [12, 11, 11, 11]);

        // Since the last accepted blocks were not flushed, the equivocations from
        // rounds 13 to 20 should not be accounted for. The metrics should remain the
        // same as before this acceptance.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
            ]
        );

        // Now we accept those lost blocks again and flush the dag state
        dag_state.accept_block_headers(dag_builder.block_headers(13..=20), DataSource::Test);
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        assert_eq!(dag_state.last_commit_index(), 19);
        assert_eq!(dag_state.last_committed_rounds(), [20, 19, 19, 19]);
        assert_eq!(dag_state.evicted_rounds(), [15, 14, 14, 14]);

        // Now all misbehaviors should be accounted for in the uncached metrics.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0, 1, 2, 0],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0, 1, 2, 0],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
            ]
        );
    }
}
