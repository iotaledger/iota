// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::AtomicU64;

use serde::{Deserialize, Serialize};

/// Per-authority misbehavior counters.
///
/// Three buckets track different lifecycle stages:
/// - `pending`: local accumulator, emitted with each CommittedSubDag for the
///   aggregator.
/// - `in_memory`: from blocks currently in the DAG cache (volatile, recomputed
///   on restart).
/// - `persisted`: from blocks evicted from cache and written to storage
///   (restored on restart).
#[expect(dead_code)]
pub(crate) struct ScoringMetricsStore {
    pending: StarfishMisbehaviorCounts,
    in_memory: StarfishMisbehaviorCounts,
    persisted: StarfishMisbehaviorCounts,
}

#[expect(dead_code)]
impl ScoringMetricsStore {
    pub(crate) fn new(committee_size: usize) -> Self {
        Self {
            pending: StarfishMisbehaviorCounts::new(committee_size),
            in_memory: StarfishMisbehaviorCounts::new(committee_size),
            persisted: StarfishMisbehaviorCounts::new(committee_size),
        }
    }
}

/// Per-authority atomic counters for each misbehavior category.
/// Each `Vec<AtomicU64>` is indexed by authority index within the committee.
#[expect(dead_code)]
struct StarfishMisbehaviorCounts {
    faulty_blocks_provable: Vec<AtomicU64>,
    faulty_blocks_unprovable: Vec<AtomicU64>,
    missing_proposals: Vec<AtomicU64>,
    equivocations: Vec<AtomicU64>,
}

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

/// Versioned envelope for persisted scoring metrics. New versions are added as
/// enum variants so existing RocksDB data deserializes without migration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum StorageScoringMetrics {
    V1(StorageScoringMetricsV1),
}

impl Default for StorageScoringMetrics {
    fn default() -> Self {
        Self::V1(StorageScoringMetricsV1::default())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct StorageScoringMetricsV1 {
    pub(crate) faulty_blocks_provable: u64,
    pub(crate) faulty_blocks_unprovable: u64,
    pub(crate) missing_proposals: u64,
    pub(crate) equivocations: u64,
}

#[cfg(test)]
impl StorageScoringMetrics {
    pub(crate) fn new_v1_for_test(
        faulty_blocks_provable: u64,
        faulty_blocks_unprovable: u64,
        missing_proposals: u64,
        equivocations: u64,
    ) -> Self {
        Self::V1(StorageScoringMetricsV1 {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        })
    }
}
