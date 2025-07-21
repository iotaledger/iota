// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use consensus_config::AuthorityIndex;
use prometheus::{
    Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Registry,
    exponential_buckets, register_histogram_vec_with_registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry, register_int_counter_with_registry,
    register_int_gauge_vec_with_registry, register_int_gauge_with_registry,
};

use crate::{BlockRef, error::ConsensusError, network::metrics::NetworkMetrics};

// starts from 1μs, 50μs, 100μs...
const FINE_GRAINED_LATENCY_SEC_BUCKETS: &[f64] = &[
    0.000_001, 0.000_050, 0.000_100, 0.000_500, 0.001, 0.005, 0.01, 0.05, 0.1, 0.15, 0.2, 0.25,
    0.3, 0.35, 0.4, 0.45, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.5, 3.0, 3.5,
    4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5, 10., 20., 30., 60., 120.,
];

const NUM_BUCKETS: &[f64] = &[
    1.0,
    2.0,
    4.0,
    8.0,
    10.0,
    20.0,
    40.0,
    80.0,
    100.0,
    150.0,
    200.0,
    400.0,
    800.0,
    1000.0,
    2000.0,
    3000.0,
    5000.0,
    10000.0,
    20000.0,
    30000.0,
    50000.0,
    100_000.0,
    200_000.0,
    300_000.0,
    500_000.0,
    1_000_000.0,
];

const LATENCY_SEC_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.6, 0.7, 0.8, 0.9,
    1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 8.5,
    9.0, 9.5, 10., 12.5, 15., 17.5, 20., 25., 30., 60., 90., 120., 180., 300.,
];

const SIZE_BUCKETS: &[f64] = &[
    100.,
    400.,
    800.,
    1_000.,
    2_000.,
    5_000.,
    10_000.,
    20_000.,
    50_000.,
    100_000.,
    200_000.0,
    300_000.0,
    400_000.0,
    500_000.0,
    1_000_000.0,
    2_000_000.0,
    3_000_000.0,
    5_000_000.0,
    10_000_000.0,
]; // size in bytes

pub(crate) struct Metrics {
    pub(crate) node_metrics: NodeMetrics,
    pub(crate) network_metrics: NetworkMetrics,
    pub(crate) scoring_metrics: ValidatorScoringMetrics,
}

enum MetricType {
    Cached,
    Uncached,
}

impl Metrics {
    pub(crate) fn update_scoring_metrics_on_eviction(
        &self,
        validator: AuthorityIndex,
        hostname: &str,
        eviction_round: u32,
        last_evicted_round: u32,
        recent_refs_by_authority: &BTreeSet<BlockRef>,
        threshold_clock_round: u32,
    ) -> Option<StoredScoringMetricsU64> {
        // Get the blocks rounds that were not evicted.
        let cached_block_rounds = recent_refs_by_authority
            .iter()
            .map(|block| block.round)
            .filter(|&round| round > eviction_round && round < threshold_clock_round)
            .collect::<Vec<u32>>();

        // Update metrics according to the blocks from rounds still in cache.
        let (cached_equivocations, missing_blocks_in_cached_rounds) =
            calculate_scoring_metrics_for_range(
                cached_block_rounds,
                eviction_round + 1,
                threshold_clock_round - 1,
            );
        self.update_missing_blocks_and_equivocations(
            missing_blocks_in_cached_rounds,
            cached_equivocations,
            hostname,
            validator,
            MetricType::Cached,
        );

        // If no eviction happened, we do not update the metrics on storage.
        if eviction_round == last_evicted_round {
            return None;
        }

        // Get the evicted blocks rounds.
        let evicted_block_rounds = recent_refs_by_authority
            .iter()
            .map(|block| block.round)
            .filter(|&round| round <= eviction_round)
            .collect::<Vec<u32>>();

        // Update metrics according to the blocks from evicted rounds.
        let (evicted_equivocations, missing_blocks_in_evicted_rounds) =
            calculate_scoring_metrics_for_range(
                evicted_block_rounds,
                last_evicted_round + 1,
                eviction_round,
            );
        self.update_missing_blocks_and_equivocations(
            missing_blocks_in_evicted_rounds,
            evicted_equivocations,
            hostname,
            validator,
            MetricType::Uncached,
        );

        // Update score
        self.update_authority_score(validator);

        Some(StoredScoringMetricsU64 {
            faulty_blocks_provable_by_authority: self.scoring_metrics.uncached[validator]
                .faulty_blocks_provable_by_authority
                .load(Ordering::Relaxed),
            faulty_blocks_unprovable_by_authority: self.scoring_metrics.uncached[validator]
                .faulty_blocks_unprovable_by_authority
                .load(Ordering::Relaxed),
            equivocations_by_authority: self.scoring_metrics.uncached[validator]
                .equivocations_by_authority
                .load(Ordering::Relaxed),
            missing_proposals_by_authority: self.scoring_metrics.uncached[validator]
                .missing_proposals_by_authority
                .load(Ordering::Relaxed),
        })
    }

    pub(crate) fn initialize_uncached_scoring_metrics(
        &self,
        metrics: Vec<(AuthorityIndex, StoredScoringMetricsU64)>,
        hostnames: &Vec<&str>,
    ) {
        for ((authority, metrics), &hostname) in metrics.into_iter().zip(hostnames.iter()) {
            let StoredScoringMetricsU64 {
                faulty_blocks_provable_by_authority,
                faulty_blocks_unprovable_by_authority,
                equivocations_by_authority,
                missing_proposals_by_authority,
            } = metrics;

            self.update_missing_blocks_and_equivocations(
                missing_proposals_by_authority,
                equivocations_by_authority,
                hostname,
                authority,
                MetricType::Uncached,
            );

            self.node_metrics
                .faulty_blocks_provable_by_authority
                .with_label_values(&[hostname, "loaded from storage", "loaded from storage"])
                .inc_by(faulty_blocks_provable_by_authority);
            self.node_metrics
                .faulty_blocks_unprovable_by_authority
                .with_label_values(&[hostname, "loaded from storage", "loaded from storage"])
                .inc_by(faulty_blocks_unprovable_by_authority);
            self.scoring_metrics.uncached[authority]
                .faulty_blocks_provable_by_authority
                .store(faulty_blocks_provable_by_authority, Ordering::Relaxed);
            self.scoring_metrics.uncached[authority]
                .faulty_blocks_unprovable_by_authority
                .store(faulty_blocks_unprovable_by_authority, Ordering::Relaxed);

            // Update score
            self.update_authority_score(authority);
        }
    }

    pub(crate) fn initialize_cached_scoring_metrics(
        &self,
        validator: AuthorityIndex,
        hostname: &str,
        block_rounds_in_cache: Vec<u32>,
        threshold_clock_round: u32,
        eviction_round: u32,
    ) {
        // Update metrics according to the blocks that are loaded to cache.
        let (cached_equivocations, missing_blocks_in_cached_rounds) =
            calculate_scoring_metrics_for_range(
                block_rounds_in_cache,
                eviction_round + 1,
                threshold_clock_round - 1,
            );
        self.update_missing_blocks_and_equivocations(
            missing_blocks_in_cached_rounds,
            cached_equivocations,
            hostname,
            validator,
            MetricType::Cached,
        );
    }

