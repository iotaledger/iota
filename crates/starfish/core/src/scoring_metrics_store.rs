// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use iota_common::scoring_metrics::{VersionedScoringMetrics, VersionedStorageScoringMetrics};
use iota_protocol_config::ProtocolConfig;
use itertools::izip;
use starfish_config::AuthorityIndex;

use crate::{BlockRef, context::Context, metrics::NodeMetrics};

/// Struct that holds the scoring metrics for all authorities in the committee,
/// both cached and uncached. It also holds a shared reference to the current
/// local metrics count used by Scorer.
pub(crate) struct ScoringMetricsStore {
    #[expect(dead_code)]
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
