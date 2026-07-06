// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Deterministic per-attestor attestation statistics for one epoch.
//!
//! One [`AttestorStats`] entry per epoch-start attestor, indexed by the
//! per-epoch dense attestor index (position in the epoch's `AttestorSet`).
//! Recording must only happen from consensus-commit-ordered processing so
//! every validator accumulates identical state. Per-commit snapshot
//! buffering and an atomic flush to the epoch table will land with the
//! verification project; until then the aggregator restores whatever the
//! table holds on epoch-store construction.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use typed_store::Map;

/// Attestation statistics for one attestor in one epoch.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AttestorStats {
    pub valid_count: u64,
    pub invalid_count: u64,
    pub valid_computation_units: u64,
    pub invalid_computation_units: u64,
}

/// Accumulates [`AttestorStats`] per epoch-start attestor.
pub struct AttestorStatsAggregator {
    stats: Mutex<Vec<AttestorStats>>,
}

impl AttestorStatsAggregator {
    pub fn new(size: usize) -> Self {
        Self {
            stats: Mutex::new(vec![AttestorStats::default(); size]),
        }
    }

    /// Record one valid attestation for the attestor at `attestor_index`,
    /// returning the post-update snapshot for commit buffering. Out-of-range
    /// indices are ignored (`None`) so a caller bug cannot diverge state.
    pub fn record_valid_attestation(
        &self,
        attestor_index: u32,
        computation_units: u64,
    ) -> Option<AttestorStats> {
        let mut stats = self.stats.lock().unwrap();
        let entry = stats.get_mut(attestor_index as usize)?;
        entry.valid_count = entry.valid_count.saturating_add(1);
        entry.valid_computation_units = entry
            .valid_computation_units
            .saturating_add(computation_units);
        Some(entry.clone())
    }

    /// Record one invalid attestation for the attestor at `attestor_index`,
    /// returning the post-update snapshot for commit buffering. Out-of-range
    /// indices are ignored (`None`).
    pub fn record_invalid_attestation(
        &self,
        attestor_index: u32,
        computation_units: u64,
    ) -> Option<AttestorStats> {
        let mut stats = self.stats.lock().unwrap();
        let entry = stats.get_mut(attestor_index as usize)?;
        entry.invalid_count = entry.invalid_count.saturating_add(1);
        entry.invalid_computation_units = entry
            .invalid_computation_units
            .saturating_add(computation_units);
        Some(entry.clone())
    }

    /// Snapshot of all entries, ordered by dense attestor index.
    pub fn current_stats(&self) -> Vec<AttestorStats> {
        self.stats.lock().unwrap().clone()
    }

    pub(crate) fn restore_from_iter(&self, rows: impl Iterator<Item = (u32, AttestorStats)>) {
        let mut stats = self.stats.lock().unwrap();
        for (idx, entry) in rows {
            if let Some(slot) = stats.get_mut(idx as usize) {
                *slot = entry;
            }
        }
    }

    /// Production restore: streams every row of
    /// `AuthorityEpochTables::attestor_stats` into the aggregator.
    pub(crate) fn restore_from_tables(
        &self,
        tables: &super::AuthorityEpochTables,
    ) -> iota_types::error::IotaResult<()> {
        let rows = tables
            .attestor_stats
            .safe_iter()
            .collect::<Result<Vec<_>, _>>()?;
        self.restore_from_iter(rows.into_iter());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_accumulates_per_validity() {
        let agg = AttestorStatsAggregator::new(2);
        assert!(agg.record_valid_attestation(0, 10).is_some());
        assert!(agg.record_valid_attestation(0, 5).is_some());
        assert!(agg.record_invalid_attestation(0, 7).is_some());
        assert!(agg.record_invalid_attestation(1, 3).is_some());
        let stats = agg.current_stats();
        assert_eq!(
            stats[0],
            AttestorStats {
                valid_count: 2,
                invalid_count: 1,
                valid_computation_units: 15,
                invalid_computation_units: 7,
            }
        );
        assert_eq!(
            stats[1],
            AttestorStats {
                valid_count: 0,
                invalid_count: 1,
                valid_computation_units: 0,
                invalid_computation_units: 3,
            }
        );
    }

    #[test]
    fn out_of_range_index_is_ignored() {
        let agg = AttestorStatsAggregator::new(1);
        assert!(agg.record_valid_attestation(1, 10).is_none());
        assert!(agg.record_invalid_attestation(7, 10).is_none());
        assert_eq!(agg.current_stats(), vec![AttestorStats::default()]);
    }

    #[test]
    fn units_saturate_instead_of_overflowing() {
        let agg = AttestorStatsAggregator::new(1);
        agg.record_valid_attestation(0, u64::MAX).unwrap();
        let snap = agg.record_valid_attestation(0, u64::MAX).unwrap();
        assert_eq!(snap.valid_computation_units, u64::MAX);
        assert_eq!(snap.valid_count, 2);
    }

    #[test]
    fn restore_round_trip() {
        let agg = AttestorStatsAggregator::new(3);
        agg.record_valid_attestation(1, 42).unwrap();
        agg.record_invalid_attestation(2, 9).unwrap();
        let rows: Vec<(u32, AttestorStats)> = agg
            .current_stats()
            .into_iter()
            .enumerate()
            .map(|(i, s)| (i as u32, s))
            .collect();
        let restored = AttestorStatsAggregator::new(3);
        restored.restore_from_iter(rows.into_iter());
        assert_eq!(restored.current_stats(), agg.current_stats());
    }
}
