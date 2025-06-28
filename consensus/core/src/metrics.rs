// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use consensus_config::AuthorityIndex;
use prometheus::{
    Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Registry,
    exponential_buckets, register_histogram_vec_with_registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry, register_int_counter_with_registry,
    register_int_gauge_vec_with_registry, register_int_gauge_with_registry,
};

use crate::network::metrics::NetworkMetrics;

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
}

pub(crate) fn initialise_metrics(registry: Registry) -> Arc<Metrics> {
    let node_metrics = NodeMetrics::new(&registry);
    let network_metrics = NetworkMetrics::new(&registry);

    Arc::new(Metrics {
        node_metrics,
        network_metrics,
    })
}

#[cfg(test)]
pub(crate) fn test_metrics() -> Arc<Metrics> {
    initialise_metrics(Registry::new())
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
    pub(crate) synchronizer_missing_blocks_by_authority: IntCounterVec,
    pub(crate) synchronizer_current_missing_blocks_by_authority: IntGaugeVec,
    pub(crate) synchronizer_fetched_blocks_by_authority: IntCounterVec,
    pub(crate) network_received_excluded_ancestors_from_authority: IntCounterVec,
    pub(crate) network_excluded_ancestors_sent_to_fetch: IntCounterVec,
    pub(crate) network_excluded_ancestors_count_by_authority: IntCounterVec,
    pub(crate) invalid_blocks: IntCounterVec,
    pub(crate) semantically_invalid_blocks: IntCounterVec,
    pub(crate) syntactically_invalid_blocks: IntCounterVec,
    pub(crate) _equivocating_rounds_by_authority: IntCounterVec,
    pub(crate) _missing_proposals_by_authority: IntCounterVec,
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
            synchronizer_missing_blocks_by_authority: register_int_counter_vec_with_registry!(
                "synchronizer_missing_blocks_by_authority",
                "Number of missing blocks per block author, as observed by the synchronizer during periodic sync.",
                &["authority"],
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
            semantically_invalid_blocks: register_int_counter_vec_with_registry!(
                "semantically_invalid_blocks",
                "Number of semantically invalid blocks per peer authority",
                &["authority", "source", "error"],
                registry,
             ).unwrap(),
            syntactically_invalid_blocks: register_int_counter_vec_with_registry!(
                "syntactically_invalid_blocks",
                "Number of syntactically invalid blocks per peer authority",
                &["authority", "source", "error"],
                registry,
            ).unwrap(),
            _equivocating_rounds_by_authority: register_int_counter_vec_with_registry!(
                "equivocating_rounds_by_authority",
                "Registers the number of rounds when the authority sent an equivocating block.",
                &["authority"],
                registry,
            ).unwrap(),
            _missing_proposals_by_authority: register_int_counter_vec_with_registry!(
                "missing_proposals_by_authority",
                "Registers the number of blocks that an authority failed to send.",
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

// Metrics stored related to the current epoch used to calculate the validator
// score.
#[derive(Clone)]
pub(crate) struct ValidatorScoreMetrics {
    // Each entry in the vector corresponds to a counter relative to an active validator, indexed
    // by AuthorityIndex. For each of those validators, we count the number of times that a
    // semantically invalid block signed by the validator was already verified in the epoch.
    #[allow(dead_code)]
    pub(crate) semantically_invalid_blocks: Arc<Vec<AtomicU64>>,
    // Each entry in the vector corresponds to a counter relative to an active validator, indexed
    // by AuthorityIndex. For each of those validators, we count the number of syntactically
    // invalid blocks sent by the validator that were already handled in the epoch.
    pub(crate) syntactically_invalid_blocks: Arc<Vec<AtomicU64>>,
    // Each entry in the vector corresponds to a counter relative to an active validator, indexed
    // by AuthorityIndex. For each of those validators, we count the number of blocks that the
    // validator failed to propose.
    pub(crate) _missing_proposals_by_authority: Arc<Vec<AtomicU64>>,
    // Each entry in the vector corresponds to a counter relative to an active validator, indexed
    // by AuthorityIndex. For each of those validators, we count the number of rounds
    // they sent equivocating blocks.
    pub(crate) _equivocating_rounds: Arc<Vec<AtomicU64>>,
}

impl ValidatorScoreMetrics {
    pub(crate) fn new(committee_size: usize) -> Self {
        let mut semantically_invalid_blocks_inner = vec![];
        semantically_invalid_blocks_inner.resize_with(committee_size, || AtomicU64::new(0));

        let mut syntactically_invalid_blocks_inner = vec![];
        syntactically_invalid_blocks_inner.resize_with(committee_size, || AtomicU64::new(0));

        let mut missing_proposals_by_authority_inner = vec![];
        missing_proposals_by_authority_inner.resize_with(committee_size, || AtomicU64::new(0));

        let mut equivocating_rounds_inner = vec![];
        equivocating_rounds_inner.resize_with(committee_size, || AtomicU64::new(0));

        Self {
            semantically_invalid_blocks: Arc::new(semantically_invalid_blocks_inner),
            syntactically_invalid_blocks: Arc::new(syntactically_invalid_blocks_inner),
            _missing_proposals_by_authority: Arc::new(missing_proposals_by_authority_inner),
            _equivocating_rounds: Arc::new(equivocating_rounds_inner),
        }
    }

    pub(crate) fn update_semantically_invalid_blocks(&self, validator: AuthorityIndex) {
        self.semantically_invalid_blocks[validator.value()].fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn update_syntactically_invalid_blocks(&self, validator: AuthorityIndex) {
        self.syntactically_invalid_blocks[validator.value()].fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use consensus_config::{AuthorityIndex, NetworkKeyPair, ProtocolKeyPair};
    use parking_lot::{Mutex, RwLock};
    use strum::IntoEnumIterator;
    use strum_macros::{Display, EnumIter};
    use tokio::sync::broadcast;

    use crate::{
        Round, Transaction, TransactionVerifier, ValidationError,
        authority_service::AuthorityService,
        block::{BlockDigest, BlockRef, SignedBlock, TestBlock, VerifiedBlock},
        block_verifier::SignedBlockVerifier,
        commit::{CertifiedCommits, CommitRange},
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        core_thread::{CoreError, CoreThreadDispatcher},
        dag_state::DagState,
        error::ConsensusResult,
        metrics::ValidatorScoreMetrics,
        network::{BlockStream, ExtendedSerializedBlock, NetworkClient, NetworkService},
        round_prober::QuorumRound,
        storage::mem_store::MemStore,
        synchronizer::Synchronizer,
    };
    #[derive(Clone)]
    struct TestParameters<'a> {
        round: u32,
        committee_size: usize,
        keypairs: Vec<&'a ProtocolKeyPair>,
    }

    fn generate_default_ancestors(round: u32, author: u32, committee_size: usize) -> Vec<BlockRef> {
        let mut ancestors = (0..committee_size)
            .map(|i| {
                BlockRef::new(
                    round - 1,
                    AuthorityIndex::new_for_test(i as u32),
                    BlockDigest::MIN,
                )
            })
            .collect::<Vec<_>>();
        let own_ancestor = ancestors.remove(author as usize);
        ancestors.insert(0, own_ancestor);
        ancestors
    }
    struct TxnSizeVerifier {}

    impl TransactionVerifier for TxnSizeVerifier {
        // Fails verification if any transaction is < 4 bytes.
        fn verify_batch(&self, transactions: &[&[u8]]) -> Result<(), ValidationError> {
            for txn in transactions {
                if txn.len() < 4 {
                    return Err(ValidationError::InvalidTransaction(format!(
                        "Length {} is too short!",
                        txn.len()
                    )));
                }
            }
            Ok(())
        }
    }
    pub(crate) struct FakeCoreThreadDispatcher {
        blocks: Mutex<Vec<VerifiedBlock>>,
    }

    impl FakeCoreThreadDispatcher {
        fn new() -> Self {
            Self {
                blocks: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait]
    impl CoreThreadDispatcher for FakeCoreThreadDispatcher {
        async fn add_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            let block_refs = blocks.iter().map(|b| b.reference()).collect();
            self.blocks.lock().extend(blocks);
            Ok(block_refs)
        }

        async fn check_block_refs(
            &self,
            _block_refs: Vec<BlockRef>,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            Ok(BTreeSet::new())
        }

        async fn add_certified_commits(
            &self,
            _commits: CertifiedCommits,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            todo!()
        }

        async fn new_block(&self, _round: Round, _force: bool) -> Result<(), CoreError> {
            Ok(())
        }

        async fn get_missing_blocks(&self) -> Result<BTreeSet<BlockRef>, CoreError> {
            Ok(Default::default())
        }

        fn set_quorum_subscribers_exists(&self, _exists: bool) -> Result<(), CoreError> {
            todo!()
        }

        fn set_propagation_delay_and_quorum_rounds(
            &self,
            _delay: Round,
            _received_quorum_rounds: Vec<QuorumRound>,
            _accepted_quorum_rounds: Vec<QuorumRound>,
        ) -> Result<(), CoreError> {
            todo!()
        }

        fn set_last_known_proposed_round(&self, _round: Round) -> Result<(), CoreError> {
            todo!()
        }

        fn highest_received_rounds(&self) -> Vec<Round> {
            todo!()
        }
    }

    pub(crate) fn new_authority_service_for_tests(
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

    #[derive(PartialEq, EnumIter, Display, Clone)]
    enum SemanticallyInvalidBlocks {
        WrongEpoch,
        BlockAtGenesisRound,
        InvalidAncestorRound,
        ParentsNotReachingQuorum,
        TooManyAncestors,
        WithoutOwnAncestor,
        WithOwnAncestorInWrongPosition,
        WithAncestorsFromSameAuthority,
        WithInvalidTransaction,
        TooManyTransactions,
        WithTransactionTooLarge,
        WithTooManyTransactionBytes,
        WrongKey,
        NoSignature,
    }

    impl SemanticallyInvalidBlocks {
        fn new(self, parameters: TestParameters, author: u32) -> ExtendedSerializedBlock {
            let round = parameters.round;
            let committee_size = parameters.committee_size;
            let keypair = parameters.keypairs[author as usize];
            assert!(
                round != 0 || self == SemanticallyInvalidBlocks::BlockAtGenesisRound,
                "round = 0 should be only used for BlockWithCorrectSignatureForTests::BlockAtGenesisRound"
            );

            let ancestors: Vec<BlockRef> = match self {
                SemanticallyInvalidBlocks::InvalidAncestorRound => {
                    let mut modified_ancestors =
                        generate_default_ancestors(round, author, committee_size);
                    let last_author = modified_ancestors.pop().unwrap().author;
                    let new_ancestor = BlockRef::new(round + 1, last_author, BlockDigest::MIN);
                    modified_ancestors.push(new_ancestor);
                    modified_ancestors
                }
                SemanticallyInvalidBlocks::ParentsNotReachingQuorum => {
                    let mut modified_ancestors =
                        generate_default_ancestors(round - 1, author, committee_size);
                    modified_ancestors[0] = BlockRef::new(
                        round - 1,
                        AuthorityIndex::new_for_test(author),
                        BlockDigest::MIN,
                    );
                    modified_ancestors
                }
                SemanticallyInvalidBlocks::TooManyAncestors => {
                    let mut modified_ancestors =
                        generate_default_ancestors(round, author, committee_size);
                    let last_author = modified_ancestors.last().unwrap().author;
                    modified_ancestors.push(BlockRef::new(
                        round - 1,
                        last_author,
                        BlockDigest::MIN,
                    ));
                    modified_ancestors
                }
                SemanticallyInvalidBlocks::WithoutOwnAncestor => {
                    let mut modified_ancestors =
                        generate_default_ancestors(round, author, committee_size);
                    modified_ancestors.remove(0);
                    modified_ancestors
                }
                SemanticallyInvalidBlocks::WithOwnAncestorInWrongPosition => {
                    let mut modified_ancestors =
                        generate_default_ancestors(round, author, committee_size);
                    let own_ancestor = modified_ancestors.remove(0);
                    modified_ancestors.insert(modified_ancestors.len(), own_ancestor);
                    modified_ancestors
                }
                SemanticallyInvalidBlocks::WithAncestorsFromSameAuthority => {
                    let mut default_ancestors =
                        generate_default_ancestors(round, author, committee_size);
                    default_ancestors.push(default_ancestors.last().unwrap().clone());
                    default_ancestors
                }
                _ => generate_default_ancestors(round, author, committee_size),
            };

            let block = TestBlock::new(round, author).set_ancestors(ancestors);

            let test_block = match self {
                SemanticallyInvalidBlocks::WrongEpoch => block.set_epoch(1),
                SemanticallyInvalidBlocks::BlockAtGenesisRound => block.set_round(0),
                SemanticallyInvalidBlocks::WithInvalidTransaction => {
                    block.set_transactions(vec![Transaction::new(vec![1; 2])])
                }
                SemanticallyInvalidBlocks::TooManyTransactions => block
                    .set_transactions((0..1000).map(|_| Transaction::new(vec![4; 8])).collect()),
                SemanticallyInvalidBlocks::WithTransactionTooLarge => {
                    block.set_transactions(vec![Transaction::new(vec![4; 257 * 1024])])
                }
                SemanticallyInvalidBlocks::WithTooManyTransactionBytes => block.set_transactions(
                    (0..100)
                        .map(|_| Transaction::new(vec![4; 8 * 1024]))
                        .collect(),
                ),
                _ => block,
            };
            let signed_block = match self {
                SemanticallyInvalidBlocks::WrongKey => {
                    let wrong_keypair = match author {
                        0 => parameters.keypairs[1],
                        _ => parameters.keypairs[0],
                    };
                    SignedBlock::new(test_block.build(), wrong_keypair).unwrap()
                }
                SemanticallyInvalidBlocks::NoSignature => {
                    let mut sig_block = SignedBlock::new(test_block.build(), keypair).unwrap();
                    sig_block.clear_signature();
                    sig_block
                }
                _ => SignedBlock::new(test_block.build(), keypair).unwrap(),
            };

            let serialized: Bytes = bcs::to_bytes(&signed_block)
                .expect("Serialization should not fail")
                .into();
            let verified_block = VerifiedBlock::new_verified(signed_block, serialized);
            let serialized = ExtendedSerializedBlock {
                block: verified_block.serialized().clone(),
                excluded_ancestors: vec![],
            };
            serialized
        }
    }

    #[derive(PartialEq, EnumIter, Display, Clone)]
    enum SyntacticallyInvalidBlocks {
        MalformedBlocks,
        InvalidAuthorityIndex,
    }

    impl SyntacticallyInvalidBlocks {
        fn new(self, parameters: TestParameters, author: u32) -> ExtendedSerializedBlock {
            if self == SyntacticallyInvalidBlocks::MalformedBlocks {
                ExtendedSerializedBlock {
                    block: Bytes::new(),
                    excluded_ancestors: vec![],
                }
            } else {
                let round = parameters.round;
                let committee_size = parameters.committee_size;
                let keypair = parameters.keypairs[author as usize];
                assert!(
                    round != 0,
                    "round = 0 should be only used for SemanticallyInvalidBlocks::BlockAtGenesisRound"
                );
                let ancestors = generate_default_ancestors(round, author, committee_size);
                let block = TestBlock::new(round, author)
                    .set_ancestors(ancestors)
                    .set_author(AuthorityIndex::new_for_test(committee_size as u32));
                let signed_block = SignedBlock::new(block.build(), keypair).unwrap();
                let serialized: Bytes = bcs::to_bytes(&signed_block)
                    .expect("Serialization should not fail")
                    .into();
                let verified_block = VerifiedBlock::new_verified(signed_block, serialized);
                let serialized = ExtendedSerializedBlock {
                    block: verified_block.serialized().clone(),
                    excluded_ancestors: vec![],
                };
                serialized
            }
        }
    }
    enum ValidBlocks {
        Valid,
    }
    impl ValidBlocks {
        fn new(self, parameters: TestParameters, author: u32) -> ExtendedSerializedBlock {
            let round = parameters.round;
            let committee_size = parameters.committee_size;
            let keypair = parameters.keypairs[author as usize];
            assert!(
                round != 0,
                "round = 0 should be only used for SemanticallyInvalidBlocks::BlockAtGenesisRound"
            );
            let ancestors = generate_default_ancestors(round, author, committee_size);
            let block = TestBlock::new(round, author).set_ancestors(ancestors);
            let signed_block = SignedBlock::new(block.build(), keypair).unwrap();
            let serialized: Bytes = bcs::to_bytes(&signed_block)
                .expect("Serialization should not fail")
                .into();
            let verified_block = VerifiedBlock::new_verified(signed_block, serialized);
            let serialized = ExtendedSerializedBlock {
                block: verified_block.serialized().clone(),
                excluded_ancestors: vec![],
            };
            serialized
        }
    }
    impl ValidatorScoreMetrics {
        fn _get_semantically_invalid_blocks(&self, validator: AuthorityIndex) -> u64 {
            self.semantically_invalid_blocks[validator.value()].load(Ordering::Relaxed)
        }
        fn _get_semantically_invalid_blocks_vec(&self) -> Vec<u64> {
            self.semantically_invalid_blocks
                .iter()
                .map(|x| x.load(Ordering::Relaxed))
                .collect()
        }

        fn _get_syntactically_invalid_blocks(&self, validator: AuthorityIndex) -> u64 {
            self.syntactically_invalid_blocks[validator.value()].load(Ordering::Relaxed)
        }
        fn _get_missing_block_proposals(&self, validator: AuthorityIndex) -> u64 {
            self._missing_proposals_by_authority[validator.value()].load(Ordering::Relaxed)
        }
        fn _get_equivocating_rounds(&self, validator: AuthorityIndex) -> u64 {
            self._equivocating_rounds[validator.value()].load(Ordering::Relaxed)
        }
        fn assert_semantically_invalid_blocks_equals(&self, vector: &Vec<u64>) {
            let semantically_invalid_blocks_state = self
                .semantically_invalid_blocks
                .iter()
                .map(|x| x.load(Ordering::Relaxed))
                .collect::<Vec<u64>>();
            assert_eq!(
                semantically_invalid_blocks_state, *vector,
                "Semantically invalid blocks state does not match expected vector \n State:  {:?}\n Vector: {:?}",
                semantically_invalid_blocks_state, vector
            );
        }
        fn assert_syntactically_invalid_blocks_equals(&self, vector: &Vec<u64>) {
            let syntactically_invalid_blocks_state = self
                .syntactically_invalid_blocks
                .iter()
                .map(|x| x.load(Ordering::Relaxed))
                .collect::<Vec<u64>>();
            assert_eq!(
                syntactically_invalid_blocks_state, *vector,
                "Syntactically invalid blocks state does not match expected vector \n State:  {:?}\n Vector: {:?}",
                syntactically_invalid_blocks_state, vector
            );
        }
    }

    fn assigned_peer(index: usize, committee_size: usize) -> usize {
        index % committee_size
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_metrics_handle_send_block() {
        // Initialize context and authority service given a committee_size
        let committee_size = 4;
        let (keys, context, _, authority_service) = new_authority_service_for_tests(committee_size);

        // Set current round and build TestParameters
        let round = 9;
        let parameters = TestParameters {
            round,
            committee_size,
            keypairs: keys.iter().map(|(_, y)| y).collect(),
        };

        // Initial check: ensure that all metrics are zero
        context
            .scoring_metrics
            .assert_semantically_invalid_blocks_equals(&vec![0; committee_size]);
        context
            .scoring_metrics
            .assert_syntactically_invalid_blocks_equals(&vec![0; committee_size]);

        // Generates a single valid block from each authority
        let blocks = (0..committee_size)
            .map(|i| ValidBlocks::Valid.new(parameters.clone(), i as u32))
            .collect::<Vec<_>>();

        // Handle blocks created in the previous step.
        let mut tasks = Vec::new();
        for (index, block) in blocks.into_iter().enumerate() {
            let service = authority_service.clone();
            tasks.push(tokio::spawn(async move {
                service
                    .handle_send_block(AuthorityIndex::new_for_test(index as u32), block)
                    .await
                    .unwrap();
            }));
        }

        let mut outputs = Vec::with_capacity(tasks.len());
        for task in tasks {
            outputs.push(task.await.unwrap());
        }

        // Ensure that the metrics are still all zero
        context
            .scoring_metrics
            .assert_semantically_invalid_blocks_equals(&vec![0; committee_size]);
        context
            .scoring_metrics
            .assert_syntactically_invalid_blocks_equals(&vec![0; committee_size]);

        // Generates one of each type of semantically invalid blocks. Assigns each of
        // those blocks to a peer. Authors and peers are the same.
        let semantically_invalid_blocks = SemanticallyInvalidBlocks::iter()
            .enumerate()
            .map(|(index, block_type)| {
                (
                    block_type.clone().new(
                        parameters.clone(),
                        assigned_peer(index, committee_size) as u32,
                    ),
                    assigned_peer(index, committee_size),
                    block_type.to_string(),
                )
            })
            .collect::<Vec<_>>();

        // Sends each semantically invalid block to the authority service and check
        // metrics between blocks
        let mut semantic_block_count = vec![0 as u64; committee_size];
        for (block, peer, _block_type) in semantically_invalid_blocks.into_iter() {
            let service = authority_service.clone();
            let handle = tokio::spawn(async move {
                let _ = service
                    .handle_send_block(AuthorityIndex::new_for_test(peer as u32), block)
                    .await;
            });
            let _ = handle.await;
            semantic_block_count[peer] += 1;
            // Check that only the metrics for semantically invalid blocks were updated
            context
                .scoring_metrics
                .assert_semantically_invalid_blocks_equals(&semantic_block_count);
            context
                .scoring_metrics
                .assert_syntactically_invalid_blocks_equals(&vec![0; committee_size]);
        }

        // Generates a single syntactically invalid block from each authority
        let syntactically_invalid_blocks = (0..committee_size)
            .map(|i| SyntacticallyInvalidBlocks::MalformedBlocks.new(parameters.clone(), i as u32))
            .collect::<Vec<_>>();

        // Sends each syntactically invalid block to the authority service and check
        // metrics between blocks
        let mut syntactic_block_count = vec![0 as u64; committee_size];
        for (peer, block) in syntactically_invalid_blocks.into_iter().enumerate() {
            let service = authority_service.clone();
            let handle = tokio::spawn(async move {
                let _ = service
                    .handle_send_block(AuthorityIndex::new_for_test(peer as u32), block)
                    .await;
            });
            let _ = handle.await;
            syntactic_block_count[peer] += 1;
            // Check that only the metrics for syntactically invalid blocks were updated
            context
                .scoring_metrics
                .assert_semantically_invalid_blocks_equals(&semantic_block_count);
            context
                .scoring_metrics
                .assert_syntactically_invalid_blocks_equals(&syntactic_block_count);
        }
    }
}
