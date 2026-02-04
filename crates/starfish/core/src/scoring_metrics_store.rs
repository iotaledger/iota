// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_common::scoring_metrics::VersionedScoringMetrics;
use iota_protocol_config::ProtocolConfig;

/// Struct that holds the scoring metrics for all authorities in the committee,
/// both cached and uncached. It also holds a shared reference to the current
/// local metrics count used by Scorer.
pub(crate) struct ScoringMetricsStore {
    #[expect(dead_code)]
    pub current_local_metrics_count: Arc<VersionedScoringMetrics>,
    #[expect(dead_code)]
    pub cached_metrics: VersionedScoringMetrics,
    #[expect(dead_code)]
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
