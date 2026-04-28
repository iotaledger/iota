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
        let incoming_counts =
            MisbehaviorCounts::from_report(report, self.config.reported_misbehaviors());
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

#[cfg(test)]
mod tests {
    use iota_protocol_config::ProtocolConfig;
    use iota_types::messages_consensus::{LegacyReportPayload, VersionedMisbehaviorReport};

    use crate::authority::authority_per_epoch_store::{
        misbehavior_config::{MisbehaviorConfig, MisbehaviorCounts},
        report_aggregator::{DBReceivedReportsStatePerAuthority, ReportAggregator},
    };

    fn mock_protocol_config() -> ProtocolConfig {
        ProtocolConfig::get_for_max_version_UNSAFE()
    }

    fn mock_misbehavior_config() -> MisbehaviorConfig {
        MisbehaviorConfig::from_protocol(&mock_protocol_config())
    }

    fn mock_aggregator(committee_size: usize) -> ReportAggregator {
        ReportAggregator::new(&mock_misbehavior_config(), committee_size)
    }

    fn report_v1(raw_counts: &[Vec<u64>; 4]) -> VersionedMisbehaviorReport {
        VersionedMisbehaviorReport::new_v1(LegacyReportPayload {
            faulty_blocks_provable: raw_counts[0].clone(),
            faulty_blocks_unprovable: raw_counts[1].clone(),
            missing_proposals: raw_counts[2].clone(),
            equivocations: raw_counts[3].clone(),
        })
    }

    fn full_snapshot(
        aggregator: &ReportAggregator,
        committee_size: usize,
    ) -> Vec<DBReceivedReportsStatePerAuthority> {
        (0..committee_size as u32)
            .map(|i| aggregator.received_reports_state_per_authority_snapshot(i))
            .collect()
    }

    fn empty_state() -> DBReceivedReportsStatePerAuthority {
        DBReceivedReportsStatePerAuthority {
            received_metrics: None,
            invalid_reports_count: 0,
        }
    }

    #[test]
    fn test_aggregator_initialization() {
        let aggregator = mock_aggregator(3);
        assert_eq!(full_snapshot(&aggregator, 3), vec![empty_state(); 3]);
    }

    #[test]
    fn test_increment_invalid_reports_count() {
        let aggregator = mock_aggregator(3);

        aggregator.increment_invalid_reports_count(2);

        assert_eq!(full_snapshot(&aggregator, 3)[2].invalid_reports_count, 1);

        aggregator.increment_invalid_reports_count(1);
        aggregator.increment_invalid_reports_count(1);

        let snapshot = full_snapshot(&aggregator, 3);
        assert_eq!(snapshot[0].invalid_reports_count, 0);
        assert_eq!(snapshot[1].invalid_reports_count, 2);
        assert_eq!(snapshot[2].invalid_reports_count, 1);
    }

    #[test]
    fn test_process_report_single() {
        let aggregator = mock_aggregator(3);

        let report = report_v1(&[vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9], vec![0, 0, 0]]);
        aggregator.process_report(0, &report);

        let snapshot = full_snapshot(&aggregator, 3);
        assert_eq!(
            snapshot[0].received_metrics,
            Some(MisbehaviorCounts(vec![
                vec![1, 2, 3],
                vec![4, 5, 6],
                vec![7, 8, 9],
                vec![0, 0, 0],
            ]))
        );
        assert!(snapshot[1].received_metrics.is_none());
        assert!(snapshot[2].received_metrics.is_none());
    }

    #[test]
    fn test_process_report_monotone_merge() {
        let aggregator = mock_aggregator(3);

        // First report
        let report1 = report_v1(&[vec![1, 5, 3], vec![4, 5, 6], vec![7, 8, 9], vec![0, 0, 0]]);
        aggregator.process_report(0, &report1);

        // Second report from same authority with some higher, some lower values
        let report2 = report_v1(&[vec![3, 2, 10], vec![1, 10, 6], vec![7, 8, 9], vec![1, 0, 0]]);
        aggregator.process_report(0, &report2);

        // Should be element-wise max
        assert_eq!(
            full_snapshot(&aggregator, 3)[0].received_metrics,
            Some(MisbehaviorCounts(vec![
                vec![3, 5, 10],
                vec![4, 10, 6],
                vec![7, 8, 9],
                vec![1, 0, 0],
            ]))
        );
    }

    #[test]
    fn test_validate_report_valid() {
        let aggregator = mock_aggregator(3);
        let report = report_v1(&[vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9], vec![0, 0, 0]]);
        assert!(aggregator.validate_report(&report, 3));
    }

    #[test]
    fn test_validate_report_wrong_committee_size() {
        let aggregator = mock_aggregator(3);
        // Report has 3 entries per metric but we validate against committee_size=4
        let report = report_v1(&[vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9], vec![0, 0, 0]]);
        assert!(!aggregator.validate_report(&report, 4));
    }
}
