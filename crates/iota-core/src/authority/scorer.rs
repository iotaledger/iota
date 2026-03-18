// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use arc_swap::ArcSwap;
use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::VersionedMisbehaviorReport;
use serde::{Deserialize, Serialize};

use crate::authority::authority_per_epoch_store::misbehavior_monitor::{
    MisbehaviorCounts, MisbehaviorMonitor, ReportedMisbehaviors,
};

/// Must match MAX_SCORE in validator_set.move in iota-framework.
pub(crate) const MAX_SCORE: u64 = u16::MAX as u64 + 1;
/// Fixed-point scale used when combining weighted minor scores before dividing
/// back down to [0, MAX_SCORE]. Chosen as 2^16 so that MAX_SCORE * SCALE_FACTOR
/// fits in a u64 without overflow.
const SCALE_FACTOR: u64 = 2_u64.pow(16);

pub struct ReceivedReportsState(Vec<ReceivedReportsStatePerAuthority>);

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

impl ReceivedReportsState {
    pub fn serializable_snapshot(&self) -> Vec<DBReceivedReportsStatePerAuthority> {
        self.0.iter().map(|state| state.to_serializable()).collect()
    }
}

// Tracks the live in-memory state of the received reports for a single
// authority. Not serialized directly — use `to_serializable()` to produce a
// `DBReceivedReportsStatePerAuthority` snapshot for DB storage.
#[derive(Debug)]
pub struct ReceivedReportsStatePerAuthority {
    // The misbehavior counts received from the authority, i.e., the information
    // contained in the MisbehaviorReports received. `None` if the authority has
    // not yet sent a report in this epoch.
    received_metrics: ArcSwap<Option<MisbehaviorCounts>>,
    // The count of invalid reports received from the authority. Validity must be
    // checked deterministically, since invalid reports are not re-propagated.
    invalid_reports_count: AtomicU64,
}

impl ReceivedReportsStatePerAuthority {
    pub fn invalid_reports_count_snapshot(&self) -> u64 {
        self.invalid_reports_count.load(Ordering::Relaxed)
    }

    pub fn received_metrics_snapshot(&self) -> Option<MisbehaviorCounts> {
        self.received_metrics.load().as_ref().clone()
    }

    pub fn to_serializable(&self) -> DBReceivedReportsStatePerAuthority {
        DBReceivedReportsStatePerAuthority {
            received_metrics: self.received_metrics_snapshot(),
            invalid_reports_count: self.invalid_reports_count_snapshot(),
        }
    }
}

/// DB storage record for a single authority's received reports state. Stored as
/// `DBMap<u32, ReceivedReportsStateRecord>`. Only authorities that have sent at
/// least one report have an entry (i.e., `received_metrics` is `Some`).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DBReceivedReportsStatePerAuthority {
    pub received_metrics: Option<MisbehaviorCounts>,
    pub invalid_reports_count: u64,
}

/// Holds all information related to scoring of authorities in the committee.
pub struct Scorer {
    // Per-authority state tracking received reports and their validity.
    received_reports_state: ReceivedReportsState,
    // The current scores of the authorities, updated after each received report. This score is
    // calculated based on the information in received_reports_state.
    current_scores: Vec<AtomicU64>,
    // The voting power of each authority in the committee.
    voting_power: Vec<u64>,
    // A summary of the last MisbehaviorReport sent by the authority in the checkpoint creation.
    // Since this particular report is meant to include misbehavior counts (instead of proofs), and
    // those counts are always non-decreasing, the summary can be created by adding all metrics for
    // all validators. This is used as part of the MisbehaviorReport rate limiting mechanism.
    last_report_summary: AtomicU64,
    // Indicates the sequence number of the last checkpoint for which the authority sent a report.
    // This is used as part of the MisbehaviorReport rate limiting mechanism.
    last_report_checkpoint_seq: AtomicU64,
    // Indicates whether the authority sent a report close to the epoch end.
    has_sent_end_of_epoch_report: AtomicBool,
    // The version of the scorer being used with its parameters.
    version: ScorerVersion,
}

