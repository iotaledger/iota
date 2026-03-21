// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use arc_swap::ArcSwapOption;
use iota_types::messages_consensus::VersionedMisbehaviorReport;
use serde::{Deserialize, Serialize};

use crate::authority::authority_per_epoch_store::misbehavior_config::{
    MisbehaviorConfig, MisbehaviorCounts, verify_legacy_payload,
};

pub struct ReportAggregator {
    config: MisbehaviorConfig,
    received_reports_state: ReceivedReportsState,
}

impl ReportAggregator {
    pub fn new(config: &MisbehaviorConfig, committee_size: usize) -> Self {
        let received_reports_state = (0..committee_size)
            .map(|_| ReceivedReportsStatePerAuthority {
                received_metrics: ArcSwapOption::empty(),
                invalid_reports_count: AtomicU64::new(0),
            })
            .collect::<ReceivedReportsState>();

        Self {
            config: config.clone(),
            received_reports_state,
        }
    }

    /// Validates an incoming report: checks that the report version matches the
    /// expected version, and that the payload structure is correct for the
    /// committee size.
    pub(crate) fn validate_report(
        &self,
        report: &VersionedMisbehaviorReport,
        committee_size: usize,
    ) -> bool {
        if !self.config.accepts_report(report) {
            return false;
        }
        match report {
            VersionedMisbehaviorReport::V1(payload, _) => {
                verify_legacy_payload(payload, committee_size)
            }
        }
    }

    /// Processes a validated report from a peer: converts it to
    /// `MisbehaviorCounts` and performs a monotone merge (element-wise max)
    /// with any previously received counts from the same authority.
    pub(crate) fn process_report(&self, authority: u32, report: &VersionedMisbehaviorReport) {
        let incoming_counts = MisbehaviorCounts::from(report);
        let state = &self.received_reports_state[authority as usize];
        let current = state.received_metrics.load();
        let updated = match current.as_deref() {
            Some(counts) => counts.merge_max(&incoming_counts),
            None => incoming_counts,
        };
        state.received_metrics.store(Some(Arc::new(updated)));
    }

    /// Increments the invalid report counter for the given authority.
    pub(crate) fn increment_invalid_reports_count(&self, authority: u32) {
        self.received_reports_state[authority as usize]
            .invalid_reports_count
            .fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) fn received_reports_state_per_authority_snapshot(
        &self,
        authority_index: u32,
    ) -> DBReceivedReportsStatePerAuthority {
        self.received_reports_state.0[authority_index as usize].to_serializable()
    }

    /// Returns the received counts paired with voting power for each authority
    /// that has submitted at least one report. Used by the `Scorer` to compute
    /// weighted medians.
    pub(crate) fn reporters_with_voting_power(
        &self,
        voting_power: &[u64],
    ) -> Vec<(Arc<MisbehaviorCounts>, u64)> {
        self.received_reports_state
            .0
            .iter()
            .zip(voting_power.iter())
            .filter_map(|(state, &vp)| {
                let guard = state.received_metrics.load();
                guard.as_ref().map(|arc| (Arc::clone(arc), vp))
            })
            .collect()
    }
}

pub(crate) struct ReceivedReportsState(Vec<ReceivedReportsStatePerAuthority>);

impl std::ops::Index<usize> for ReceivedReportsState {
    type Output = ReceivedReportsStatePerAuthority;
    fn index(&self, authority: usize) -> &Self::Output {
        &self.0[authority]
    }
}

impl FromIterator<ReceivedReportsStatePerAuthority> for ReceivedReportsState {
    fn from_iter<T: IntoIterator<Item = ReceivedReportsStatePerAuthority>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Tracks the live in-memory state of the received reports for a single
/// authority. Not serialized directly — use `to_serializable()` to produce a
/// `DBReceivedReportsStatePerAuthority` snapshot for DB storage.
#[derive(Debug)]
pub(crate) struct ReceivedReportsStatePerAuthority {
    // The misbehavior counts received from the authority, i.e., the information
    // contained in the MisbehaviorReports received. `None` if the authority has
    // not yet sent a report in this epoch.
    received_metrics: ArcSwapOption<MisbehaviorCounts>,
    // The count of invalid reports received from the authority. Validity must be
    // checked deterministically, since invalid reports are not re-propagated.
    invalid_reports_count: AtomicU64,
}

impl ReceivedReportsStatePerAuthority {
    #[cfg(test)]
    pub fn invalid_reports_count_snapshot(&self) -> u64 {
        self.invalid_reports_count.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub fn received_metrics_snapshot(&self) -> Option<MisbehaviorCounts> {
        self.received_metrics.load().as_deref().cloned()
    }

    #[cfg(test)]
    pub fn to_serializable(&self) -> DBReceivedReportsStatePerAuthority {
        DBReceivedReportsStatePerAuthority {
            received_metrics: self.received_metrics_snapshot(),
            invalid_reports_count: self.invalid_reports_count_snapshot(),
        }
    }

    #[expect(dead_code)]
    pub fn update_from_serializable(&self, serializable: DBReceivedReportsStatePerAuthority) {
        self.received_metrics
            .store(serializable.received_metrics.map(Arc::new));
        self.invalid_reports_count
            .store(serializable.invalid_reports_count, Ordering::Relaxed);
    }
}

/// DB storage record for a single authority's received reports state. Stored as
/// `DBMap<u32, DBReceivedReportsStatePerAuthority>`. Only authorities that have
/// sent at least one report have an entry (i.e., `received_metrics` is `Some`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct DBReceivedReportsStatePerAuthority {
    pub received_metrics: Option<MisbehaviorCounts>,
    pub invalid_reports_count: u64,
}
