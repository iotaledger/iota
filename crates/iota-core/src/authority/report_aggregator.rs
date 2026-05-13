// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use arc_swap::ArcSwapOption;
use iota_types::messages_consensus::{MisbehaviorObservations, VersionedMisbehaviorReport};

use crate::authority::authority_per_epoch_store::misbehavior::{
    MisbehaviorReportVersion, merge_max,
};

/// Reasons a peer-submitted report fails `validate_report`. Surfaced in the
/// caller's warn log so operators can distinguish wire-version drift from
/// malformed payloads without re-deriving the failure from generic logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReportValidationError {
    /// The report's wire-format variant doesn't match the
    /// `MisbehaviorReportVersion` this epoch is configured for (e.g. locally
    /// V1 but the report is V2).
    WrongReportVersion,
    /// The payload's per-metric vector lengths don't match the committee size
    /// (i.e. the report is malformed at the wire level).
    PayloadShape,
}

impl std::fmt::Display for ReportValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongReportVersion => {
                f.write_str("report wire-format version does not match the local report version")
            }
            Self::PayloadShape => f.write_str("report payload has wrong vector lengths"),
        }
    }
}

pub struct ReportAggregator {
    report_version: MisbehaviorReportVersion,
    received_reports_state: Vec<ReceivedReportsStatePerAuthority>,
}

impl ReportAggregator {
    pub fn new(report_version: MisbehaviorReportVersion, committee_size: usize) -> Self {
        let received_reports_state = (0..committee_size)
            .map(|_| ReceivedReportsStatePerAuthority {
                received_metrics: ArcSwapOption::empty(),
                invalid_reports_count: AtomicU64::new(0),
            })
            .collect();

        Self {
            report_version,
            received_reports_state,
        }
    }

    /// Validates an incoming report: checks that the report version matches the
    /// expected version, and that the payload structure is correct for the
    /// committee size (derived from internal state, so caller and aggregator
    /// can't disagree).
    pub(crate) fn validate_report(
        &self,
        report: &VersionedMisbehaviorReport,
    ) -> Result<(), ReportValidationError> {
        if !self.report_version.accepts_report(report) {
            return Err(ReportValidationError::WrongReportVersion);
        }
        let committee_size = self.received_reports_state.len();
        match &report.payload {
            MisbehaviorObservations::V1(payload) => {
                if payload.verify(committee_size) {
                    Ok(())
                } else {
                    Err(ReportValidationError::PayloadShape)
                }
            }
        }
    }

    /// Processes a validated report from a peer: performs a monotone merge
    /// (element-wise max) with any previously received observations from the
    /// same authority.
    ///
    /// Uses `rcu` so the load + merge + store is atomic against concurrent
    /// callers; the closure may run more than once under contention.
    pub(crate) fn process_report(&self, authority: u32, report: &VersionedMisbehaviorReport) {
        let incoming = &report.payload;
        let state = &self.received_reports_state[authority as usize];
        state.received_metrics.rcu(|current| {
            Some(Arc::new(match current.as_deref() {
                Some(existing) => merge_max(existing, incoming),
                None => incoming.clone(),
            }))
        });
    }

    /// Increments the invalid report counter for the given authority.
    ///
    /// On this branch the counter is in-memory only and resets on restart.
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
        self.received_reports_state[authority_index as usize].to_serializable()
    }

    /// Returns the received counts paired with voting power for each authority
    /// that has submitted at least one report. Used by the `Scorer` to compute
    /// weighted medians.
    pub(crate) fn reporters_with_voting_power(
        &self,
        voting_power: &[u64],
    ) -> Vec<(Arc<MisbehaviorObservations>, u64)> {
        let mut reporters = Vec::new();
        for (state, &vp) in self.received_reports_state.iter().zip(voting_power) {
            if let Some(arc) = state.received_metrics.load_full() {
                reporters.push((arc, vp));
            }
        }
        reporters
    }
}

/// Tracks the live in-memory state of the received reports for a single
/// authority.
#[derive(Debug)]
pub(crate) struct ReceivedReportsStatePerAuthority {
    // The misbehavior counts received from the authority, i.e., the information
    // contained in the MisbehaviorReports received. `None` if the authority has
    // not yet sent a report in this epoch.
    received_metrics: ArcSwapOption<MisbehaviorObservations>,
    // The count of invalid reports received from the authority. Bumped by
    // `validate_report` / `verify_consensus_transaction` when a report fails
    // checks; validity must be evaluated deterministically since invalid
    // reports are not re-propagated.
    invalid_reports_count: AtomicU64,
}