    fn update_missing_blocks_and_equivocations(
        &self,
        missing_blocks: u64,
        equivocations: u64,
        hostname: &str,
        authority: AuthorityIndex,
        metric_type: MetricType,
    ) {
        match metric_type {
            MetricType::Cached => {
                self.scoring_metrics.cached[authority]
                    .equivocations_by_authority
                    .store(equivocations, Ordering::Relaxed);
                self.scoring_metrics.cached[authority]
                    .missing_proposals_by_authority
                    .store(missing_blocks, Ordering::Relaxed);
                self.node_metrics
                    .equivocations_in_cache_by_authority
                    .with_label_values(&[hostname])
                    .set(equivocations as i64);
                self.node_metrics
                    .missing_proposals_in_cache_by_authority
                    .with_label_values(&[hostname])
                    .set(missing_blocks as i64);
            }

            MetricType::Uncached => {
                self.scoring_metrics.uncached[authority]
                    .equivocations_by_authority
                    .fetch_add(equivocations, Ordering::Relaxed);
                self.scoring_metrics.uncached[authority]
                    .missing_proposals_by_authority
                    .fetch_add(missing_blocks, Ordering::Relaxed);
                self.node_metrics
                    .uncached_equivocations_by_authority
                    .with_label_values(&[hostname])
                    .inc_by(equivocations);
                self.node_metrics
                    .uncached_missing_proposals_by_authority
                    .with_label_values(&[hostname])
                    .inc_by(missing_blocks);
            }
        }
    }

    fn update_authority_score(&self, authority: AuthorityIndex) {
        let StoredScoringMetricsU64 {
            faulty_blocks_provable_by_authority,
            faulty_blocks_unprovable_by_authority,
            equivocations_by_authority,
            missing_proposals_by_authority,
        } = StoredScoringMetricsU64 {
            faulty_blocks_provable_by_authority: self.scoring_metrics.uncached[authority]
                .faulty_blocks_provable_by_authority
                .load(Ordering::Relaxed),
            faulty_blocks_unprovable_by_authority: self.scoring_metrics.uncached[authority]
                .faulty_blocks_unprovable_by_authority
                .load(Ordering::Relaxed),
            equivocations_by_authority: self.scoring_metrics.uncached[authority]
                .equivocations_by_authority
                .load(Ordering::Relaxed),
            missing_proposals_by_authority: self.scoring_metrics.uncached[authority]
                .missing_proposals_by_authority
                .load(Ordering::Relaxed),
        };

        let misbehaviour_count = missing_proposals_by_authority
            + equivocations_by_authority
            + faulty_blocks_provable_by_authority
            + faulty_blocks_unprovable_by_authority;
        self.scoring_metrics.score[authority].store(u32::MAX as u64 / (1 + misbehaviour_count));
    }

    pub(crate) fn update_scoring_metrics_on_block_receival(
        &self,
        index: AuthorityIndex,
        hostname: &str,
        error: ConsensusError,
        source: &str,
    ) {
        if should_update_provable_metrics(&error, source) {
            self.scoring_metrics.uncached[index]
                .faulty_blocks_provable_by_authority
                .fetch_add(1, Ordering::Relaxed);
            self.node_metrics
                .faulty_blocks_provable_by_authority
                .with_label_values(&[hostname, source, error.name()])
                .inc();
        } else if should_update_unprovable_metrics(&error, source) {
            self.scoring_metrics.uncached[index]
                .faulty_blocks_unprovable_by_authority
                .fetch_add(1, Ordering::Relaxed);
            self.node_metrics
                .faulty_blocks_unprovable_by_authority
                .with_label_values(&[hostname, source, error.name()])
                .inc();
        } else {
            return;
        }
    }
}

pub(crate) struct Score(AtomicU64);
impl Score {
    pub(crate) fn new() -> Self {
        Self(AtomicU64::new(u64::MAX))
    }

    pub(crate) fn store(&self, value: u64) {
        self.0.store(value, Ordering::Relaxed);
    }
}

pub(crate) struct NodeMetrics {
    pub(crate) block_commit_latency: Histogram,
    pub(crate) proposed_blocks: IntCounterVec,
    pub(crate) proposed_block_size: Histogram,
    pub(crate) proposed_block_transactions: Histogram,
    pub(crate) proposed_block_ancestors: Histogram,
    pub(crate) proposed_block_ancestors_depth: HistogramVec,
    pub(crate) highest_verified_authority_round: IntGaugeVec,
    pub(crate) lowest_verified_authority_round: IntGaugeVec,
    pub(crate) block_proposal_interval: Histogram,
    pub(crate) block_proposal_leader_wait_ms: IntCounterVec,
    pub(crate) block_proposal_leader_wait_count: IntCounterVec,
    pub(crate) block_timestamp_drift_wait_ms: IntCounterVec,
    pub(crate) blocks_per_commit_count: Histogram,
    pub(crate) blocks_pruned_on_commit: IntCounterVec,
    pub(crate) broadcaster_rtt_estimate_ms: IntGaugeVec,
    pub(crate) core_add_blocks_batch_size: Histogram,
    pub(crate) core_check_block_refs_batch_size: Histogram,
    pub(crate) core_lock_dequeued: IntCounter,
    pub(crate) core_lock_enqueued: IntCounter,
    pub(crate) core_skipped_proposals: IntCounterVec,
    pub(crate) highest_accepted_authority_round: IntGaugeVec,
    pub(crate) highest_accepted_round: IntGauge,
    pub(crate) accepted_blocks: IntCounterVec,
    pub(crate) dag_state_recent_blocks: IntGauge,
    pub(crate) dag_state_recent_refs: IntGauge,
    pub(crate) dag_state_store_read_count: IntCounterVec,
    pub(crate) dag_state_store_write_count: IntCounter,
    pub(crate) fetch_blocks_scheduler_inflight: IntGauge,
    pub(crate) fetch_blocks_scheduler_skipped: IntCounterVec,
    pub(crate) synchronizer_fetched_blocks_by_peer: IntCounterVec,
    pub(crate) synchronizer_requested_blocks_by_peer: IntCounterVec,
    pub(crate) synchronizer_missing_blocks_by_authority: IntCounterVec,
    pub(crate) synchronizer_current_missing_blocks_by_authority: IntGaugeVec,
    pub(crate) synchronizer_fetched_blocks_by_authority: IntCounterVec,
    pub(crate) synchronizer_requested_blocks_by_authority: IntCounterVec,
    pub(crate) synchronizer_fetch_failures_by_peer: IntCounterVec,
    pub(crate) synchronizer_process_fetched_failures_by_peer: IntCounterVec,
    pub(crate) network_received_excluded_ancestors_from_authority: IntCounterVec,
    pub(crate) network_excluded_ancestors_sent_to_fetch: IntCounterVec,
    pub(crate) network_excluded_ancestors_count_by_authority: IntCounterVec,
    pub(crate) invalid_blocks: IntCounterVec,
    pub(crate) faulty_blocks_provable_by_authority: IntCounterVec,
    pub(crate) faulty_blocks_unprovable_by_authority: IntCounterVec,
    pub(crate) uncached_equivocations_by_authority: IntCounterVec,
    pub(crate) uncached_missing_proposals_by_authority: IntCounterVec,
    pub(crate) equivocations_in_cache_by_authority: IntGaugeVec,
    pub(crate) missing_proposals_in_cache_by_authority: IntGaugeVec,
    pub(crate) rejected_blocks: IntCounterVec,
    pub(crate) rejected_future_blocks: IntCounterVec,
    pub(crate) subscribed_blocks: IntCounterVec,
    pub(crate) verified_blocks: IntCounterVec,
    pub(crate) committed_leaders_total: IntCounterVec,
    pub(crate) last_committed_authority_round: IntGaugeVec,
    pub(crate) last_committed_leader_round: IntGauge,
    pub(crate) last_commit_index: IntGauge,
    pub(crate) last_known_own_block_round: IntGauge,
    pub(crate) sync_last_known_own_block_retries: IntCounter,
    pub(crate) commit_round_advancement_interval: Histogram,
    pub(crate) last_decided_leader_round: IntGauge,
    pub(crate) leader_timeout_total: IntCounterVec,
    pub(crate) smart_selection_wait: IntCounter,
    pub(crate) ancestor_state_change_by_authority: IntCounterVec,
    pub(crate) excluded_proposal_ancestors_count_by_authority: IntCounterVec,
    pub(crate) included_excluded_proposal_ancestors_count_by_authority: IntCounterVec,
    pub(crate) missing_blocks_total: IntCounter,
    pub(crate) missing_blocks_after_fetch_total: IntCounter,
    pub(crate) num_of_bad_nodes: IntGauge,
    pub(crate) quorum_receive_latency: Histogram,
    pub(crate) reputation_scores: IntGaugeVec,
    pub(crate) scope_processing_time: HistogramVec,
    pub(crate) sub_dags_per_commit_count: Histogram,
    pub(crate) block_suspensions: IntCounterVec,
    pub(crate) block_unsuspensions: IntCounterVec,
    pub(crate) suspended_block_time: HistogramVec,
    pub(crate) block_manager_suspended_blocks: IntGauge,
    pub(crate) block_manager_missing_ancestors: IntGauge,
    pub(crate) block_manager_missing_blocks: IntGauge,
    pub(crate) block_manager_missing_blocks_by_authority: IntCounterVec,
    pub(crate) block_manager_missing_ancestors_by_authority: IntCounterVec,
    pub(crate) block_manager_gced_blocks: IntCounterVec,
    pub(crate) block_manager_gc_unsuspended_blocks: IntCounterVec,
    pub(crate) block_manager_skipped_blocks: IntCounterVec,
    pub(crate) threshold_clock_round: IntGauge,
    pub(crate) subscriber_connection_attempts: IntCounterVec,
    pub(crate) subscribed_to: IntGaugeVec,
    pub(crate) subscribed_by: IntGaugeVec,
    pub(crate) commit_sync_inflight_fetches: IntGauge,
    pub(crate) commit_sync_pending_fetches: IntGauge,
    pub(crate) commit_sync_fetch_commits_handler_uncertified_skipped: IntCounter,
    pub(crate) commit_sync_fetched_commits: IntCounterVec,
    pub(crate) commit_sync_fetched_blocks: IntCounterVec,
    pub(crate) commit_sync_total_fetched_blocks_size: IntCounterVec,
    pub(crate) commit_sync_quorum_index: IntGauge,
    pub(crate) commit_sync_highest_synced_index: IntGauge,
    pub(crate) commit_sync_highest_fetched_index: IntGauge,
    pub(crate) commit_sync_local_index: IntGauge,
    pub(crate) commit_sync_gap_on_processing: IntCounter,
    pub(crate) commit_sync_fetch_loop_latency: Histogram,
    pub(crate) commit_sync_fetch_once_latency: HistogramVec,
    pub(crate) commit_sync_fetch_once_errors: IntCounterVec,
    pub(crate) commit_sync_fetch_missing_blocks: IntCounterVec,
    pub(crate) round_prober_received_quorum_round_gaps: IntGaugeVec,
    pub(crate) round_prober_accepted_quorum_round_gaps: IntGaugeVec,
    pub(crate) round_prober_low_received_quorum_round: IntGaugeVec,
    pub(crate) round_prober_low_accepted_quorum_round: IntGaugeVec,
    pub(crate) round_prober_current_received_round_gaps: IntGaugeVec,
    pub(crate) round_prober_current_accepted_round_gaps: IntGaugeVec,
    pub(crate) round_prober_propagation_delays: Histogram,
    pub(crate) round_prober_last_propagation_delay: IntGauge,
    pub(crate) round_prober_request_errors: IntCounterVec,
    pub(crate) uptime: Histogram,
}