impl Scorer {
    pub fn new(
        voting_power: Vec<u64>,
        protocol_config: &ProtocolConfig,
        misbehavior_monitor: &Arc<MisbehaviorMonitor>,
    ) -> Self {
        let committee_size = voting_power.len();
        let reported_misbehaviors = misbehavior_monitor.version.reported_misbehaviors();
        let version = Self::load_parameters(protocol_config, reported_misbehaviors);

        let received_reports_state = (0..committee_size)
            .map(|_| ReceivedReportsStatePerAuthority {
                received_metrics: ArcSwap::new(Arc::new(None)),
                invalid_reports_count: AtomicU64::new(0),
            })
            .collect::<ReceivedReportsState>();
        let current_scores: Vec<AtomicU64> = (0..committee_size)
            .map(|_| AtomicU64::new(MAX_SCORE))
            .collect();

        Self {
            received_reports_state,
            current_scores,
            voting_power,
            last_report_summary: AtomicU64::new(0),
            last_report_checkpoint_seq: AtomicU64::new(0),
            has_sent_end_of_epoch_report: AtomicBool::new(false),
            version,
        }
    }

    fn load_parameters(
        protocol_config: &ProtocolConfig,
        reported_misbehaviors: &ReportedMisbehaviors,
    ) -> ScorerVersion {
        match (
            protocol_config.scorer_version_as_option(),
            reported_misbehaviors,
        ) {
            (None, ReportedMisbehaviors::V1(misbehaviors))
            | (Some(1), ReportedMisbehaviors::V1(misbehaviors)) => {
                assert!(
                    misbehaviors.len() == 4,
                    "Scorer V1 requires exactly 4 misbehavior metrics"
                );

                let allowances = vec![
                    1, /* 1 provable faulty block is allowed without punishment, to account for
                        * potential honest mistakes and edge cases in the protocol. */
                    2, /* 2 unprovable faulty blocks are allowed without punishment, as they are
                        * less severe than provable ones and can also result from honest
                        * mistakes. */
                    48_000, /* roughly 3% of consensus rounds in an epoch, to allow for some
                             * minor issues without harsh penalties. */
                    0, /* no equivocation is allowed without punishment, as it is a severe
                        * misbehavior that should be penalized immediately. */
                ];

                let maximums = vec![
                    5, /* more than 5 provable faulty blocks lead to zero score, as this
                        * indicates a significant issue with the
                        * authority's behavior. */
                    10, /* more than 10 unprovable faulty blocks lead to zero score, as this
                         * also indicates a significant issue, even if
                         * less severe than provable faults. */
                    160_000, /* roughly 10% of consensus rounds in an epoch, to ensure that a
                              * very high number of minor issues
                              * also leads to penalties. */
                    1, // any equivocation leads to zero score, as it is a critical misbehavior.
                ];

                let weights = [
                    SCALE_FACTOR * 30 / 100, /* provable faulty blocks contribute to 30% of the
                                              * score reduction, as they are the most severe
                                              * type of minor misbehavior. */
                    SCALE_FACTOR * 10 / 100, /* unprovable faulty blocks contribute to 10% of
                                              * the
                                              * score reduction, as they are less severe than
                                              * provable ones. */
                    SCALE_FACTOR * 35 / 100, /* missing proposals contribute to 35% of the score
                                              * reduction, as they can indicate issues with
                                              * availability or responsiveness. */
                    1, /* equivocations contribute multiplicatively to the final score, as
                        * they are a major misbehavior that should lead to a zero score if
                        * present. */
                ];

                let is_major = [false, false, false, true];

                // Assert that the allowance for major misbehaviors is 0,
                // maximum is 1 and weight is 1. This is because major misbehaviors should
                // reduce the score to 0 if there are any occurrences.
                assert!(is_major.iter().enumerate().all(|(i, &major)| {
                    !major || (allowances[i] == 0 && maximums[i] == 1 && weights[i] == 1)
                }));
                // Assert that allowances are compatible with the maximums for all metrics.
                assert!(allowances.iter().zip(maximums.iter()).all(|(a, m)| a < m));
                // Assert that maximums are compatible with MAX_SCORE for all metrics, to
                // prevent overflows.
                assert!(maximums.iter().all(|&a| a <= u64::MAX / MAX_SCORE));

                // Precompute minor/major indices and weights.
                let minor_indices: Vec<usize> = is_major
                    .iter()
                    .enumerate()
                    .filter(|(_, &major)| !major)
                    .map(|(i, _)| i)
                    .collect();
                let minor_weights: Vec<u64> = minor_indices.iter().map(|&i| weights[i]).collect();
                let major_indices: Vec<usize> = is_major
                    .iter()
                    .enumerate()
                    .filter(|(_, &major)| major)
                    .map(|(i, _)| i)
                    .collect();
                // Assert that the sum of minor weights does not exceed SCALE_FACTOR, to
                // prevent a u64 underflow when computing the baseline score.
                let minor_weights_sum = minor_weights.iter().sum::<u64>();
                assert!(
                    minor_weights_sum <= SCALE_FACTOR,
                    "minor weights sum ({minor_weights_sum}) exceeds SCALE_FACTOR ({SCALE_FACTOR})"
                );
                let baseline_score = SCALE_FACTOR - minor_weights_sum;

                ScorerVersion::V1(Parameters {
                    allowances,
                    maximums,
                    minor_indices,
                    minor_weights,
                    major_indices,
                    baseline_score,
                })
            }
            _ => panic!("Unsupported scorer version"),
        }
    }

