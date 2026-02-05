// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicU64, Ordering};

use iota_protocol_config::ProtocolConfig;

use crate::messages_consensus::{MisbehaviorsV1, VersionedMisbehaviorReport};

// This struct represents the scoring metrics collected by all authorities. They
// are stored locally by each authority and then converted to a misbehavior
// report when they share their metrics with the network. When a report is
// received, it is also used to update a variable of this type stored in the
// Scorer. Any metric contained in this struct must be guaranteed to be
// monotonically increasing, because of the way updates are applied from
// reports.
pub enum VersionedScoringMetrics {
    V1(ScoringMetricsV1),
}

// Basic getters, setters and increments for the metrics.
impl VersionedScoringMetrics {
    pub fn new(committee_size: usize, protocol_config: &ProtocolConfig) -> Self {
        // Any version of ScoringMetrics created here must be initialized to zero.
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => VersionedScoringMetrics::V1(ScoringMetricsV1::new(committee_size)),
            _ => panic!("Unsupported scorer version"),
        }
    }

    pub fn increment_faulty_blocks_provable(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_provable[authority_index]
                    .fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    pub fn increment_faulty_blocks_unprovable(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_unprovable[authority_index]
                    .fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    pub fn increment_equivocations(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.equivocations[authority_index].fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    pub fn increment_missing_proposals(&self, authority_index: usize, increment: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.missing_proposals[authority_index].fetch_add(increment, Ordering::Relaxed);
            }
        }
    }

    pub fn store_faulty_blocks_provable(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_provable[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    pub fn store_faulty_blocks_unprovable(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.faulty_blocks_unprovable[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    pub fn store_equivocations(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.equivocations[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    pub fn store_missing_proposals(&self, authority_index: usize, value: u64) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                metrics.missing_proposals[authority_index].store(value, Ordering::Relaxed);
            }
        }
    }

    pub fn load_faulty_blocks_provable(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .faulty_blocks_provable
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn load_faulty_blocks_unprovable(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .faulty_blocks_unprovable
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn load_equivocations(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .equivocations
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn load_missing_proposals(&self) -> Vec<u64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => metrics
                .missing_proposals
                .iter()
                .map(|metric| metric.load(Ordering::Relaxed))
                .collect(),
        }
    }

    pub fn faulty_blocks_provable(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => &metrics.faulty_blocks_provable,
        }
    }

    pub fn faulty_blocks_unprovable(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => &metrics.faulty_blocks_unprovable,
        }
    }

    pub fn equivocations(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => &metrics.equivocations,
        }
    }

    pub fn missing_proposals(&self) -> &Vec<AtomicU64> {
        match self {
            VersionedScoringMetrics::V1(metrics) => &metrics.missing_proposals,
        }
    }

    pub fn reset(&self) {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                for metric in &metrics.faulty_blocks_provable {
                    metric.store(0, Ordering::Relaxed);
                }
                for metric in &metrics.faulty_blocks_unprovable {
                    metric.store(0, Ordering::Relaxed);
                }
                for metric in &metrics.equivocations {
                    metric.store(0, Ordering::Relaxed);
                }
                for metric in &metrics.missing_proposals {
                    metric.store(0, Ordering::Relaxed);
                }
            }
        }
    }
}

