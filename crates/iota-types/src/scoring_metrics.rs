// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicU64, Ordering};

use iota_protocol_config::ProtocolConfig;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{messages_consensus::VersionedMisbehaviorReport, misbehavior_counts::MisbehaviorsV1};

// Misbehavior counts using atomic counters, for in-memory concurrent updates.
// Each field is a `Vec<AtomicU64>` with one entry per authority.
type ScoringMetricsV1 = MisbehaviorsV1<Vec<AtomicU64>>;

// Versioned container for scoring metrics using atomic counters.
pub enum VersionedScoringMetrics {
    V1(ScoringMetricsV1),
}

// Basic getters, setters and increments for the metrics. We also introduce
// methods to convert to/from VersionedMisbehaviorReport.
impl VersionedScoringMetrics {
    pub fn new(committee_size: usize, protocol_config: &ProtocolConfig) -> Self {
        // All metrics must be initialized to zero independently of the Misbehaviors
        // version.
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => {
                VersionedScoringMetrics::V1(ScoringMetricsV1::new_zeroed(committee_size))
            }
            _ => panic!("Unsupported scorer version"),
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn increment_faulty_blocks_provable(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_provable()[authority_index]
                    .fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn increment_faulty_blocks_unprovable(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_unprovable()[authority_index]
                    .fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn increment_equivocations(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.equivocations()[authority_index].fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn increment_missing_proposals(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.missing_proposals()[authority_index]
                    .fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn store_faulty_blocks_provable(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_provable()[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn store_faulty_blocks_unprovable(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_unprovable()[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn store_equivocations(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.equivocations()[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    // Validity checks are done at a higher level to ensure authority_index is
    // valid.
    pub fn store_missing_proposals(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.missing_proposals()[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    pub fn load_faulty_blocks_provable(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .faulty_blocks_provable()
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn load_faulty_blocks_unprovable(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .faulty_blocks_unprovable()
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn load_equivocations(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .equivocations()
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn load_missing_proposals(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .missing_proposals()
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn faulty_blocks_provable(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics.faulty_blocks_provable(),
        }
    }

    pub fn faulty_blocks_unprovable(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics.faulty_blocks_unprovable(),
        }
    }

    pub fn equivocations(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics.equivocations(),
        }
    }

    pub fn missing_proposals(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics.missing_proposals(),
        }
    }

    pub fn reset(&self) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                for metric in metrics.faulty_blocks_provable() {
                    metric.store(0, Ordering::Relaxed);
                }
                for metric in metrics.faulty_blocks_unprovable() {
                    metric.store(0, Ordering::Relaxed);
                }
                for metric in metrics.equivocations() {
                    metric.store(0, Ordering::Relaxed);
                }
                for metric in metrics.missing_proposals() {
                    metric.store(0, Ordering::Relaxed);
                }
            }
        }
    }

    pub fn iterate_over_metrics(&self) -> std::vec::IntoIter<&Vec<AtomicU64>> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics.iter(),
        }
    }

    // Given a VersionedMisbehaviorReport received from another authority, we use
    // this method to update the received scoring metrics counts. To avoid
    // updates to be dependent on the order they are applied, we only effectively
    // update counts that are increased by the report. This also means that any type
    // of metric contained in this struct must be guaranteed to be monotonically
    // increasing. Example: number of faulty blocks detected for a given authority
    // is monotonically increasing by design; average faulty blocks per minute is
    // not.
    pub fn update_from_report(&self, report: &VersionedMisbehaviorReport) {
        if !self.has_compatible_version(report) {
            warn!(
                "Metrics counts being updated according to a report with incompatible version, but report versions were already checked before this point!"
            );
        }
        for (current_value, new_value) in self
            .iterate_over_metrics()
            .flatten()
            .zip(report.iterate_over_metrics().flatten())
        {
            current_value.fetch_max(*new_value, Ordering::Relaxed);
        }
    }

    // Given a VersionedMisbehaviorReport, create a VersionedScoringMetrics struct
    // with the same values. Used when an authority receives a report from the
    // network and needs to create a local copy of the metrics contained in it.
    pub fn from_report(report: &VersionedMisbehaviorReport) -> Self {
        match report {
            VersionedMisbehaviorReport::V1(non_atomic_metrics, _) => {
                let atomic_metrics = non_atomic_metrics.as_atomic();
                VersionedScoringMetrics::V1(atomic_metrics)
            }
        }
    }

    // Given a VersionedScoringMetrics struct, create a VersionedMisbehaviorReport
    // with the same values. Used when an authority needs to share its local
    // metrics with the network.
    pub fn to_report(&self) -> VersionedMisbehaviorReport {
        match self {
            VersionedScoringMetrics::V1(atomic_metrics) => {
                let non_atomic_metrics = atomic_metrics.as_non_atomic();
                VersionedMisbehaviorReport::new_v1(non_atomic_metrics)
            }
        }
    }

    // Checks if the version of the scoring metrics is compatible with the version
    // of the misbehavior report.
    pub fn has_compatible_version(&self, report: &VersionedMisbehaviorReport) -> bool {
        match (self, report) {
            (VersionedScoringMetrics::V1(_), VersionedMisbehaviorReport::V1(_, _)) => true,
        }
    }
}

// Misbehavior counts using u64, used for storage. Given an authority, each field
// of this type is a u64 with the metric value for that specific authority.
type StorageScoringMetricsV1 = MisbehaviorsV1<u64>;

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

    // Returns an iterator over references to the metric values.
    pub fn iterate_over_metrics(&self) -> std::vec::IntoIter<&u64> {
        match self {
            VersionedStorageScoringMetrics::V1(inner) => inner.iter(),
        }
    }

    pub fn new_v1_for_test(
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