impl NodeMetrics {
    pub(crate) fn new(registry: &Registry) -> Self {
        Self {
            block_commit_latency: register_histogram_with_registry!(
                "block_commit_latency",
                "The time taken between block creation and block commit.",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            proposed_blocks: register_int_counter_vec_with_registry!(
                "proposed_blocks",
                "Total number of proposed blocks. If force is true then this block has been created forcefully via a leader timeout event.",
                &["force"],
                registry,
            ).unwrap(),
            proposed_block_size: register_histogram_with_registry!(
                "proposed_block_size",
                "The size (in bytes) of proposed blocks",
                SIZE_BUCKETS.to_vec(),
                registry
            ).unwrap(),
            proposed_block_transactions: register_histogram_with_registry!(
                "proposed_block_transactions",
                "# of transactions contained in proposed blocks",
                NUM_BUCKETS.to_vec(),
                registry
            ).unwrap(),
            proposed_block_ancestors: register_histogram_with_registry!(
                "proposed_block_ancestors",
                "Number of ancestors in proposed blocks",
                exponential_buckets(1.0, 1.4, 20).unwrap(),
                registry,
            ).unwrap(),
            proposed_block_ancestors_depth: register_histogram_vec_with_registry!(
                "proposed_block_ancestors_depth",
                "The depth in rounds of ancestors included in newly proposed blocks",
                &["authority"],
                exponential_buckets(1.0, 2.0, 14).unwrap(),
                registry,
            ).unwrap(),
            highest_verified_authority_round: register_int_gauge_vec_with_registry!(
                "highest_verified_authority_round",
                "The highest round of verified block for the corresponding authority",
                &["authority"],
                registry,
            ).unwrap(),
            lowest_verified_authority_round: register_int_gauge_vec_with_registry!(
                "lowest_verified_authority_round",
                "The lowest round of verified block for the corresponding authority",
                &["authority"],
                registry,
            ).unwrap(),
            block_proposal_interval: register_histogram_with_registry!(
                "block_proposal_interval",
                "Intervals (in secs) between block proposals.",
                FINE_GRAINED_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            block_proposal_leader_wait_ms: register_int_counter_vec_with_registry!(
                "block_proposal_leader_wait_ms",
                "Total time in ms spent waiting for a leader when proposing blocks.",
                &["authority"],
                registry,
            ).unwrap(),
            block_proposal_leader_wait_count: register_int_counter_vec_with_registry!(
                "block_proposal_leader_wait_count",
                "Total times waiting for a leader when proposing blocks.",
                &["authority"],
                registry,
            ).unwrap(),
            block_timestamp_drift_wait_ms: register_int_counter_vec_with_registry!(
                "block_timestamp_drift_wait_ms",
                "Total time in ms spent waiting, when a received block has timestamp in future.",
                &["authority", "source"],
                registry,
            ).unwrap(),
            blocks_per_commit_count: register_histogram_with_registry!(
                "blocks_per_commit_count",
                "The number of blocks per commit.",
                NUM_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            blocks_pruned_on_commit: register_int_counter_vec_with_registry!(
                "blocks_pruned_on_commit",
                "Number of blocks that got pruned due to garbage collection during a commit. This is not an accurate metric and measures the pruned blocks on the edge of the commit.",
                &["authority", "commit_status"],
                registry,
            ).unwrap(),
            broadcaster_rtt_estimate_ms: register_int_gauge_vec_with_registry!(
                "broadcaster_rtt_estimate_ms",
                "Estimated RTT latency per peer authority, for block sending in Broadcaster",
                &["peer"],
                registry,
            ).unwrap(),
            core_add_blocks_batch_size: register_histogram_with_registry!(
                "core_add_blocks_batch_size",
                "The number of blocks received from Core for processing on a single batch",
                NUM_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            core_check_block_refs_batch_size: register_histogram_with_registry!(
                "core_check_block_refs_batch_size",
                "The number of excluded blocks received from Core for search on a single batch",
                NUM_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            core_lock_dequeued: register_int_counter_with_registry!(
                "core_lock_dequeued",
                "Number of dequeued core requests",
                registry,
            ).unwrap(),
            core_lock_enqueued: register_int_counter_with_registry!(
                "core_lock_enqueued",
                "Number of enqueued core requests",
                registry,
            ).unwrap(),
            core_skipped_proposals: register_int_counter_vec_with_registry!(
                "core_skipped_proposals",
                "Number of proposals skipped in the Core, per reason",
                &["reason"],
                registry,
            ).unwrap(),
            highest_accepted_authority_round: register_int_gauge_vec_with_registry!(
                "highest_accepted_authority_round",
                "The highest round where a block has been accepted per authority. Resets on restart.",
                &["authority"],
                registry,
            ).unwrap(),
            highest_accepted_round: register_int_gauge_with_registry!(
                "highest_accepted_round",
                "The highest round where a block has been accepted. Resets on restart.",
                registry,
            ).unwrap(),
            accepted_blocks: register_int_counter_vec_with_registry!(
                "accepted_blocks",
                "Number of accepted blocks by source (own, others)",
                &["source"],
                registry,
            ).unwrap(),
            dag_state_recent_blocks: register_int_gauge_with_registry!(
                "dag_state_recent_blocks",
                "Number of recent blocks cached in the DagState",
                registry,
            ).unwrap(),
            dag_state_recent_refs: register_int_gauge_with_registry!(
                "dag_state_recent_refs",
                "Number of recent refs cached in the DagState",
                registry,
            ).unwrap(),
            dag_state_store_read_count: register_int_counter_vec_with_registry!(
                "dag_state_store_read_count",
                "Number of times DagState needs to read from store per operation type",
                &["type"],
                registry,
            ).unwrap(),
            dag_state_store_write_count: register_int_counter_with_registry!(
                "dag_state_store_write_count",
                "Number of times DagState needs to write to store",
                registry,
            ).unwrap(),
            fetch_blocks_scheduler_inflight: register_int_gauge_with_registry!(
                "fetch_blocks_scheduler_inflight",
                "Designates whether the synchronizer scheduler task to fetch blocks is currently running",
                registry,
            ).unwrap(),
            fetch_blocks_scheduler_skipped: register_int_counter_vec_with_registry!(
                "fetch_blocks_scheduler_skipped",
                "Number of times the scheduler skipped fetching blocks",
                &["reason"],
                registry
            ).unwrap(),
            synchronizer_fetched_blocks_by_peer: register_int_counter_vec_with_registry!(
                "synchronizer_fetched_blocks_by_peer",
                "Number of fetched blocks per peer authority via the synchronizer and also by block authority",
                &["peer", "type"],
                registry,
            ).unwrap(),
            synchronizer_requested_blocks_by_peer: register_int_counter_vec_with_registry!(
                "synchronizer_requested_blocks_by_peer",
                "Number of requested blocks per peer authority via the synchronizer and also by block authority",
                &["peer", "type"],
                registry,
            ).unwrap(),
            synchronizer_missing_blocks_by_authority: register_int_counter_vec_with_registry!(
                "synchronizer_missing_blocks_by_authority",
                "Number of missing blocks per block author, as observed by the synchronizer during periodic sync.",
                &["authority"],
                registry,
            ).unwrap(),
            synchronizer_fetch_failures_by_peer: register_int_counter_vec_with_registry!(
                "synchronizer_fetch_failures",
                "Number of fetch failures against each peer",
                &["peer", "type"],
                registry,
            ).unwrap(),
            synchronizer_process_fetched_failures_by_peer: register_int_counter_vec_with_registry!(
                "synchronizer_process_fetched_failures",
                "Number of failures for processing fetched blocks against each peer",
                &["peer", "type"],
                registry,
            ).unwrap(),
            synchronizer_current_missing_blocks_by_authority: register_int_gauge_vec_with_registry!(
                "synchronizer_current_missing_blocks_by_authority",
                "Current number of missing blocks per block author, as observed by the synchronizer during periodic sync.",
                &["authority"],
                registry,
            ).unwrap(),
            synchronizer_fetched_blocks_by_authority: register_int_counter_vec_with_registry!(
                "synchronizer_fetched_blocks_by_authority",
                "Number of fetched blocks per block author via the synchronizer",
                &["authority", "type"],
                registry,
            ).unwrap(),
            synchronizer_requested_blocks_by_authority: register_int_counter_vec_with_registry!(
                "synchronizer_requested_blocks_by_authority",
                "Number of requested blocks per block author via the synchronizer",
                &["authority", "type"],
                registry,
            ).unwrap(),
            network_received_excluded_ancestors_from_authority: register_int_counter_vec_with_registry!(
                "network_received_excluded_ancestors_from_authority",
                "Number of excluded ancestors received from each authority.",
                &["authority"],
                registry,
            ).unwrap(),
            network_excluded_ancestors_count_by_authority: register_int_counter_vec_with_registry!(
                "network_excluded_ancestors_count_by_authority",
                "Total number of excluded ancestors per authority.",
                &["authority"],
                registry,
            ).unwrap(),
            network_excluded_ancestors_sent_to_fetch: register_int_counter_vec_with_registry!(
                "network_excluded_ancestors_sent_to_fetch",
                "Number of excluded ancestors sent to fetch.",
                &["authority"],
                registry,
            ).unwrap(),
            last_known_own_block_round: register_int_gauge_with_registry!(
                "last_known_own_block_round",
                "The highest round of our own block as this has been synced from peers during an amnesia recovery",
                registry,
            ).unwrap(),
            sync_last_known_own_block_retries: register_int_counter_with_registry!(
                "sync_last_known_own_block_retries",
                "Number of times this node tried to fetch the last own block from peers",
                registry,
            ).unwrap(),
            // TODO: add a short status label.
            invalid_blocks: register_int_counter_vec_with_registry!(
                "invalid_blocks",
                "Number of invalid blocks per peer authority",
                &["authority", "source", "error"],
                registry,
            ).unwrap(),
            faulty_blocks_provable_by_authority: register_int_counter_vec_with_registry!(
                "faulty_blocks_provable_by_authority",
                "Number of semantically invalid blocks per peer authority",
                &["authority", "source", "error"],
                registry,
             ).unwrap(),
            faulty_blocks_unprovable_by_authority: register_int_counter_vec_with_registry!(
                "faulty_blocks_unprovable_by_authority",
                "Number of syntactically invalid blocks per peer authority",
                &["authority", "source", "error"],
                registry,
            ).unwrap(),
            uncached_equivocations_by_authority: register_int_counter_vec_with_registry!(
                "uncached_equivocations_by_authority",
                "Registers the number of equivocations per authority that were already evicted from cache.",
                &["authority"],
                registry,
            ).unwrap(),
            uncached_missing_proposals_by_authority: register_int_counter_vec_with_registry!(
                "uncached_missing_proposals_by_authority",
                "Registers the number of blocks that should be already evicted from cache but authority failed to send.",
                &["authority"],
                registry,
            ).unwrap(),
            equivocations_in_cache_by_authority: register_int_gauge_vec_with_registry!(
                "equivocations_in_cache_by_authority",
                "Registers the number of equivocations per authority stored on cache.",
                &["authority"],
                registry,
            ).unwrap(),
            missing_proposals_in_cache_by_authority: register_int_gauge_vec_with_registry!(
                "missing_proposals_in_cache_by_authority",
                "Registers the number of blocks on the cache that an authority failed to send.",
                &["authority"],
                registry,
            ).unwrap(),
            rejected_blocks: register_int_counter_vec_with_registry!(
                "rejected_blocks",
                "Number of blocks rejected before verifications",
                &["reason"],
                registry,
            ).unwrap(),
            rejected_future_blocks: register_int_counter_vec_with_registry!(
                "rejected_future_blocks",
                "Number of blocks rejected because their timestamp is too far in the future",
                &["authority"],
                registry,
            ).unwrap(),
            subscribed_blocks: register_int_counter_vec_with_registry!(
                "subscribed_blocks",
                "Number of blocks received from each peer before verification",
                &["authority"],
                registry,
            ).unwrap(),
            verified_blocks: register_int_counter_vec_with_registry!(
                "verified_blocks",
                "Number of blocks received from each peer that are verified",
                &["authority"],
                registry,
            ).unwrap(),
            committed_leaders_total: register_int_counter_vec_with_registry!(
                "committed_leaders_total",
                "Total number of (direct or indirect) committed leaders per authority",
                &["authority", "commit_type"],
                registry,
            ).unwrap(),
            last_committed_authority_round: register_int_gauge_vec_with_registry!(
                "last_committed_authority_round",
                "The last round committed by authority.",
                &["authority"],
                registry,
            ).unwrap(),
            last_committed_leader_round: register_int_gauge_with_registry!(
                "last_committed_leader_round",
                "The last round where a leader was committed to store and sent to commit consumer.",
                registry,
            ).unwrap(),
            last_commit_index: register_int_gauge_with_registry!(
                "last_commit_index",
                "Index of the last commit.",
                registry,
            ).unwrap(),
            commit_round_advancement_interval: register_histogram_with_registry!(
                "commit_round_advancement_interval",
                "Intervals (in secs) between commit round advancements.",
                FINE_GRAINED_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            last_decided_leader_round: register_int_gauge_with_registry!(
                "last_decided_leader_round",
                "The last round where a commit decision was made.",
                registry,
            ).unwrap(),
            leader_timeout_total: register_int_counter_vec_with_registry!(
                "leader_timeout_total",
                "Total number of leader timeouts, either when the min round time has passed, or max leader timeout",
                &["timeout_type"],
                registry,
            ).unwrap(),
            smart_selection_wait: register_int_counter_with_registry!(
                "smart_selection_wait",
                "Number of times we waited for smart ancestor selection.",
                registry,
            ).unwrap(),
            ancestor_state_change_by_authority: register_int_counter_vec_with_registry!(
                "ancestor_state_change_by_authority",
                "The total number of times an ancestor state changed to EXCLUDE or INCLUDE.",
                &["authority", "state"],
                registry,
            ).unwrap(),
            excluded_proposal_ancestors_count_by_authority: register_int_counter_vec_with_registry!(
                "excluded_proposal_ancestors_count_by_authority",
                "Total number of excluded ancestors per authority during proposal.",
                &["authority"],
                registry,
            ).unwrap(),
            included_excluded_proposal_ancestors_count_by_authority: register_int_counter_vec_with_registry!(
                "included_excluded_proposal_ancestors_count_by_authority",
                "Total number of ancestors per authority with 'excluded' status that got included in proposal. Either weak or strong type.",
                &["authority", "type"],
                registry,
            ).unwrap(),
            missing_blocks_total: register_int_counter_with_registry!(
                "missing_blocks_total",
                "Total cumulative number of missing blocks",
                registry,
            ).unwrap(),
            missing_blocks_after_fetch_total: register_int_counter_with_registry!(
                "missing_blocks_after_fetch_total",
                "Total number of missing blocks after fetching blocks from peer",
                registry,
            ).unwrap(),
            num_of_bad_nodes: register_int_gauge_with_registry!(
                "num_of_bad_nodes",
                "The number of bad nodes in the new leader schedule",
                registry
            ).unwrap(),
            quorum_receive_latency: register_histogram_with_registry!(
                "quorum_receive_latency",
                "The time it took to receive a new round quorum of blocks",
                registry
            ).unwrap(),
            reputation_scores: register_int_gauge_vec_with_registry!(
                "reputation_scores",
                "Reputation scores for each authority",
                &["authority"],
                registry,
            ).unwrap(),
            scope_processing_time: register_histogram_vec_with_registry!(
                "scope_processing_time",
                "The processing time of a specific code scope",
                &["scope"],
                FINE_GRAINED_LATENCY_SEC_BUCKETS.to_vec(),
                registry
            ).unwrap(),
            sub_dags_per_commit_count: register_histogram_with_registry!(
                "sub_dags_per_commit_count",
                "The number of subdags per commit.",
                registry,
            ).unwrap(),
            block_suspensions: register_int_counter_vec_with_registry!(
                "block_suspensions",
                "The number block suspensions. The counter is reported uniquely, so if a block is sent for reprocessing while already suspended then is not double counted",
                &["authority"],
                registry,
            ).unwrap(),
            block_unsuspensions: register_int_counter_vec_with_registry!(
                "block_unsuspensions",
                "The number of block unsuspensions.",
                &["authority"],
                registry,
            ).unwrap(),
            suspended_block_time: register_histogram_vec_with_registry!(
                "suspended_block_time",
                "The time for which a block remains suspended",
                &["authority"],
                registry,
            ).unwrap(),
            block_manager_suspended_blocks: register_int_gauge_with_registry!(
                "block_manager_suspended_blocks",
                "The number of blocks currently suspended in the block manager",
                registry,
            ).unwrap(),
            block_manager_missing_ancestors: register_int_gauge_with_registry!(
                "block_manager_missing_ancestors",
                "The number of missing ancestors tracked in the block manager",
                registry,
            ).unwrap(),
            block_manager_missing_blocks: register_int_gauge_with_registry!(
                "block_manager_missing_blocks",
                "The number of blocks missing content tracked in the block manager",
                registry,
            ).unwrap(),
            block_manager_missing_blocks_by_authority: register_int_counter_vec_with_registry!(
                "block_manager_missing_blocks_by_authority",
                "The number of new missing blocks by block authority",
                &["authority"],
                registry,
            ).unwrap(),
            block_manager_missing_ancestors_by_authority: register_int_counter_vec_with_registry!(
                "block_manager_missing_ancestors_by_authority",
                "The number of missing ancestors by ancestor authority across received blocks",
                &["authority"],
                registry,
            ).unwrap(),
            block_manager_gced_blocks: register_int_counter_vec_with_registry!(
                "block_manager_gced_blocks",
                "The number of blocks that garbage collected and did not get accepted, counted by block's source authority",
                &["authority"],
                registry,
            ).unwrap(),
            block_manager_gc_unsuspended_blocks: register_int_counter_vec_with_registry!(
                "block_manager_gc_unsuspended_blocks",
                "The number of blocks unsuspended because their missing ancestors are garbage collected by the block manager, counted by block's source authority",
                &["authority"],
                registry,
            ).unwrap(),
            block_manager_skipped_blocks: register_int_counter_vec_with_registry!(
                "block_manager_skipped_blocks",
                "The number of blocks skipped by the block manager due to block round being <= gc_round",
                &["authority"],
                registry,
            ).unwrap(),
            threshold_clock_round: register_int_gauge_with_registry!(
                "threshold_clock_round",
                "The current threshold clock round. We only advance to a new round when a quorum of parents have been synced.",
                registry,
            ).unwrap(),
            subscriber_connection_attempts: register_int_counter_vec_with_registry!(
                "subscriber_connection_attempts",
                "The number of connection attempts per peer",
                &["authority", "status"],
                registry,
            ).unwrap(),
            subscribed_to: register_int_gauge_vec_with_registry!(
                "subscribed_to",
                "Peers that this authority subscribed to for block streams.",
                &["authority"],
                registry,
            ).unwrap(),
            subscribed_by: register_int_gauge_vec_with_registry!(
                "subscribed_by",
                "Peers subscribing for block streams from this authority.",
                &["authority"],
                registry,
            ).unwrap(),
            commit_sync_inflight_fetches: register_int_gauge_with_registry!(
                "commit_sync_inflight_fetches",
                "The number of inflight fetches in commit syncer",
                registry,
            ).unwrap(),
            commit_sync_pending_fetches: register_int_gauge_with_registry!(
                "commit_sync_pending_fetches",
                "The number of pending fetches in commit syncer",
                registry,
            ).unwrap(),
            commit_sync_fetched_commits: register_int_counter_vec_with_registry!(
                "commit_sync_fetched_commits",
                "The number of commits fetched via commit syncer, labeled by authority.",
                &["authority"],
                registry,
            ).unwrap(),
            commit_sync_fetched_blocks: register_int_counter_vec_with_registry!(
                "commit_sync_fetched_blocks",
                "The number of blocks fetched via commit syncer, labeled by authority",
                &["authority"],
                registry,
            ).unwrap(),
            commit_sync_total_fetched_blocks_size: register_int_counter_vec_with_registry!(
                "commit_sync_total_fetched_blocks_size",
                "The total size in bytes of blocks fetched via commit syncer",
                &["authority"],
                registry,
            ).unwrap(),
            commit_sync_quorum_index: register_int_gauge_with_registry!(
                "commit_sync_quorum_index",
                "The maximum commit index voted by a quorum of authorities",
                registry,
            ).unwrap(),
            commit_sync_highest_synced_index: register_int_gauge_with_registry!(
                "commit_sync_fetched_index",
                "The max commit index among local and fetched commits",
                registry,
            ).unwrap(),
            commit_sync_highest_fetched_index: register_int_gauge_with_registry!(
                "commit_sync_highest_fetched_index",
                "The max commit index that has been fetched via network",
                registry,
            ).unwrap(),
            commit_sync_local_index: register_int_gauge_with_registry!(
                "commit_sync_local_index",
                "The local commit index",
                registry,
            ).unwrap(),
            commit_sync_gap_on_processing: register_int_counter_with_registry!(
                "commit_sync_gap_on_processing",
                "Number of instances where a gap was found in fetched commit processing",
                registry,
            ).unwrap(),
            commit_sync_fetch_loop_latency: register_histogram_with_registry!(
                "commit_sync_fetch_loop_latency",
                "The time taken to finish fetching commits and blocks from a given range",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            commit_sync_fetch_once_latency: register_histogram_vec_with_registry!(
                "commit_sync_fetch_once_latency",
                "The time taken to fetch commits and blocks once, labeled by target authority.",
                &["authority"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            commit_sync_fetch_once_errors: register_int_counter_vec_with_registry!(
                "commit_sync_fetch_once_errors",
                "Number of errors when attempting to fetch commits and blocks from single authority during commit sync.",
                &["authority", "error"],
                registry
            ).unwrap(),
            commit_sync_fetch_commits_handler_uncertified_skipped: register_int_counter_with_registry!(
                "commit_sync_fetch_commits_handler_uncertified_skipped",
                "Number of uncertified commits that got skipped when fetching commits due to lack of votes",
                registry,
            ).unwrap(),
            commit_sync_fetch_missing_blocks: register_int_counter_vec_with_registry!(
                "commit_sync_fetch_missing_blocks",
                "Number of ancestor blocks that are missing when processing blocks via commit sync.",
                &["authority"],
                registry
            ).unwrap(),
            round_prober_received_quorum_round_gaps: register_int_gauge_vec_with_registry!(
                "round_prober_received_quorum_round_gaps",
                "Received round gaps among peers for blocks proposed from each authority",
                &["authority"],
                registry
            ).unwrap(),
            round_prober_accepted_quorum_round_gaps: register_int_gauge_vec_with_registry!(
                "round_prober_accepted_quorum_round_gaps",
                "Accepted round gaps among peers for blocks proposed & accepted from each authority",
                &["authority"],
                registry
            ).unwrap(),
            round_prober_low_received_quorum_round: register_int_gauge_vec_with_registry!(
                "round_prober_low_received_quorum_round",
                "Low quorum round among peers for blocks proposed from each authority",
                &["authority"],
                registry
            ).unwrap(),
            round_prober_low_accepted_quorum_round: register_int_gauge_vec_with_registry!(
                "round_prober_low_accepted_quorum_round",
                "Low quorum round among peers for blocks proposed & accepted from each authority",
                &["authority"],
                registry
            ).unwrap(),
            round_prober_current_received_round_gaps: register_int_gauge_vec_with_registry!(
                "round_prober_current_received_round_gaps",
                "Received round gaps from local last proposed round to the low received quorum round of each peer. Can be negative.",
                &["authority"],
                registry
            ).unwrap(),
            round_prober_current_accepted_round_gaps: register_int_gauge_vec_with_registry!(
                "round_prober_current_accepted_round_gaps",
                "Accepted round gaps from local last proposed & accepted round to the low accepted quorum round of each peer. Can be negative.",
                &["authority"],
                registry
            ).unwrap(),
            round_prober_propagation_delays: register_histogram_with_registry!(
                "round_prober_propagation_delays",
                "Round gaps between the last proposed block round and the lower bound of own quorum round",
                NUM_BUCKETS.to_vec(),
                registry
            ).unwrap(),
            round_prober_last_propagation_delay: register_int_gauge_with_registry!(
                "round_prober_last_propagation_delay",
                "Most recent propagation delay observed by RoundProber",
                registry
            ).unwrap(),
            round_prober_request_errors: register_int_counter_vec_with_registry!(
                "round_prober_request_errors",
                "Number of errors when probing against peers per error type",
                &["error_type"],
                registry
            ).unwrap(),
            uptime: register_histogram_with_registry!(
                "uptime",
                "Total node uptime",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
        }
    }
}

pub(crate) struct ValidatorScoringMetrics {
    pub(crate) uncached: Vec<UncachedScoringMetrics>,
    pub(crate) cached: Vec<CachedScoringMetrics>,
    pub(crate) score: Vec<Score>,
}

impl ValidatorScoringMetrics {
    pub(crate) fn new(committee_size: usize) -> Self {
        let uncached = (0..committee_size)
            .map(|_| UncachedScoringMetrics::new())
            .collect();
        let cached = (0..committee_size)
            .map(|_| CachedScoringMetrics::new())
            .collect();
        let score = (0..committee_size).map(|_| Score::new()).collect();
        Self {
            uncached,
            cached,
            score,
        }
    }
}

#[derive(Debug)]
pub(crate) struct UncachedScoringMetrics {
    // Counts the number of times that a faulty block signed by the validator was already verified
    // in the epoch.
    pub(crate) faulty_blocks_provable_by_authority: AtomicU64,
    // Counts the number of times that a faulty block not signed by the validator was already
    // verified in the epoch.
    pub(crate) faulty_blocks_unprovable_by_authority: AtomicU64,
    // Counts the number of equivocations that were already evicted from cache in the epoch.
    pub(crate) equivocations_by_authority: AtomicU64,
    // Counts the number of blocks that the validator failed to propose, or that the node did not
    // receive, from the rounds already evicted from cache in the epoch.
    pub(crate) missing_proposals_by_authority: AtomicU64,
}

impl UncachedScoringMetrics {
    pub(crate) fn new() -> Self {
        Self {
            faulty_blocks_provable_by_authority: AtomicU64::new(0),
            faulty_blocks_unprovable_by_authority: AtomicU64::new(0),
            equivocations_by_authority: AtomicU64::new(0),
            missing_proposals_by_authority: AtomicU64::new(0),
        }
    }
}

pub(crate) struct CachedScoringMetrics {
    // Counts the number of equivocations in cache, below the threshold clock round.
    pub(crate) equivocations_by_authority: AtomicU64,
    // Counts the number of blocks that the validator failed to propose, or that the node did not
    // receive yet, from the rounds stored in cache and below the threshold clock round.
    pub(crate) missing_proposals_by_authority: AtomicU64,
}

impl CachedScoringMetrics {
    pub(crate) fn new() -> Self {
        Self {
            equivocations_by_authority: AtomicU64::new(0),
            missing_proposals_by_authority: AtomicU64::new(0),
        }
    }
}

// This struct is used in storage. It holds the same data as
// `UncachedScoringMetrics`, but uses `u64` instead of `AtomicU64`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct StoredScoringMetricsU64 {
    pub(crate) faulty_blocks_provable_by_authority: u64,
    pub(crate) faulty_blocks_unprovable_by_authority: u64,
    pub(crate) equivocations_by_authority: u64,
    pub(crate) missing_proposals_by_authority: u64,
}

pub(crate) fn initialise_metrics(registry: Registry, committee_size: usize) -> Arc<Metrics> {
    let node_metrics = NodeMetrics::new(&registry);
    let network_metrics = NetworkMetrics::new(&registry);
    let scoring_metrics = ValidatorScoringMetrics::new(committee_size);
    Arc::new(Metrics {
        node_metrics,
        network_metrics,
        scoring_metrics,
    })
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

fn should_update_provable_metrics(error: &ConsensusError, source: &str) -> bool {
    if source == "handle_send_block"
        && (is_from_signed_block_verification(error)
            || matches!(
                error,
                ConsensusError::BlockRejected { .. }
                //| ConsensusError::MalformedAncestorBlock { .. }
                ))
    {
        return true;
    }
    false
}

fn should_update_unprovable_metrics(error: &ConsensusError, source: &str) -> bool {
    if source == "handle_send_block" {
        return is_from_unsigned_block_verification(error)
            || matches!(
                error,
                ConsensusError::MalformedBlock { .. } | ConsensusError::UnexpectedAuthority { .. }
            );
    } else if source == "fetch_once" {
        return is_from_commit_syncer(error);
    } else if source == "process_fetched_blocks" {
        return is_from_unsigned_block_verification(error)
            || is_from_signed_block_verification(error)
            || matches!(error, ConsensusError::MalformedBlock { .. });
    }
    false
}

fn is_from_unsigned_block_verification(err: &ConsensusError) -> bool {
    matches!(
        err,
        ConsensusError::WrongEpoch { .. }
            | ConsensusError::UnexpectedGenesisBlock
            | ConsensusError::InvalidAuthorityIndex { .. }
            | ConsensusError::SerializationFailure { .. }
            | ConsensusError::MalformedSignature { .. }
            | ConsensusError::SignatureVerificationFailure { .. }
    )
}

fn is_from_signed_block_verification(err: &ConsensusError) -> bool {
    matches!(
        err,
        ConsensusError::TooManyAncestors { .. }
            | ConsensusError::InsufficientParentStakes { .. }
            | ConsensusError::InvalidAuthorityIndex { .. }
            | ConsensusError::InvalidAncestorPosition { .. }
            | ConsensusError::InvalidAncestorRound { .. }
            | ConsensusError::InvalidGenesisAncestor { .. }
            | ConsensusError::DuplicatedAncestorsAuthority { .. }
            | ConsensusError::TransactionTooLarge { .. }
            | ConsensusError::TooManyTransactions { .. }
            | ConsensusError::TooManyTransactionBytes { .. }
            | ConsensusError::InvalidTransaction { .. }
    )
}

fn is_from_commit_syncer(err: &ConsensusError) -> bool {
    matches!(
        err,
        ConsensusError::MalformedCommit { .. }
            | ConsensusError::UnexpectedStartCommit { .. }
            | ConsensusError::UnexpectedCommitSequence { .. }
            | ConsensusError::NoCommitReceived { .. }
            | ConsensusError::MalformedBlock { .. }
            | ConsensusError::NotEnoughCommitVotes { .. }
            | ConsensusError::UnexpectedNumberOfBlocksFetched { .. }
            | ConsensusError::UnexpectedBlockForCommit { .. }
    ) || is_from_unsigned_block_verification(err)
        || is_from_signed_block_verification(err)
}

#[cfg(test)]
pub(crate) fn test_metrics(committee_size: usize) -> Arc<Metrics> {
    initialise_metrics(Registry::new(), committee_size)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, atomic::Ordering},
        time::Duration,
        vec,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use consensus_config::{AuthorityIndex, NetworkKeyPair, ProtocolKeyPair};
    use parking_lot::RwLock;
    use tokio::sync::broadcast;

    use crate::{
        Round, TransactionVerifier, ValidationError,
        authority_service::{AuthorityService, tests::FakeCoreThreadDispatcher},
        block::{BlockDigest, BlockRef, VerifiedBlock},
        block_verifier::SignedBlockVerifier,
        commit::CommitRange,
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        dag_state::DagState,
        error::ConsensusResult,
        metrics::{ConsensusError, ValidatorScoringMetrics},
        network::{BlockStream, NetworkClient},
        storage::mem_store::MemStore,
        synchronizer::Synchronizer,
        test_dag_builder::DagBuilder,
    };

    struct TxnSizeVerifier {}

    impl TransactionVerifier for TxnSizeVerifier {
        fn verify_batch(&self, _transactions: &[&[u8]]) -> Result<(), ValidationError> {
            unimplemented!("Unimplemented")
        }
    }

    #[derive(Default)]
    struct FakeNetworkClient {}

    #[async_trait]
    impl NetworkClient for FakeNetworkClient {
        const SUPPORT_STREAMING: bool = false;

        async fn send_block(
            &self,
            _peer: AuthorityIndex,
            _block: &VerifiedBlock,
            _timeout: Duration,
        ) -> ConsensusResult<()> {
            unimplemented!("Unimplemented")
        }

        async fn subscribe_blocks(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockStream> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_blocks(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_latest_blocks(
            &self,
            _peer: AuthorityIndex,
            _authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn get_latest_rounds(
            &self,
            _peer: AuthorityIndex,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Round>, Vec<Round>)> {
            unimplemented!("Unimplemented")
        }
    }

    // Creates a new authority service for scoring metrics testing purposes.
    fn new_authority_service_for_metrics_tests(
        committee_size: usize,
    ) -> (
        Vec<(NetworkKeyPair, ProtocolKeyPair)>,
        Arc<Context>,
        Arc<FakeCoreThreadDispatcher>,
        Arc<AuthorityService<FakeCoreThreadDispatcher>>,
    ) {
        let (context, keys) = Context::new_for_test(committee_size);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher::new());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let synchronizer = Synchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state,
            store,
        ));
        (keys, context, core_dispatcher, authority_service)
    }

    impl ValidatorScoringMetrics {
        pub(crate) fn uncached_missing_proposals_by_authority(&self) -> Vec<u64> {
            self.uncached
                .iter()
                .map(|metrics| {
                    metrics
                        .missing_proposals_by_authority
                        .load(Ordering::Relaxed)
                })
                .collect()
        }

        pub(crate) fn equivocations_in_cache_by_authority(&self) -> Vec<u64> {
            self.cached
                .iter()
                .map(|metrics| metrics.equivocations_by_authority.load(Ordering::Relaxed))
                .collect()
        }

        pub(crate) fn missing_proposals_in_cache_by_authority(&self) -> Vec<u64> {
            self.cached
                .iter()
                .map(|metrics| {
                    metrics
                        .missing_proposals_by_authority
                        .load(Ordering::Relaxed)
                })
                .collect()
        }

        pub(crate) fn uncached_equivocations_by_authority(&self) -> Vec<u64> {
            self.uncached
                .iter()
                .map(|metrics| metrics.equivocations_by_authority.load(Ordering::Relaxed))
                .collect()
        }

        pub(crate) fn faulty_blocks_provable_by_authority(&self) -> Vec<u64> {
            self.uncached
                .iter()
                .map(|metrics| {
                    metrics
                        .faulty_blocks_provable_by_authority
                        .load(Ordering::Relaxed)
                })
                .collect()
        }

        pub(crate) fn faulty_blocks_unprovable_by_authority(&self) -> Vec<u64> {
            self.uncached
                .iter()
                .map(|metrics| {
                    metrics
                        .faulty_blocks_unprovable_by_authority
                        .load(Ordering::Relaxed)
                })
                .collect()
        }
    }

    #[tokio::test]
    async fn test_metrics_flush_and_recovery_gc_enabled() {
        telemetry_subscribers::init_for_testing();

        const GC_DEPTH: u32 = 3;
        const CACHED_ROUNDS: u32 = 4;

        let committee_size = 4;
        let (mut context, _) = Context::new_for_test(committee_size);

        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;
        context
            .protocol_config
            .set_consensus_gc_depth_for_testing(GC_DEPTH);
        context
            .protocol_config
            .set_consensus_linearize_subdag_v2_for_testing(true);

        let context = Arc::new(context);
        let metrics = &context.metrics.scoring_metrics;

        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

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
        dag_state.accept_blocks(dag_builder.blocks(1..=10));
        for commit in commits.clone() {
            dag_state.add_commit(commit);
        }

        // Checks that metrics are still all zeroed, since even though we accepted
        // blocks to the dag state, the metrics updates are done when the dag state is
        // flushed.
        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
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
        // - The last_commit_round should be 10, so gc_round should be 6. The eviction
        //   round, then, should be 6 for all authorities.
        // - The threshold_clock_round should be 11, since we already accepted all
        //   blocks from epoch 10.
        // - Then, finally, we should have counted:
        //      - 1 uncached missing proposal for authority 0;
        //      - 2 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
            ]
        );

        // Clear and check all metrics
        metrics.uncached[0]
            .missing_proposals_by_authority
            .store(0, Ordering::Relaxed);
        metrics.cached[0]
            .missing_proposals_by_authority
            .store(0, Ordering::Relaxed);
        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Metrics should have been initialized as before the recovery.
        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
            ]
        );

        // Add blocks and commits from rounds 11 and 12 to the dag state.
        let second_temp_commits = temp_commits.split_off(2);
        dag_state.accept_blocks(dag_builder.blocks(11..=12));
        for commit in temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Flush the dag state
        dag_state.flush();

        // Check that metrics were updated correctly after flushing.
        //
        // Missing proposals:
        // - The last_commit_round should be 12, so gc_round should be 8. The eviction
        //   round, then, should be 8 for all authorities. Then, we should have counted:
        //      - 3 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        //
        // Equivocations:
        // - We only removed from cache blocks from rounds <= 8, thus, no equivocations
        //   should be in cache. Then, we should have counted:
        //      - 0 uncached equivocations;
        //      - 1 equivocation in cache for authority 1;
        //      - 0 equivocations in cache for authorities 0, 2 and 3;
        //

        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0; committee_size],
            ]
        );

        // Accept all the rest of blocks and commits.
        dag_state.accept_blocks(dag_builder.blocks(13..=20));
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Clear and check all metrics
        metrics.uncached[0]
            .missing_proposals_by_authority
            .store(0, Ordering::Relaxed);
        metrics.cached[1]
            .equivocations_by_authority
            .store(0, Ordering::Relaxed);

        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Since the last accepted blocks were not flushed, the equivocations from
        // rounds 13 to 20 should not be accounted for. The metrics should remain
        // the same as before this acceptance.
        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0; committee_size],
            ]
        );