impl ReceivedReportsStatePerAuthority {
    #[cfg(test)]
    pub fn invalid_reports_count_snapshot(&self) -> u64 {
        self.invalid_reports_count.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub fn received_metrics_snapshot(&self) -> Option<MisbehaviorObservations> {
        self.received_metrics.load().as_deref().cloned()
    }

    #[cfg(test)]
    pub fn to_serializable(&self) -> DBReceivedReportsStatePerAuthority {
        DBReceivedReportsStatePerAuthority {
            received_metrics: self.received_metrics_snapshot(),
            invalid_reports_count: self.invalid_reports_count_snapshot(),
        }
    }
}

/// Serializable snapshot of a single authority's received-reports state.
/// Scaffolding for the storage layer of `ReportAggregator`: a future PR will
/// persist these records via `DBMap<u32, DBReceivedReportsStatePerAuthority>`
/// in `AuthorityEpochTables`, enabling report state to survive restarts.
/// Until then, this struct is only used in tests.
#[cfg(test)]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub(crate) struct DBReceivedReportsStatePerAuthority {
    pub received_metrics: Option<MisbehaviorObservations>,
    pub invalid_reports_count: u64,
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::ProtocolConfig;
    use iota_types::messages_consensus::{
        MisbehaviorObservations, MisbehaviorObservationsV1, VersionedMisbehaviorReport,
    };

    use crate::authority::authority_per_epoch_store::{
        misbehavior::MisbehaviorReportVersion,
        report_aggregator::{
            DBReceivedReportsStatePerAuthority, ReportAggregator, ReportValidationError,
        },
    };

    fn mock_protocol_config() -> ProtocolConfig {
        ProtocolConfig::get_for_max_version_UNSAFE()
    }

    fn mock_report_version() -> MisbehaviorReportVersion {
        MisbehaviorReportVersion::from_protocol(&mock_protocol_config())
    }

    fn mock_aggregator(committee_size: usize) -> ReportAggregator {
        ReportAggregator::new(mock_report_version(), committee_size)
    }

    fn report_v1(raw_counts: &[Vec<u64>; 4]) -> VersionedMisbehaviorReport {
        VersionedMisbehaviorReport::new_v1(
            iota_types::base_types::AuthorityName::default(),
            0,
            MisbehaviorObservationsV1 {
                faulty_blocks_provable: raw_counts[0].clone(),
                faulty_blocks_unprovable: raw_counts[1].clone(),
                missing_proposals: raw_counts[2].clone(),
                equivocations: raw_counts[3].clone(),
            },
        )
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
            Some(MisbehaviorObservations::V1(MisbehaviorObservationsV1 {
                faulty_blocks_provable: vec![1, 2, 3],
                faulty_blocks_unprovable: vec![4, 5, 6],
                missing_proposals: vec![7, 8, 9],
                equivocations: vec![0, 0, 0],
            }))
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
            Some(MisbehaviorObservations::V1(MisbehaviorObservationsV1 {
                faulty_blocks_provable: vec![3, 5, 10],
                faulty_blocks_unprovable: vec![4, 10, 6],
                missing_proposals: vec![7, 8, 9],
                equivocations: vec![1, 0, 0],
            }))
        );
    }

    #[test]
    fn test_validate_report_valid() {
        let aggregator = mock_aggregator(3);
        let report = report_v1(&[vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9], vec![0, 0, 0]]);
        assert!(aggregator.validate_report(&report).is_ok());
    }

    #[test]
    fn test_validate_report_wrong_committee_size() {
        // Aggregator built for a 3-validator committee; incoming report has
        // 4-element vectors — should be rejected as malformed.
        let aggregator = mock_aggregator(3);
        let report = report_v1(&[
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            vec![9, 10, 11, 12],
            vec![0, 0, 0, 0],
        ]);
        assert_eq!(
            aggregator.validate_report(&report),
            Err(ReportValidationError::PayloadShape)
        );
    }
}