    fn get_parameters(&self) -> &Parameters {
        match &self.version {
            ScorerVersion::V1(params) => params,
        }
    }

    // Boundary checks are done at a higher level. `authority` must be derived
    // from a valid AuthorityIndex.
    pub(crate) fn increment_invalid_reports_count(&self, authority: u32) {
        self.received_reports_state[authority as usize]
            .invalid_reports_count
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn update_scores(&self) {
        match self.version {
            ScorerVersion::V1(_) => self.update_scores_v1(),
        };
    }

    pub(crate) fn update_received_reports(
        &self,
        authority: u32,
        report: &VersionedMisbehaviorReport,
    ) {
        let state = &self.received_reports_state[authority as usize];
        let current = state.received_metrics.load();
        let updated = match current.as_ref().as_ref() {
            Some(counts) => counts.get_updated_from_report(report),
            None => MisbehaviorCounts::from(report),
        };
        state.received_metrics.swap(Arc::new(Some(updated)));
    }

    pub(crate) fn last_report_checkpoint_seq(&self) -> u64 {
        self.last_report_checkpoint_seq.load(Ordering::Relaxed)
    }

    pub(crate) fn store_last_report_checkpoint_seq(&self, checkpoint_seq: u64) {
        self.last_report_checkpoint_seq
            .store(checkpoint_seq, Ordering::Relaxed)
    }

    pub(crate) fn last_report_summary(&self) -> u64 {
        self.last_report_summary.load(Ordering::Relaxed)
    }

    pub(crate) fn store_last_report_summary(&self, summary: u64) {
        self.last_report_summary.store(summary, Ordering::Relaxed)
    }

    pub(crate) fn has_sent_end_of_epoch_report(&self) -> bool {
        self.has_sent_end_of_epoch_report.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_end_of_epoch_report_sent(&self) {
        self.has_sent_end_of_epoch_report
            .store(true, Ordering::Relaxed);
    }

    pub(crate) fn current_scores(&self) -> Vec<u64> {
        self.current_scores
            .iter()
            .map(|x| x.load(Ordering::Relaxed))
            .collect()
    }
}

impl Scorer {
    /// Calculates the weighted median across all reporters for each metric and
    /// authority. Returns `None` if no authority has sent a report yet.
    fn calculate_median_report(&self) -> Option<MisbehaviorCounts> {
        let misbehavior_count = self.get_parameters().allowances.len();
        let reporters: Vec<(MisbehaviorCounts, VotingPower)> = self
            .received_reports_state
            .0
            .iter()
            .zip(self.voting_power.iter())
            .filter_map(|(state, &vp)| {
                state
                    .received_metrics
                    .load()
                    .as_ref()
                    .as_ref()
                    .map(|counts| (counts.clone(), vp))
            })
            .collect();

        if reporters.is_empty() {
            return None;
        }

        let committee_size = self.voting_power.len();
        // Sum only over reporters, not the full committee — the median is weighted
        // by the voting power of authorities that actually submitted a report.
        let total_voting_power: VotingPower = reporters.iter().map(|(_, vp)| *vp).sum();

        // Reused across all (metric, authority) pairs — one allocation total.
        let mut chunk: Vec<(u64, VotingPower)> = Vec::with_capacity(reporters.len());
        let mut medians = Vec::with_capacity(misbehavior_count);

        for metric_index in 0..misbehavior_count {
            let mut median_for_metric = Vec::with_capacity(committee_size);
            for authority in 0..committee_size {
                chunk.clear();
                chunk.extend(
                    reporters
                        .iter()
                        .map(|(counts, vp)| (counts.get_value(metric_index, authority), *vp)),
                );
                chunk.sort_unstable_by_key(|&(val, _)| val);

                let mut accumulated = 0;
                for &(val, vp) in &chunk {
                    accumulated += vp;
                    if accumulated * 2 >= total_voting_power {
                        median_for_metric.push(val);
                        break;
                    }
                }
            }
            debug_assert_eq!(
                median_for_metric.len(),
                committee_size,
                "weighted median did not produce a value for every authority; \
                 this is a bug — accumulated voting power must always reach total"
            );
            medians.push(median_for_metric);
        }

        Some(MisbehaviorCounts(medians))
    }

    /// Given the median reports for all metrics, calculate the final scores. A
    /// score is an integer between 0 and max_score. For each metric, we have an
    /// allowance (allowed misbehaviors without any punishment) and a maximum
    /// (number of misbehaviors that lead to zero score). Each individual score
    /// for minor misbehaviors (non-equivocation) is also an integer between 0
    /// and max_score, and the weights are such that
    /// `sum(weights) + baseline_score = scale_factor`. Thus we need
    /// `max_score * scale_factor < 2^64` to avoid overflows.
    /// Major misbehaviors (equivocations) multiplicatively impact the final
    /// score — their contribution is either 0 or 1.
    fn calculate_scores_v1(&self, median_counts: MisbehaviorCounts) -> Vec<u64> {
        let parameters = self.get_parameters();
        let committee_size = self.voting_power.len();

        // Initialise with the baseline; values are in [0, MAX_SCORE * SCALE_FACTOR].
        let mut final_scores = vec![parameters.baseline_score * MAX_SCORE; committee_size];

        // Accumulate weighted minor misbehavior scores directly into final_scores.
        for (&i, &weight) in parameters
            .minor_indices
            .iter()
            .zip(parameters.minor_weights.iter())
        {
            for (authority, &count) in median_counts.get_metric(i).iter().enumerate() {
                final_scores[authority] += metric_to_score(
                    count,
                    parameters.allowances[i],
                    parameters.maximums[i],
                    MAX_SCORE,
                ) * weight;
            }
        }

        // Scale down to [0, MAX_SCORE].
        for score in final_scores.iter_mut() {
            *score /= SCALE_FACTOR;
        }

        // Multiply by each major misbehavior score (0 or 1).
        for &i in &parameters.major_indices {
            for (authority, score) in final_scores.iter_mut().enumerate() {
                *score *= metric_to_score(
                    median_counts.get_metric(i)[authority],
                    parameters.allowances[i],
                    parameters.maximums[i],
                    1,
                );
            }
        }

        final_scores
    }

    fn update_scores_v1(&self) {
        if let Some(median_counts) = self.calculate_median_report() {
            let scores = self.calculate_scores_v1(median_counts);
            // Relaxed: current_scores is read from the checkpoint service thread, but
            // each score is an independent value with no causality chain between entries.
            // Reading a mix of old and new scores is no worse than reading at two
            // different instants. At epoch end (the only point scores affect staking),
            // consensus processing has stopped so there is no active writer.
            for (i, &score) in scores.iter().enumerate() {
                self.current_scores[i].store(score, Ordering::Relaxed);
            }
        }
    }

    pub(crate) fn received_reports_state_snapshot(
        &self,
    ) -> Vec<DBReceivedReportsStatePerAuthority> {
        self.received_reports_state.serializable_snapshot()
    }

    pub(crate) fn received_reports_state_per_authority_snapshot(
        &self,
        authority_index: u32,
    ) -> DBReceivedReportsStatePerAuthority {
        self.received_reports_state.0[authority_index as usize].to_serializable()
    }
}

// Scorer version. V1 is active when scorer_version is None or Some(1).
enum ScorerVersion {
    V1(Parameters),
}

/// Scoring parameters for a given version. All `Vec` fields are indexed by the
/// tracked misbehavior index (same order as `ReportedMisbehaviors`).
struct Parameters {
    // Allowed misbehaviors without any punishment
    allowances: Vec<u64>,
    // Number of misbehaviors that lead to zero score
    maximums: Vec<u64>,
    // Precomputed indices and weights for minor misbehaviors.
    minor_indices: Vec<usize>,
    minor_weights: Vec<u64>,
    // Precomputed indices for major misbehaviors.
    major_indices: Vec<usize>,
    // Precomputed baseline score (SCALE_FACTOR - sum of minor weights).
    baseline_score: u64,
}

type VotingPower = u64;

/// Maps a single misbehavior count to a score in [0, max_score].
/// Returns max_score if value <= allowance (no penalty), 0 if value >= max
/// (zero score), and linearly interpolates in between.
fn metric_to_score(value: u64, allowance: u64, max: u64, max_score: u64) -> u64 {
    if value <= allowance {
        max_score
    } else if value >= max {
        0
    } else {
        // max - allowance > 0 and the multiplication not overflowing are guaranteed by
        // assertions done during scorer initialization
        max.saturating_sub(value).saturating_mul(max_score) / max.saturating_sub(allowance)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::Ordering};

    use iota_protocol_config::ProtocolConfig;
    use iota_types::messages_consensus::{LegacyReportPayload, VersionedMisbehaviorReport};

    use super::{MAX_SCORE, Scorer};
    use crate::authority::authority_per_epoch_store::misbehavior_monitor::{
        MisbehaviorCounts, MisbehaviorMonitor,
    };

    fn mock_protocol_config() -> ProtocolConfig {
        ProtocolConfig::get_for_max_version_UNSAFE()
    }

    fn mock_misbehavior_monitor(committee_size: usize) -> Arc<MisbehaviorMonitor> {
        Arc::new(MisbehaviorMonitor::new(
            &mock_protocol_config(),
            committee_size,
        ))
    }

    fn report(
        provable: Vec<u64>,
        unprovable: Vec<u64>,
        missing: Vec<u64>,
        equivocations: Vec<u64>,
    ) -> VersionedMisbehaviorReport {
        VersionedMisbehaviorReport::new_v1(LegacyReportPayload {
            faulty_blocks_provable: provable,
            faulty_blocks_unprovable: unprovable,
            missing_proposals: missing,
            equivocations,
        })
    }

    impl Scorer {
        fn set_reports(&self, reports_and_authorities: &[(VersionedMisbehaviorReport, u32)]) {
            for (report, authority) in reports_and_authorities {
                self.update_received_reports(*authority, report);
            }
        }
    }

    #[test]
    fn test_scorer_initialization() {
        let voting_power = vec![10, 20, 30];
        let committee_size = voting_power.len();
        let scorer = Scorer::new(
            voting_power,
            &mock_protocol_config(),
            &mock_misbehavior_monitor(committee_size),
        );

        assert_eq!(scorer.current_scores.len(), committee_size);
        for score in scorer.current_scores.iter() {
            assert_eq!(score.load(Ordering::Relaxed), MAX_SCORE);
        }
        for i in 0..committee_size {
            assert_eq!(
                scorer.received_reports_state[i].invalid_reports_count_snapshot(),
                0
            );
            assert!(
                scorer.received_reports_state[i]
                    .received_metrics
                    .load()
                    .is_none()
            );
        }
    }

    #[test]
    fn test_update_invalid_reports_count() {
        let voting_power = vec![10, 20, 30];
        let committee_size = voting_power.len();
        let scorer = Scorer::new(
            voting_power,
            &mock_protocol_config(),
            &mock_misbehavior_monitor(committee_size),
        );

        scorer.update_invalid_reports_count(2);
        assert_eq!(
            scorer.received_reports_state[0].invalid_reports_count_snapshot(),
            0
        );
        assert_eq!(
            scorer.received_reports_state[1].invalid_reports_count_snapshot(),
            0
        );
        assert_eq!(
            scorer.received_reports_state[2].invalid_reports_count_snapshot(),
            1
        );

        scorer.increment_invalid_reports_count(1);
        scorer.increment_invalid_reports_count(1);
        assert_eq!(
            scorer.received_reports_state[0].invalid_reports_count_snapshot(),
            0
        );
        assert_eq!(
            scorer.received_reports_state[1].invalid_reports_count_snapshot(),
            2
        );
        assert_eq!(
            scorer.received_reports_state[2].invalid_reports_count_snapshot(),
            1
        );
    }

    #[test]
    fn test_update_scores() {
        // Committee of 3, voting powers [2, 5, 20].
        //
        // Weighted medians (total_vp = 27, threshold = 14):
        //   provable[0]:      reporters [(5,2),(0,5),(0,20)] → sorted
        // [(0,5),(0,20),(5,2)]                     accumulated 5, 25 ≥ 14 →
        // median = 0   provable[2]:      reporters [(0,2),(0,5),(15,20)] →
        // accumulated 2,7,27 → median = 15   equivocations[0]: reporters
        // [(0,2),(0,5),(5,20)] → accumulated 2,7,27 → median = 5
        //
        // Scores:
        //   authority 0: equivocations median = 5 ≥ max(1) → major factor = 0 → score =
        // 0   authority 1: all medians = 0 → MAX_SCORE
        //   authority 2: provable median = 15 ≥ max(5) → provable contribution = 0,
        //                all other metrics 0 → score = baseline + unprovable + missing
        // = 45876
        let voting_power = vec![2, 5, 20];
        let committee_size = voting_power.len();
        let scorer = Scorer::new(
            voting_power,
            &mock_protocol_config(),
            &mock_misbehavior_monitor(committee_size),
        );

        for score in scorer.current_scores.iter() {
            assert_eq!(score.load(Ordering::Relaxed), MAX_SCORE);
        }

        scorer.set_reports(&[
            (
                report(vec![5, 0, 0], vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]),
                0,
            ),
            (
                report(vec![0, 10, 0], vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]),
                1,
            ),
            (
                report(vec![0, 0, 15], vec![0, 0, 0], vec![0, 0, 0], vec![5, 0, 0]),
                2,
            ),
        ]);

        scorer.update_scores();

        let actual: Vec<u64> = scorer
            .current_scores
            .iter()
            .map(|s| s.load(Ordering::Relaxed))
            .collect();
        assert_eq!(actual, vec![0, MAX_SCORE, 45876]);
    }

