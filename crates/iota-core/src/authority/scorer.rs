// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use arc_swap::ArcSwap;
use iota_protocol_config::ProtocolConfig;

use crate::authority::authority_per_epoch_store::{
    misbehavior::{MisbehaviorCounts, MisbehaviorCountsV1, MisbehaviorSchemaVersion},
    report_aggregator::ReportAggregator,
};

/// Must match MAX_SCORE in validator_set.move in iota-framework.
pub(crate) const MAX_SCORE: u64 = u16::MAX as u64 + 1;
/// Fixed-point scale used when combining weighted minor scores before dividing
/// back down to [0, MAX_SCORE]. Chosen as 2^16 so that MAX_SCORE * SCALE_FACTOR
/// fits in a u64 without overflow.
const SCALE_FACTOR: u64 = 2_u64.pow(16);

/// Pure score computation engine. Does not own any report state — reads
/// aggregated counts from a `ReportAggregator` when updating scores.
pub struct Scorer {
    // The current scores of the authorities, updated after each scoring round.
    // Published as a single `Arc<Vec<u64>>` so readers always see a consistent
    // snapshot across all authorities (no torn reads mixing old and new scores).
    current_scores: ArcSwap<Vec<u64>>,
    // The voting power of each authority in the committee.
    voting_power: Vec<u64>,
    // The misbehavior schema version this scorer is bound to. Lets `calculate_scores_v1`
    // resolve `Parameters` indices back to `Misbehavior` variants when looking up rows.
    schema_version: MisbehaviorSchemaVersion,
    // The version of the scorer being used with its parameters.
    version: ScorerVersion,
}

impl Scorer {
    pub fn new(
        voting_power: Vec<u64>,
        protocol_config: &ProtocolConfig,
        schema_version: MisbehaviorSchemaVersion,
    ) -> Self {
        let committee_size = voting_power.len();
        let version = ScorerVersion::from_protocol(protocol_config, schema_version.num_metrics());
        let current_scores = ArcSwap::from_pointee(vec![MAX_SCORE; committee_size]);

        Self {
            current_scores,
            voting_power,
            schema_version,
            version,
        }
    }

    /// Recomputes all authority scores from the aggregated reports in the
    /// `ReportAggregator`.
    pub(crate) fn update_scores(&self, aggregator: &ReportAggregator) {
        match self.version {
            ScorerVersion::V1(_) => self.update_scores_v1(aggregator),
        };
    }

    pub(crate) fn current_scores(&self) -> Vec<u64> {
        self.current_scores.load().as_ref().clone()
    }

    fn get_parameters(&self) -> &Parameters {
        match &self.version {
            ScorerVersion::V1(params) => params,
        }
    }
}