        // Now we accept those lost blocks again and flush the dag state
        dag_state.accept_blocks(dag_builder.blocks(13..=20));
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Now all misbehaviours should be accounted for in the uncached metrics.
        assert_eq!(
            [
                metrics.uncached_equivocations_by_authority(),
                metrics.uncached_missing_proposals_by_authority(),
                metrics.equivocations_in_cache_by_authority(),
                metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0, 1, 2, 0],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
            ]
        );
    }

    #[tokio::test]
    async fn test_metrics_handle_send_block() {
        // Initialize context and authority service given a committee_size
        let committee_size = 4;
        let (_, context, _, _) = new_authority_service_for_metrics_tests(committee_size);
        let metrics = &context.metrics.scoring_metrics;
        let source = "handle_send_block";
        let hostname = "hostname";
        // Create a set of errors to test
        let ignored_error = ConsensusError::Shutdown;
        let parsing_error = ConsensusError::MalformedBlock(bcs::Error::Eof);
        let block_verification_error = ConsensusError::BlockRejected {
            block_ref: BlockRef::new(10, AuthorityIndex::new_for_test(10), BlockDigest::MIN),
            reason: "string".to_string(),
        };

        let authorities = (0..committee_size as u32)
            .map(AuthorityIndex::new_for_test)
            .collect::<Vec<_>>();

        // Update metrics for each authority with an error that should be ignored.
        // Metrics should not be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                ignored_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![0, 0, 0, 0]]
        );

        // Update metrics for each authority with a parsing error.
        // Only unprovable metrics should be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                parsing_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![1, 1, 1, 1]]
        );

        // Update metrics for each authority with a signed block verification error.
        // Only provable metrics should be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                block_verification_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![1, 1, 1, 1], vec![1, 1, 1, 1]]
        );
    }

    #[tokio::test]
    async fn test_metrics_fetch_once() {
        // Initialize context and authority service given a committee_size
        let committee_size = 4;
        let (_, context, _, _) = new_authority_service_for_metrics_tests(committee_size);
        let metrics = &context.metrics.scoring_metrics;
        let source = "fetch_once";
        let hostname = "hostname";
        // Create a set of errors to test
        let ignored_error = ConsensusError::Shutdown;
        let parsing_error = ConsensusError::MalformedBlock(bcs::Error::Eof);
        let block_verification_error = ConsensusError::TooManyAncestors(2, 2);

        let authorities = (0..committee_size as u32)
            .map(AuthorityIndex::new_for_test)
            .collect::<Vec<_>>();

        // Update metrics for each authority with an error that should be ignored.
        // Metrics should not be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                ignored_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![0, 0, 0, 0]]
        );

        // Update metrics for each authority with a parsing error.
        // Only unprovable metrics should be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                parsing_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![1, 1, 1, 1]]
        );

        // Update metrics for each authority with a signed block verification error.
        // Since for error comes from the commit syncer, blocks received are not
        // necessarily from the peer. Thus, it is not provable that the peer actually
        // sent this block. Only unprovable metrics should be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                block_verification_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![2, 2, 2, 2]]
        );
    }

    #[tokio::test]
    async fn test_metrics_process_fetched_blocks() {
        // Initialize context and authority service given a committee_size
        let committee_size = 4;
        let (_, context, _, _) = new_authority_service_for_metrics_tests(committee_size);
        let metrics = &context.metrics.scoring_metrics;
        let source = "process_fetched_blocks";
        let hostname = "hostname";
        // Create a set of errors to test
        let ignored_error = ConsensusError::Shutdown;
        let parsing_error = ConsensusError::MalformedBlock(bcs::Error::Eof);
        let block_verification_error = ConsensusError::TooManyAncestors(2, 2);

        let authorities = (0..committee_size as u32)
            .map(AuthorityIndex::new_for_test)
            .collect::<Vec<_>>();

        // Update metrics for each authority with an error that should be ignored.
        // Metrics should not be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                ignored_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![0, 0, 0, 0]]
        );

        // Update metrics for each authority with a parsing error.
        // Only unprovable metrics should be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                parsing_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![1, 1, 1, 1]]
        );

        // Update metrics for each authority with a signed block verification error.
        // Since for error comes from the synchronizer, blocks received are not
        // necessarily from the peer. Thus, it is not provable that the peer actually
        // sent this block. Only unprovable metrics should be updated for this error.
        for authority in authorities.iter() {
            context.metrics.update_scoring_metrics_on_block_receival(
                *authority,
                hostname,
                block_verification_error.clone(),
                source,
            );
        }
        assert_eq!(
            [
                metrics.faulty_blocks_provable_by_authority(),
                metrics.faulty_blocks_unprovable_by_authority()
            ],
            [vec![0, 0, 0, 0], vec![2, 2, 2, 2]]
        );
    }
}
