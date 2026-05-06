// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::AtomicU64;

/// Per-authority misbehavior counters, both cached and uncached.
#[allow(dead_code)]
pub(crate) struct ScoringMetricsStore {
    current_local_metrics_count: StarfishMisbehaviorCounts,
    cached_metrics: StarfishMisbehaviorCounts,
    uncached_metrics: StarfishMisbehaviorCounts,
}

#[allow(dead_code)]
impl ScoringMetricsStore {
    pub(crate) fn new(committee_size: usize) -> Self {
        Self {
            current_local_metrics_count: StarfishMisbehaviorCounts::new(committee_size),
            cached_metrics: StarfishMisbehaviorCounts::new(committee_size),
            uncached_metrics: StarfishMisbehaviorCounts::new(committee_size),
        }
    }
}

/// Per-authority atomic counters for each misbehavior category.
/// Each `Vec<AtomicU64>` is indexed by authority index within the committee.
#[allow(dead_code)]
struct StarfishMisbehaviorCounts {
    faulty_blocks_provable: Vec<AtomicU64>,
    faulty_blocks_unprovable: Vec<AtomicU64>,
    missing_proposals: Vec<AtomicU64>,
    equivocations: Vec<AtomicU64>,
}

#[allow(dead_code)]
impl StarfishMisbehaviorCounts {
    fn new(committee_size: usize) -> Self {
        Self {
            faulty_blocks_provable: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            faulty_blocks_unprovable: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            missing_proposals: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
            equivocations: (0..committee_size).map(|_| AtomicU64::new(0)).collect(),
        }
    }
}
