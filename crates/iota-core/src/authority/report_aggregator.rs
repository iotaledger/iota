// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use arc_swap::ArcSwapOption;
use iota_types::messages_consensus::{MisbehaviorObservations, VersionedMisbehaviorReport};
use typed_store::Map;

use crate::{
    authority::authority_per_epoch_store::misbehavior::{MisbehaviorReportVersion, merge_max},
    consensus_types::AuthorityIndex,
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
    /// same authority. Returns the **post-merge** snapshot for the touched
    /// authority — callers persist this verbatim against the active
    /// `ConsensusCommitOutput`, so the on-disk row for commit N is exactly
    /// the aggregator state produced by commit N's processing.
    ///
    /// Uses `rcu` so the load + merge + store is atomic against concurrent
    /// callers; the closure may run more than once under contention.
    pub(crate) fn process_report(
        &self,
        authority: AuthorityIndex,
        report: &VersionedMisbehaviorReport,
    ) -> DBReceivedReportsStatePerAuthority {
        let incoming = &report.payload;
        let state = &self.received_reports_state[authority as usize];
        state.received_metrics.rcu(|current| {
            Some(Arc::new(match current.as_deref() {
                Some(existing) => merge_max(existing, incoming),
                None => incoming.clone(),
            }))
        });
        state.to_serializable()
    }

    /// Repopulates the in-memory aggregator from persisted rows. Authorities
    /// without a row keep their default (empty) state. Returns an error if a
    /// row's key is out of range for the configured committee size — that
    /// would indicate cross-epoch table contamination.
    pub(crate) fn restore_from_iter(
        &self,
        rows: impl Iterator<Item = (AuthorityIndex, DBReceivedReportsStatePerAuthority)>,
    ) -> iota_types::error::IotaResult<()> {
        let committee_size = self.received_reports_state.len();
        for (authority_index, db_state) in rows {
            let idx = authority_index as usize;
            if idx >= committee_size {
                return Err(iota_types::error::IotaError::Storage(format!(
                    "received_reports_state row for authority {authority_index} \
                     is out of range for committee size {committee_size}"
                )));
            }
            let slot = &self.received_reports_state[idx];
            if let Some(observations) = db_state.received_metrics {
                slot.received_metrics.store(Some(Arc::new(observations)));
            }
            slot.invalid_reports_count
                .store(db_state.invalid_reports_count, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Production restore: streams every row of
    /// `AuthorityEpochTables::received_reports_state` into the aggregator.
    /// Widens the on-disk `u8` key to the in-memory `AuthorityIndex` at this
    /// storage boundary.
    pub(crate) fn restore_from_tables(
        &self,
        tables: &super::AuthorityEpochTables,
    ) -> iota_types::error::IotaResult<()> {
        let rows = tables
            .received_reports_state
            .safe_iter()
            .map(|res| res.map(|(idx, state)| (AuthorityIndex::from(idx), state)))
            .collect::<Result<Vec<_>, _>>()?;
        self.restore_from_iter(rows.into_iter())
    }

    /// Increments the invalid report counter for the given authority.
    ///
    /// Called from `verify_consensus_transaction`, which runs outside the
    /// per-commit `ConsensusCommitOutput` scope. The bump updates the
    /// in-memory `AtomicU64` correctly, but the persisted row is only
    /// refreshed when a subsequent `process_report` for the same authority
    /// captures a fresh snapshot. The counter is observability-only, so
    /// losing isolated bumps across crashes is acceptable.
    pub(crate) fn increment_invalid_reports_count(&self, authority: AuthorityIndex) {
        self.received_reports_state[authority as usize]
            .invalid_reports_count
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Per-authority snapshot of the cumulative invalid-report counts, indexed
    /// by consensus `AuthorityIndex`. Used by the consensus handler to publish
    /// the corresponding Prometheus gauge with hostname labels.
    pub(crate) fn invalid_reports_counts(&self) -> Vec<u64> {
        self.received_reports_state
            .iter()
            .map(|s| s.invalid_reports_count.load(Ordering::Relaxed))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn received_reports_state_per_authority_snapshot(
        &self,
        authority_index: AuthorityIndex,
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
    pub fn to_serializable(&self) -> DBReceivedReportsStatePerAuthority {
        DBReceivedReportsStatePerAuthority {
            received_metrics: self.received_metrics.load().as_deref().cloned(),
            invalid_reports_count: self.invalid_reports_count.load(Ordering::Relaxed),
        }
    }
}

/// Serializable snapshot of a single authority's received-reports state.
/// Persisted to `AuthorityEpochTables::received_reports_state` so the
/// aggregator survives restarts.
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

    use crate::{
        authority::authority_per_epoch_store::{
            misbehavior::MisbehaviorReportVersion,
            report_aggregator::{
                DBReceivedReportsStatePerAuthority, ReportAggregator, ReportValidationError,
            },
        },
        consensus_types::AuthorityIndex,
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
    fn test_restore_from_iter_populates_in_memory_state() {
        let aggregator = mock_aggregator(3);

        // Simulate restored DB rows: authority 0 has observations, authority 2
        // has only an invalid-count bump, authority 1 has no row at all.
        let rows: Vec<(AuthorityIndex, DBReceivedReportsStatePerAuthority)> = vec![
            (
                0,
                DBReceivedReportsStatePerAuthority {
                    received_metrics: Some(MisbehaviorObservations::V1(
                        MisbehaviorObservationsV1 {
                            faulty_blocks_provable: vec![1, 2, 3],
                            faulty_blocks_unprovable: vec![0, 0, 0],
                            missing_proposals: vec![0, 0, 0],
                            equivocations: vec![0, 0, 0],
                        },
                    )),
                    invalid_reports_count: 5,
                },
            ),
            (
                2,
                DBReceivedReportsStatePerAuthority {
                    received_metrics: None,
                    invalid_reports_count: 9,
                },
            ),
        ];

        aggregator
            .restore_from_iter(rows.into_iter())
            .expect("restore");

        let snapshot = full_snapshot(&aggregator, 3);
        assert_eq!(snapshot[0].invalid_reports_count, 5);
        assert!(snapshot[0].received_metrics.is_some());
        assert_eq!(snapshot[1], empty_state());
        assert_eq!(snapshot[2].invalid_reports_count, 9);
        assert!(snapshot[2].received_metrics.is_none());
    }

    #[test]
    fn test_restore_from_iter_rejects_out_of_range_authority() {
        let aggregator = mock_aggregator(3);
        let rows: Vec<(AuthorityIndex, DBReceivedReportsStatePerAuthority)> = vec![(
            7, // committee size is 3, so 7 is out of range
            DBReceivedReportsStatePerAuthority {
                received_metrics: None,
                invalid_reports_count: 1,
            },
        )];
        assert!(aggregator.restore_from_iter(rows.into_iter()).is_err());
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

    #[tokio::test]
    async fn test_restore_round_trip_through_dbmap() {
        use iota_types::base_types::EpochId;

        use crate::authority::authority_per_epoch_store::AuthorityEpochTables;

        let tempdir = tempfile::tempdir().unwrap();
        let epoch: EpochId = 0;
        let tables = AuthorityEpochTables::open(epoch, tempdir.path(), None);

        // Original aggregator: process a report for authority 0 and bump
        // invalid-reports count for authority 2.
        let original = mock_aggregator(3);
        let report = report_v1(&[vec![1, 2, 3], vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]]);
        original.process_report(0, &report);
        original.increment_invalid_reports_count(2);

        // Write what `ConsensusCommitOutput::write_to_batch` would write for
        // dirty authorities {0, 2}: snapshot current state, insert into DBMap.
        // DBMap key is `u8`; truncate at this storage boundary.
        let dirty: std::collections::BTreeSet<AuthorityIndex> = [0, 2].into_iter().collect();
        let mut batch = tables.received_reports_state.batch();
        let rows: Vec<(u8, DBReceivedReportsStatePerAuthority)> = dirty
            .iter()
            .map(|&i| {
                (
                    i as u8,
                    original.received_reports_state_per_authority_snapshot(i),
                )
            })
            .collect();
        batch
            .insert_batch(&tables.received_reports_state, rows)
            .unwrap();
        batch.write().unwrap();

        // Restore into a fresh aggregator from the same tables.
        let restored = mock_aggregator(3);
        restored.restore_from_tables(&tables).unwrap();

        let snap = full_snapshot(&restored, 3);
        assert!(snap[0].received_metrics.is_some());
        assert_eq!(snap[0].invalid_reports_count, 0);
        assert_eq!(snap[1], empty_state());
        assert!(snap[2].received_metrics.is_none());
        assert_eq!(snap[2].invalid_reports_count, 1);
    }
}