impl Scorer {
    /// Calculates the weighted median across all reporters for each metric and
    /// authority. Returns `None` if no authority has sent a report yet.
    fn calculate_median_report(&self, aggregator: &ReportAggregator) -> Option<MisbehaviorCounts> {
        let reporters = aggregator.reporters_with_voting_power(&self.voting_power);

        if reporters.is_empty() {
            return None;
        }

        // Destructure each reporter into its V1 inner once so the per-metric
        // closures don't repeat the match. Forces a deliberate decision when V2
        // lands (this destructure becomes a match against multiple variants).
        let reporters_v1: Vec<(&MisbehaviorCountsV1, VotingPower)> = reporters
            .iter()
            .map(|(counts, vp)| match counts.as_ref() {
                MisbehaviorCounts::V1(c) => (c, *vp),
            })
            .collect();

        let committee_size = self.voting_power.len();
        // Sum only over reporters, not the full committee — the median is weighted
        // by the voting power of authorities that actually submitted a report.
        let total_voting_power: VotingPower = reporters_v1.iter().map(|(_, vp)| *vp).sum();

        // Reused across all (metric, authority) pairs — one allocation total.
        let mut chunk: Vec<(u64, VotingPower)> = Vec::with_capacity(reporters_v1.len());

        let mut weighted_median_for_metric = |select: &dyn Fn(&MisbehaviorCountsV1) -> &[u64]| {
            let mut median_for_metric = Vec::with_capacity(committee_size);
            for authority in 0..committee_size {
                chunk.clear();
                chunk.extend(
                    reporters_v1
                        .iter()
                        .map(|(counts, vp)| (select(counts)[authority], *vp)),
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
            median_for_metric
        };

        // Each named metric is computed explicitly. Adding a metric to
        // `MisbehaviorCountsV1` makes this struct literal a missing-field error,
        // forcing the new metric to be wired into the median computation.
        Some(MisbehaviorCounts::V1(MisbehaviorCountsV1 {
            faulty_blocks_provable: weighted_median_for_metric(&|c| &c.faulty_blocks_provable),
            faulty_blocks_unprovable: weighted_median_for_metric(&|c| &c.faulty_blocks_unprovable),
            missing_proposals: weighted_median_for_metric(&|c| &c.missing_proposals),
            equivocations: weighted_median_for_metric(&|c| &c.equivocations),
        }))
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
        let MisbehaviorCounts::V1(median) = median_counts;
        // `Parameters` arrays are positionally aligned with `reported_misbehaviors()`
        // (out of scope for this refactor); look up rows by `Misbehavior` variant
        // to bridge into `MisbehaviorCountsV1`'s named fields.
        let metrics = self.schema_version.reported_misbehaviors();

        // Initialise with the baseline; values are in [0, MAX_SCORE * SCALE_FACTOR].
        let mut final_scores = vec![parameters.baseline_score * MAX_SCORE; committee_size];

        // Accumulate weighted minor misbehavior scores directly into final_scores.
        for (&i, &weight) in parameters
            .minor_indices
            .iter()
            .zip(parameters.minor_weights.iter())
        {
            for (authority, &count) in median.metric(metrics[i]).iter().enumerate() {
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
                    median.metric(metrics[i])[authority],
                    parameters.allowances[i],
                    parameters.maximums[i],
                    1,
                );
            }
        }

        final_scores
    }

    fn update_scores_v1(&self, aggregator: &ReportAggregator) {
        if let Some(median_counts) = self.calculate_median_report(aggregator) {
            let scores = self.calculate_scores_v1(median_counts);
            // Single pointer swap publishes the whole vector; checkpoint readers
            // never observe a mix of old and new scores.
            self.current_scores.store(Arc::new(scores));
        }
    }
}

/// Scorer version with its associated parameters. Loaded from `ProtocolConfig`
/// and validated against the misbehavior schema's metric count at construction.
enum ScorerVersion {
    V1(Parameters),
}

impl ScorerVersion {
    fn from_protocol(protocol_config: &ProtocolConfig, num_metrics: usize) -> Self {
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => {
                assert!(
                    num_metrics == 4,
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
            Some(version) => panic!("Unsupported scorer version {version}"),
        }
    }
}

/// Scoring parameters for a given version. All `Vec` fields are indexed by the
/// tracked misbehavior index — the order produced by
/// `MisbehaviorSchemaVersion::reported_misbehaviors()`.
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
    use iota_protocol_config::ProtocolConfig;
    use iota_types::messages_consensus::{ReportPayloadV1, VersionedMisbehaviorReport};

    use crate::authority::authority_per_epoch_store::{
        misbehavior::{MisbehaviorCounts, MisbehaviorCountsV1, MisbehaviorSchemaVersion},
        report_aggregator::ReportAggregator,
        scorer::{MAX_SCORE, Scorer},
    };

    fn mock_protocol_config() -> ProtocolConfig {
        ProtocolConfig::get_for_max_version_UNSAFE()
    }

    fn mock_schema_version() -> MisbehaviorSchemaVersion {
        MisbehaviorSchemaVersion::from_protocol(&mock_protocol_config())
    }

    fn mock_scorer(voting_power: Vec<u64>) -> Scorer {
        Scorer::new(voting_power, &mock_protocol_config(), mock_schema_version())
    }

    fn mock_aggregator(committee_size: usize) -> ReportAggregator {
        ReportAggregator::new(mock_schema_version(), committee_size)
    }

    fn report_v1(raw_counts: &[Vec<u64>; 4]) -> VersionedMisbehaviorReport {
        VersionedMisbehaviorReport::new_v1(ReportPayloadV1 {
            faulty_blocks_provable: raw_counts[0].clone(),
            faulty_blocks_unprovable: raw_counts[1].clone(),
            missing_proposals: raw_counts[2].clone(),
            equivocations: raw_counts[3].clone(),
        })
    }

    fn set_reports(
        aggregator: &ReportAggregator,
        reports_and_authorities: &[(VersionedMisbehaviorReport, u32)],
    ) {
        for (report, authority) in reports_and_authorities {
            aggregator.process_report(*authority, report);
        }
    }