impl VersionedScoringMetrics {
    // Given a VersionedMisbehaviorReport received from another authority, we use
    // this method to update the received scoring metrics counts. To avoid
    // updates to be dependent on the order they are applied, we only effectively
    // update counts that are increased by the report. This also means that any type
    // of metric contained in this struct must be guaranteed to be monotonically
    // increasing. Example: number of faulty blocks detected for a given authority
    // is monotonically increasing by design; average faulty blocks per minute is
    // not.
    pub fn update_from_report(&self, report: &VersionedMisbehaviorReport) {
        match (self, report) {
            (
                VersionedScoringMetrics::V1(metrics),
                VersionedMisbehaviorReport::V1(report_v1, _),
            ) => {
                for (i, value) in report_v1.faulty_blocks_provable.iter().enumerate() {
                    metrics.faulty_blocks_provable[i].fetch_max(*value, Ordering::Relaxed);
                }
                for (i, value) in report_v1.faulty_blocks_unprovable.iter().enumerate() {
                    metrics.faulty_blocks_unprovable[i].fetch_max(*value, Ordering::Relaxed);
                }
                for (i, value) in report_v1.equivocations.iter().enumerate() {
                    metrics.equivocations[i].fetch_max(*value, Ordering::Relaxed);
                }
                for (i, value) in report_v1.missing_proposals.iter().enumerate() {
                    metrics.missing_proposals[i].fetch_max(*value, Ordering::Relaxed);
                }
            }
        }
    }

    // Given a VersionedMisbehaviorReport, create a VersionedScoringMetrics struct
    // with the same values. Used when an authority receives a report from the
    // network and needs to create a local copy of the metrics contained in it.
    pub fn from_report(report: &VersionedMisbehaviorReport) -> Self {
        match report {
            VersionedMisbehaviorReport::V1(report_v1, _) => {
                let committee_size = report_v1.faulty_blocks_provable.len();
                let metrics = ScoringMetricsV1::new(committee_size);
                for (i, value) in report_v1.faulty_blocks_provable.iter().enumerate() {
                    metrics.faulty_blocks_provable[i].store(*value, Ordering::Relaxed);
                }
                for (i, value) in report_v1.faulty_blocks_unprovable.iter().enumerate() {
                    metrics.faulty_blocks_unprovable[i].store(*value, Ordering::Relaxed);
                }
                for (i, value) in report_v1.equivocations.iter().enumerate() {
                    metrics.equivocations[i].store(*value, Ordering::Relaxed);
                }
                for (i, value) in report_v1.missing_proposals.iter().enumerate() {
                    metrics.missing_proposals[i].store(*value, Ordering::Relaxed);
                }
                VersionedScoringMetrics::V1(metrics)
            }
        }
    }

    // Given a VersionedScoringMetrics struct, create a VersionedMisbehaviorReport
    // with the same values. Used when an authority needs to share its local
    // metrics with the network.
    pub fn to_report(&self) -> VersionedMisbehaviorReport {
        match self {
            VersionedScoringMetrics::V1(metrics) => {
                let faulty_blocks_provable = metrics
                    .faulty_blocks_provable
                    .iter()
                    .map(|metric| metric.load(Ordering::Relaxed))
                    .collect();
                let faulty_blocks_unprovable = metrics
                    .faulty_blocks_unprovable
                    .iter()
                    .map(|metric| metric.load(Ordering::Relaxed))
                    .collect();
                let equivocations = metrics
                    .equivocations
                    .iter()
                    .map(|metric| metric.load(Ordering::Relaxed))
                    .collect();
                let missing_proposals = metrics
                    .missing_proposals
                    .iter()
                    .map(|metric| metric.load(Ordering::Relaxed))
                    .collect();
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable,
                    faulty_blocks_unprovable,
                    missing_proposals,
                    equivocations,
                })
            }
        }
    }
}

pub struct ScoringMetricsV1 {
    faulty_blocks_provable: Vec<AtomicU64>,
    faulty_blocks_unprovable: Vec<AtomicU64>,
    missing_proposals: Vec<AtomicU64>,
    equivocations: Vec<AtomicU64>,
}

impl ScoringMetricsV1 {
    pub fn new(committee_size: usize) -> Self {
        Self {
            // Blocks considered faulty with provable evidence, i.e., they pass the signature check.
            faulty_blocks_provable: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            // Blocks considered faulty before passing the signature check.
            faulty_blocks_unprovable: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            // Number or rounds that the authority did not propose any block
            missing_proposals: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            // Number of additional blocks issued by a validator within rounds where another block
            // was already produced by them.
            equivocations: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
        }
    }
}