    #[test]
    fn test_calculate_median_report() {
        let protocol_config = mock_protocol_config();

        // Single reporter: median equals their own report.
        {
            let scorer = Scorer::new(
                vec![10, 10, 10],
                &protocol_config,
                &mock_misbehavior_monitor(3),
            );
            scorer.update_received_reports(
                0,
                &report(
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3],
                ),
            );
            let median = scorer.calculate_median_report().unwrap();
            assert_eq!(
                median.0,
                vec![
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3]
                ]
            );
        }

        // Two reporters with 2:1 voting power — the majority reporter's values win.
        // total_vp = 30, threshold = 15. Authority 0 (vp=20) always crosses the
        // threshold first.
        {
            let scorer = Scorer::new(
                vec![20, 10, 10],
                &protocol_config,
                &mock_misbehavior_monitor(3),
            );
            scorer.update_received_reports(
                0,
                &report(
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3],
                ),
            );
            scorer.update_received_reports(
                1,
                &report(
                    vec![70, 80, 90],
                    vec![100, 110, 120],
                    vec![40, 50, 60],
                    vec![10, 20, 30],
                ),
            );
            let median = scorer.calculate_median_report().unwrap();
            assert_eq!(
                median.0,
                vec![
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3]
                ]
            );
        }

        // Three equal reporters — standard weighted median.
        // total_vp = 30, threshold = 15. The middle value wins for each (metric,
        // authority) pair.
        {
            let scorer = Scorer::new(
                vec![10, 10, 10],
                &protocol_config,
                &mock_misbehavior_monitor(3),
            );
            scorer.update_received_reports(
                0,
                &report(
                    vec![1, 8, 9],
                    vec![10, 15, 12],
                    vec![4, 5, 6],
                    vec![1, 20, 3],
                ),
            );
            scorer.update_received_reports(
                1,
                &report(
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 0],
                ),
            );
            scorer.update_received_reports(
                2,
                &report(
                    vec![6, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 22, 6],
                    vec![1, 2, 30],
                ),
            );
            let median = scorer.calculate_median_report().unwrap();
            assert_eq!(
                median.0,
                vec![
                    vec![6, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3]
                ]
            );
        }
    }

    #[test]
    fn test_calculate_scores_v1() {
        // V1 parameters (see load_parameters):
        //   allowances:      [1, 2, 48_000, 0]
        //   maximums:        [5, 10, 160_000, 1]
        //   minor weights:   [19660, 6553, 22937]  (30%, 10%, 35% of SCALE_FACTOR)
        //   baseline_score:  16386  (SCALE_FACTOR - sum_of_minor_weights)
        //
        // All-zero misbehaviors → every authority gets MAX_SCORE (65536).
        // Derivation: (16386 + 19660 + 6553 + 22937) * MAX_SCORE / SCALE_FACTOR =
        // MAX_SCORE.
        let committee_size = 3;
        let scorer = Scorer::new(
            vec![10; committee_size],
            &mock_protocol_config(),
            &mock_misbehavior_monitor(committee_size),
        );

        assert_eq!(
            scorer.calculate_scores_v1(MisbehaviorCounts(vec![
                vec![0, 0, 0], // provable
                vec![0, 0, 0], // unprovable
                vec![0, 0, 0], // missing
                vec![0, 0, 0], // equivocations
            ])),
            vec![MAX_SCORE, MAX_SCORE, MAX_SCORE]
        );

        // Authority 0 equivocates (≥ max 1) → major factor = 0 → score = 0.
        assert_eq!(
            scorer.calculate_scores_v1(MisbehaviorCounts(vec![
                vec![0, 0, 0],
                vec![0, 0, 0],
                vec![0, 0, 0],
                vec![1, 0, 0],
            ])),
            vec![0, MAX_SCORE, MAX_SCORE]
        );

        // Authority 0 reaches provable-fault maximum (≥ 5) → provable contribution = 0.
        // score = (baseline + unprovable_weight + missing_weight) = 16386 + 6553 +
        // 22937 = 45876.
        assert_eq!(
            scorer.calculate_scores_v1(MisbehaviorCounts(vec![
                vec![5, 0, 0],
                vec![0, 0, 0],
                vec![0, 0, 0],
                vec![0, 0, 0],
            ])),
            vec![45876, MAX_SCORE, MAX_SCORE]
        );
    }
}
