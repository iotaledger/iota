// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::AtomicU64;

use strum::EnumCount;

#[derive(Clone, PartialEq, EnumCount)]
#[repr(usize)]
#[expect(dead_code)]
pub enum StarfishMisbehavior {
    FaultyBlocksProvable = 0,
    FaultyBlocksUnprovable = 1,
    MissingProposals = 2,
    Equivocations = 3,
}

/// Struct that holds the scoring metrics for all authorities in the committee,
/// both cached and uncached. It also holds a shared reference to the current
/// local metrics count used by Scorer.
pub(crate) struct ScoringMetricsStore {
    #[expect(dead_code)]
    pub current_local_metrics_count: StarfishMisbehaviorCounts,
    #[expect(dead_code)]
    pub cached_metrics: StarfishMisbehaviorCounts,
    #[expect(dead_code)]
    pub uncached_metrics: StarfishMisbehaviorCounts,
}

impl ScoringMetricsStore {
    pub(crate) fn new(committee_size: usize) -> Self {
        let num_misbehaviors = StarfishMisbehavior::COUNT;
        Self {
            current_local_metrics_count: StarfishMisbehaviorCounts::new(
                committee_size,
                num_misbehaviors,
            ),
            cached_metrics: StarfishMisbehaviorCounts::new(committee_size, num_misbehaviors),
            uncached_metrics: StarfishMisbehaviorCounts::new(committee_size, num_misbehaviors),
        }
    }
}

pub(crate) struct StarfishMisbehaviorCounts(#[expect(dead_code)] pub(crate) Vec<Vec<AtomicU64>>);

impl StarfishMisbehaviorCounts {
    pub(crate) fn new(committee_size: usize, num_misbehaviors: usize) -> Self {
        // Local metrics count are always initialized as zero.
        Self(
            (0..num_misbehaviors)
                .map(|_| (0..committee_size).map(|_| AtomicU64::new(0)).collect())
                .collect(),
        )
    }
}
