// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicU64, Ordering};

use iota_protocol_config::ProtocolConfig;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tracing::warn;

use crate::{messages_consensus::VersionedMisbehaviorReport, misbehavior_counts::MisbehaviorsV1};

// Misbehavior counts using atomic counters, for in-memory concurrent updates.
// Each field is a `Vec<AtomicU64>` with one entry per authority.
type ScoringMetricsV1 = MisbehaviorsV1<Vec<AtomicU64>>;

// We can't serialize VersionedScoringMetrics directly because it contains
// atomic types. This type is only introduces to enable this serialization.
// Converts between atomic (in-memory) and non-atomic (serialized)
// representations.
#[derive(Serialize, Deserialize)]
enum SerializableScoringMetrics {
    V1(MisbehaviorsV1<Vec<u64>>),
}

// Versioned container for scoring metrics using atomic counters.
#[derive(Debug)]
pub enum VersionedScoringMetrics {
    V1(ScoringMetricsV1),
}

impl Serialize for VersionedScoringMetrics {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let serializable = match self {
            VersionedScoringMetrics::V1(metrics) => {
                SerializableScoringMetrics::V1(metrics.as_non_atomic())
            }
        };
        serializable.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VersionedScoringMetrics {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let serializable = SerializableScoringMetrics::deserialize(deserializer)?;
        Ok(match serializable {
            SerializableScoringMetrics::V1(non_atomic) => {
                VersionedScoringMetrics::V1(non_atomic.as_atomic())
            }
        })
    }
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
        for metric in self.iter().flatten() {
            metric.store(0, Ordering::Relaxed);
        }
    }

    pub fn iter(&self) -> std::vec::IntoIter<&Vec<AtomicU64>> {
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
        for (current_value, new_value) in self.iter().flatten().zip(report.iter().flatten()) {
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

    // Creates an independent snapshot by reading all atomic values and creating
    // new atomics. Used to produce an owned copy for storage writes.
    pub fn snapshot(&self) -> Self {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                VersionedScoringMetrics::V1(metrics.as_non_atomic().as_atomic())
            }
        }
    }
}