    #[test]
    fn test_scorer_initialization() {
        let voting_power = vec![10, 20, 30];
        let committee_size = voting_power.len();
        let scorer = mock_scorer(voting_power);

        let scores = scorer.current_scores();
        assert_eq!(scores.len(), committee_size);
        assert!(scores.iter().all(|&s| s == MAX_SCORE));
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
        let aggregator = mock_aggregator(committee_size);
        let scorer = mock_scorer(voting_power);

        assert!(scorer.current_scores().iter().all(|&s| s == MAX_SCORE));

        set_reports(
            &aggregator,
            &[
                (
                    report_v1(&[vec![5, 0, 0], vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]]),
                    0,
                ),
                (
                    report_v1(&[vec![0, 10, 0], vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]]),
                    1,
                ),
                (
                    report_v1(&[vec![0, 0, 15], vec![0, 0, 0], vec![0, 0, 0], vec![5, 0, 0]]),
                    2,
                ),
            ],
        );

        scorer.update_scores(&aggregator);

        let expected_score = vec![0, 65536, 45876];
        let actual_score = scorer.current_scores();
        assert_eq!(actual_score, expected_score);
    }

    #[test]
    fn test_calculate_median_report() {
        // Single reporter: median equals their own report.
        {
            let aggregator = mock_aggregator(3);
            let scorer = mock_scorer(vec![10, 10, 10]);
            aggregator.process_report(
                0,
                &report_v1(&[
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3],
                ]),
            );
            let median = scorer.calculate_median_report(&aggregator).unwrap();
            assert_eq!(
                median,
                MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                    faulty_blocks_provable: vec![7, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 3],
                })
            );
        }

        // Two reporters with 2:1 voting power — the majority reporter's values win.
        // total_vp = 30, threshold = 15. Authority 0 (vp=20) always crosses the
        // threshold first.
        {
            let aggregator = mock_aggregator(3);
            let scorer = mock_scorer(vec![20, 10, 10]);
            aggregator.process_report(
                0,
                &report_v1(&[
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3],
                ]),
            );
            aggregator.process_report(
                1,
                &report_v1(&[
                    vec![70, 80, 90],
                    vec![100, 110, 120],
                    vec![40, 50, 60],
                    vec![10, 20, 30],
                ]),
            );
            let median = scorer.calculate_median_report(&aggregator).unwrap();
            assert_eq!(
                median,
                MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                    faulty_blocks_provable: vec![7, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 3],
                })
            );
        }

        // Three equal reporters — standard weighted median.
        // total_vp = 30, threshold = 15. The middle value wins for each (metric,
        // authority) pair.
        {
            let aggregator = mock_aggregator(3);
            let scorer = mock_scorer(vec![10, 10, 10]);
            aggregator.process_report(
                0,
                &report_v1(&[
                    vec![1, 8, 9],
                    vec![10, 15, 12],
                    vec![4, 5, 6],
                    vec![1, 20, 3],
                ]),
            );
            aggregator.process_report(
                1,
                &report_v1(&[
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 0],
                ]),
            );
            aggregator.process_report(
                2,
                &report_v1(&[
                    vec![6, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 22, 6],
                    vec![1, 2, 30],
                ]),
            );
            let median = scorer.calculate_median_report(&aggregator).unwrap();
            assert_eq!(
                median,
                MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                    faulty_blocks_provable: vec![6, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 3],
                })
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
        let scorer = mock_scorer(vec![10; committee_size]);

        assert_eq!(
            scorer.calculate_scores_v1(MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                faulty_blocks_provable: vec![0, 0, 0],
                faulty_blocks_unprovable: vec![0, 0, 0],
                missing_proposals: vec![0, 0, 0],
                equivocations: vec![0, 0, 0],
            })),
            vec![MAX_SCORE, MAX_SCORE, MAX_SCORE]
        );

        // Authority 0 equivocates (≥ max 1) → major factor = 0 → score = 0.
        assert_eq!(
            scorer.calculate_scores_v1(MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                faulty_blocks_provable: vec![0, 0, 0],
                faulty_blocks_unprovable: vec![0, 0, 0],
                missing_proposals: vec![0, 0, 0],
                equivocations: vec![1, 0, 0],
            })),
            vec![0, MAX_SCORE, MAX_SCORE]
        );

        // Authority 0 reaches provable-fault maximum (≥ 5) → provable contribution = 0.
        // score = (baseline + unprovable_weight + missing_weight) = 16386 + 6553 +
        // 22937 = 45876.
        assert_eq!(
            scorer.calculate_scores_v1(MisbehaviorCounts::V1(MisbehaviorCountsV1 {
                faulty_blocks_provable: vec![5, 0, 0],
                faulty_blocks_unprovable: vec![0, 0, 0],
                missing_proposals: vec![0, 0, 0],
                equivocations: vec![0, 0, 0],
            })),
            vec![45876, MAX_SCORE, MAX_SCORE]
        );
    }
}
