// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use iota_protocol_config::ProtocolConfig;
use iota_types::{
    messages_consensus::{MisbehaviorsV1, VersionedMisbehaviorReport},
    scoring_metrics::VersionedScoringMetrics,
};

pub(crate) const MAX_SCORE: u64 = u16::MAX as u64 + 1; // Note: must be consistent with MAX_SCORE in validator_set.move in iota-framework.
const SCALE_FACTOR: u64 = 2_u64.pow(16);

/// Holds all information related to scoring of authorities in the committee.
pub struct Scorer {
    // The current metrics counts collected by the authority, i.e., the local view of the node
    // about the behaviour of the rest of the committee, according to the blocks received.
    pub(crate) current_local_metrics_count: Arc<VersionedScoringMetrics>,
    // The metrics counts received from other authorities, i.e., the information contained in the
    // MisbehaviourReports received by the authority. If an authority has not sent a report, its
    // entry in this vector will be all zeroed.
    received_metrics: Vec<VersionedScoringMetrics>,
    // Indicates whether an authority did not send any misbehavior reports in the epoch. We use
    // this to differentiate an authority that did not send a report from another one who sent
    // zeroed reports.
    has_not_sent_report: Vec<AtomicBool>,
    // The current scores of the authorities, updated after each received report. This score is
    // calculated based on the information in the received reports and the validity of the reports
    // themselves.
    pub(crate) current_scores: Scores,
    // The count of invalid reports received from each authority. Validity here must be checked in
    // a deterministic way, since this information will not be propagated again to the rest of the
    // committee.
    invalid_reports_count: Vec<AtomicU64>,
    // The voting power of each authority in the committee.
    voting_power: Vec<u64>,
    // The version of the scorer being used with its parameters.
    version: ScorerVersion,
}

impl Scorer {
    pub fn new(voting_power: Vec<u64>, protocol_config: &ProtocolConfig) -> Self {
        let committee_size = voting_power.len();
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => {
                // Local metrics count are always initialized as zero.
                let current_local_metrics_count = Arc::new(VersionedScoringMetrics::new(
                    committee_size,
                    protocol_config,
                ));
                let (received_metrics, has_not_sent_report, current_scores, invalid_reports_count) =
                    (0..committee_size)
                        .map(|_| {
                            (
                                // Received metrics initialized to zero.
                                VersionedScoringMetrics::new(committee_size, protocol_config),
                                // Initially, none of the authorities had sent any valid report.
                                AtomicBool::new(true),
                                // Current scores initialized to max score.
                                AtomicU64::new(MAX_SCORE),
                                // Invalid reports count initialized to zero.
                                AtomicU64::new(0),
                            )
                        })
                        .collect();
                let parameters = ParametersV1 {
                    allowances: MisbehaviorsV1 {
                        faulty_blocks_provable: 1,
                        faulty_blocks_unprovable: 2,
                        missing_proposals: 48_000, // roughly 3% of consensus rounds in an epoch
                        equivocations: 0,
                    },
                    maximums: MisbehaviorsV1 {
                        faulty_blocks_provable: 5,
                        faulty_blocks_unprovable: 10,
                        missing_proposals: 160_000, // roughly 10% of consensus rounds in an epoch
                        equivocations: 1,
                    },
                    weights: MisbehaviorsV1 {
                        faulty_blocks_provable: SCALE_FACTOR * 30 / 100,
                        faulty_blocks_unprovable: SCALE_FACTOR * 10 / 100,
                        missing_proposals: SCALE_FACTOR * 35 / 100,
                        equivocations: 1,
                    },
                };
                // Assert that the allowance for major misbehaviors is 0,
                // maximum is 1 and weight is 1. This is because major misbehaviors should
                // reduce the score to 0 is there are any occurrences.
                // Only equivocation is considered a major misbehavior in this version.
                assert!(
                    parameters
                        .allowances
                        .iter_major_misbehaviors()
                        .all(|&a| a == 0)
                        && parameters
                            .maximums
                            .iter_major_misbehaviors()
                            .all(|&m| m == 1)
                        && parameters
                            .weights
                            .iter_major_misbehaviors()
                            .all(|&w| w == 1)
                );
                // Assert that allowances are compatible with the maximums for all metrics.
                assert!(
                    parameters
                        .allowances
                        .iter()
                        .zip(parameters.maximums.iter())
                        .all(|(&a, &m)| a < m)
                );

                // Assert that maximums are compatible with MAX_SCORE for all metrics, to
                // prevent overflows.
                assert!(
                    parameters
                        .maximums
                        .iter()
                        .all(|&a| a <= u64::MAX / MAX_SCORE)
                );

                Self {
                    current_local_metrics_count,
                    received_metrics,
                    has_not_sent_report,
                    current_scores,
                    invalid_reports_count,
                    voting_power,
                    version: ScorerVersion::V1(parameters),
                }
            }
            _ => panic!("Unsupported scorer version"),
        }
    }

    fn get_parameters_v1(&self) -> ParametersV1 {
        match &self.version {
            ScorerVersion::V1(params) => params.clone(),
        }
    }

    // Boundary checks for this functions are done at a higher level. `authority``
    // should always be derived from a valid AuthorityIndex
    pub(crate) fn update_invalid_reports_count(&self, authority: u32) {
        self.invalid_reports_count[authority as usize].fetch_add(1, Ordering::Relaxed);
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
        // Update the received metrics for the authority, and mark that we have received
        // metrics from them. Then, update the scores accordingly.
        self.received_metrics[authority as usize].update_from_report(report);
        self.has_not_sent_report[authority as usize].store(false, Ordering::Relaxed);
    }
}

// Methods for ScorerVersion::V1
impl Scorer {
    fn update_scores_v1(&self) {
        // Vector with the highest received reports from each authority and their voting
        // power. Authorities that did not send reports are filtered out.
        let highest_received_reports_from_authority = self
            .received_metrics
            .iter()
            .zip(self.voting_power.iter())
            .zip(self.has_not_sent_report.iter())
            .filter(|((_, _), is_missing)| !is_missing.load(Ordering::Relaxed))
            .map(|((metrics, voting_power), _)| (metrics.to_report(), *voting_power))
            .collect::<Vec<(VersionedMisbehaviorReport, VotingPower)>>();
        // Ensure that we have at least one report to calculate the scores, otherwise we
        // do nothing.
        if highest_received_reports_from_authority.is_empty() {
        } else {
            let median_report = calculate_median_report(&highest_received_reports_from_authority);
            let scores = calculate_scores_v1(median_report, self.get_parameters_v1());
            for (i, &score) in scores.iter().enumerate() {
                self.current_scores[i].store(score, Ordering::Relaxed);
            }
        }
    }
}

/// Given a vector of pairs (VersionedMisbehaviorReport, VotingPower), calculate
/// the medians for all metrics in VersionedMisbehaviorReport and authorities:
///
/// - Assume we have N authorities in the committee, but n<=N reports R_1, R_2,
///   ..., R_n from authorities with voting powers VP_1, VP_2, ..., VP_n.
/// - For each metric M in VersionedMisbehaviorReport, we'll have n vectors of
///   metric values: M_1, M_2, ..., M_n, where M_i is the vector of metric
///   values for report R_i.
/// - Each M_i is a vector of length N, where the j-th value corresponds to the
///   metric value for authority j.
///
/// Example: If we have 4 authorities in the committee, and we receive 3
/// reports:
/// - Report R_1 from authority A_1 with voting power VP_1 = 1:
///     - Metric M1: [0, 0, 0, 0] (values for authorities A_1, A_2, A_3, A_4)
///     - Metric M2: [0, 0, 0, 0] (values for authorities A_1, A_2, A_3, A_4)
/// - Report R_2 from authority A_2 with voting power VP_2 = 1:
///     - Metric M1: [1, 1, 1, 1] (values for authorities A_1, A_2, A_3, A_4)
///     - Metric M2: [2, 2, 2, 2] (values for authorities A_1, A_2, A_3, A_4)
/// - Report R_3 from authority A_3 with voting power VP_3 = 1:
///     - Metric M1: [2, 2, 2, 2] (values for authorities A_1, A_2, A_3, A_4)
///     - Metric M2: [1, 1, 1, 1] (values for authorities A_1, A_2, A_3, A_4)
///
/// For Metric M1, we have that the median metric vector is [1, 1, 1, 1].
///
/// This method returns a vector of MedianMetricVec, one per metric in
/// VersionedMisbehaviorReport
fn calculate_median_report(
    reports_and_voting_power: &[(VersionedMisbehaviorReport, VotingPower)],
) -> MisbehaviorsV1<MedianMetricVec> {
    // Calls to this method should ensure that we have at least one report to
    // process.
    assert!(!reports_and_voting_power.is_empty());

    let number_of_metrics = reports_and_voting_power[0].0.iterate_over_metrics().len();

    // In the case of the example in the method documentation,
    // reports_and_voting_power_per_metric should be
    // vec![
    //      vec![([0, 0, 0, 0],VP_1),([1, 1, 1, 1],VP_2),([2, 2, 2, 2],VP_3)],
    //      vec![([0, 0, 0, 0],VP_1),([2, 2, 2, 2],VP_2),([1, 1, 1, 1],VP_3)]
    //      ]
    let mut reports_and_voting_power_per_metric: Vec<Vec<(MetricVec, VotingPower)>> =
        vec![vec![]; number_of_metrics];
    for (versioned_report, voting_power) in reports_and_voting_power.iter() {
        for (i, metric) in versioned_report.iterate_over_metrics().enumerate() {
            reports_and_voting_power_per_metric[i].push((metric.clone(), *voting_power));
        }
    }

    // Calculate and return the weighted median for each metric
    let median_report = reports_and_voting_power_per_metric
        .iter_mut()
        .map(|vec| calculate_weighted_median(vec.as_mut_slice()))
        .collect::<MisbehaviorsV1<MedianMetricVec>>();
    median_report
}

// Given a vector of pairs (MetricVec, VotingPower), calculate the weighted
// median of each entry of MetricVec and returns a MedianMetricVec. Each entry
// of reports corresponds to a single authority i who sent the report. MetricVec
// always corresponds to a single metric, and each of its entries corresponds to
// the number of misbehaviors that i claims to have detected from each authority
// in the committee.
fn calculate_weighted_median(reports: &mut [(MetricVec, VotingPower)]) -> MedianMetricVec {
    // Calls to this method should ensure that we have at least one pair (MetricVec,
    // VotingPower) to process.
    assert!(!reports.is_empty());

    // We calculate the weighted median relative to the voting power of the
    // authorities who actually sent a report.
    let voting_power_used = reports.iter().map(|(_, vp)| *vp).sum::<VotingPower>();
    // The caller should also guarantee that the MetricVec in all reports have the
    // same length (committee_size). This is naturally guaranteed when these data
    // come from MisbehaviorReports, since they would been considered invalid
    // otherwise.
    let committee_size = reports[0].0.len();
    let mut median_per_validator_being_scored = Vec::new();

    for validator_being_scored in 0..committee_size {
        let mut accumulated_voting_power = 0;
        reports.sort_by_key(|(reported_counts, _)| reported_counts[validator_being_scored]);
        for (reported_counts, voting_power) in reports.iter() {
            accumulated_voting_power += *voting_power;
            if accumulated_voting_power * 2 >= voting_power_used {
                median_per_validator_being_scored.push(reported_counts[validator_being_scored]);
                break;
            }
        }
    }

    median_per_validator_being_scored
}

// Scorer version. Currently, only V1 is implemented, relative to both
// protocol_config.scorer_version = None or Some(1).
enum ScorerVersion {
    V1(ParametersV1),
}

// Parameters for ScorerVersion::V1
#[derive(Clone)]
struct ParametersV1 {
    // Allowed misbehaviors without any punishment
    allowances: MisbehaviorsV1<u64>,
    // Number of misbehaviors that lead to zero score
    maximums: MisbehaviorsV1<u64>,
    // Weights for each metric. The sum of minor misbehavior weights + baseline_score =
    // scale_factor. Major misbehavior weights are either 0 or 1.
    weights: MisbehaviorsV1<u64>,
}

// Aliases for better readability.
pub(crate) type Scores = Vec<Score>;
pub(crate) type Score = AtomicU64;
type VotingPower = u64;
type MedianMetricVec = Vec<u64>;
type MetricVec = Vec<u64>;

// Given the median reports for all metrics, calculate the final scores. A score
// is an integer between 0 and max_score. For each metrics, we have an allowance
// (allowed misbehaviors without any punishment) and a maximum (number of
// misbehaviors that lead to zero score). Based on those values, we calculate a
// score per metric, and then combine them into a final score. Each individual
// score for minor misbeahviors (non-equivocation) is also  an integer between 0
// and max_score, and the weights used for the combination are such that
// sum(weights) + baseline_score = scale_factor. Thus, we need
// max_score*scale_factor < 2^64 to avoid overflows.
// Major misbehaviors (equivocations) are treated differently, as they
// multiplicatively impact the final score. Their value is either 0 or 1.
fn calculate_scores_v1(
    median_reports: MisbehaviorsV1<MedianMetricVec>,
    parameters: ParametersV1,
) -> Vec<u64> {
    let baseline_score = SCALE_FACTOR - parameters.weights.iter_minor_misbehaviors().sum::<u64>();

    let median_minor_reports_and_parameters = median_reports
        .iter_minor_misbehaviors()
        .zip(parameters.allowances.iter_minor_misbehaviors())
        .zip(parameters.maximums.iter_minor_misbehaviors());

    // Calculate individual metric scores
    let minor_metric_scores = median_minor_reports_and_parameters
        .map(
            |((median_report_for_a_single_metric, metric_allowance), metric_maximum)| {
                median_report_single_metric_to_score(
                    median_report_for_a_single_metric,
                    *metric_allowance,
                    *metric_maximum,
                    MAX_SCORE,
                )
            },
        )
        .collect::<Vec<Vec<u64>>>();

    let median_major_reports_and_parameters = median_reports
        .iter_major_misbehaviors()
        .zip(parameters.allowances.iter_major_misbehaviors())
        .zip(parameters.maximums.iter_major_misbehaviors());

    // Calculate individual metric scores
    let major_metric_scores = median_major_reports_and_parameters
        .map(
            |((median_report_for_a_single_metric, metric_allowance), metric_maximum)| {
                median_report_single_metric_to_score(
                    median_report_for_a_single_metric,
                    *metric_allowance,
                    *metric_maximum,
                    1,
                )
            },
        )
        .collect::<Vec<Vec<u64>>>();

    metrics_scores_to_final_scores(
        minor_metric_scores,
        major_metric_scores,
        parameters.weights,
        baseline_score,
        SCALE_FACTOR,
        MAX_SCORE,
    )
}

fn metrics_scores_to_final_scores(
    minor_metric_scores: Vec<Vec<u64>>,
    major_metric_scores: Vec<Vec<u64>>,
    weights: MisbehaviorsV1<u64>,
    baseline_score: u64,
    scale_factor: u64,
    max_score: u64,
) -> Vec<u64> {
    // Initialise the final scores with the baseline score whose value is between 0
    // and max_score * scale_factor.
    let committee_size = minor_metric_scores.first().unwrap().len();
    let mut final_scores = vec![baseline_score * max_score; committee_size];
    // First, calculate the weights sum of minor misbehavior scores vector. The
    // values in final_scores will still be between 0 and max_score * scale_factor
    minor_metric_scores
        .iter()
        .zip(weights.iter_minor_misbehaviors())
        .for_each(|(scores, weight)| {
            for (i, &score) in scores.iter().enumerate() {
                final_scores[i] += score * weight;
            }
        });
    // Then, multiply by each major misbehavior score which is a value of either 0
    // or 1.
    major_metric_scores.iter().for_each(|scores| {
        for (i, &score) in scores.iter().enumerate() {
            final_scores[i] *= score;
        }
    });
    // Finally, divide by the scale factor and scale to max_score
    for score in final_scores.iter_mut() {
        *score /= scale_factor;
    }
    final_scores
}

// Calculate the metric scores for a single metric's median report vector. It
// returns a vector of values between 0 and the max score for that metric.
fn median_report_single_metric_to_score(
    median_report_for_metric: &MedianMetricVec,
    metric_allowance: u64,
    metric_max: u64,
    max_metric_score: u64,
) -> Vec<u64> {
    median_report_for_metric
        .iter()
        .map(|&report| metric_to_score(report, metric_allowance, metric_max, max_metric_score))
        .collect()
}

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

// NOTE: the tests below are going to be finalized in a different PR
#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use iota_protocol_config::{ConsensusChoice, ProtocolConfig};
    use iota_types::messages_consensus::{MisbehaviorsV1, VersionedMisbehaviorReport};

    use crate::authority::authority_per_epoch_store::scorer::{
        MAX_SCORE, ParametersV1, SCALE_FACTOR, Scorer, calculate_median_report, calculate_scores_v1,
    };

    fn mock_protocol_config(consensus_choice: ConsensusChoice) -> ProtocolConfig {
        let mut config = ProtocolConfig::get_for_max_version_UNSAFE();
        config.set_consensus_choice_for_testing(consensus_choice);
        config
    }

    impl Scorer {
        fn set_reports_for_tests(
            &self,
            reports_and_authorities: &[(VersionedMisbehaviorReport, u32)],
        ) {
            for (report, authority) in reports_and_authorities.iter() {
                self.update_received_reports(*authority, report);
            }
        }
    }
    #[test]
    fn test_scorer_initialization() {
        let voting_power = vec![10, 20, 30];
        let committee_size = voting_power.len();
        let protocol_config = mock_protocol_config(ConsensusChoice::Mysticeti);

        let scorer = Scorer::new(voting_power, &protocol_config);

        assert_eq!(scorer.current_scores.len(), committee_size);
        assert_eq!(scorer.invalid_reports_count.len(), committee_size);
        assert_eq!(scorer.received_metrics.len(), committee_size);
        assert_eq!(scorer.has_not_sent_report.len(), committee_size);
    }

    #[test]
    fn test_update_invalid_reports_count() {
        let voting_power = vec![10, 20, 30];

        let protocol_config = mock_protocol_config(ConsensusChoice::Mysticeti);

        let scorer = Scorer::new(voting_power, &protocol_config);

        let authority_index = 2;

        // Before update
        assert_eq!(
            scorer.invalid_reports_count[authority_index as usize].load(Ordering::Relaxed),
            0
        );

        // Call the method
        scorer.update_invalid_reports_count(authority_index);

        // After update
        assert_eq!(
            scorer.invalid_reports_count[0_usize].load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            scorer.invalid_reports_count[1_usize].load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            scorer.invalid_reports_count[2_usize].load(Ordering::Relaxed),
            1
        );

        let authority_index = 1;
        // Call the method twice
        scorer.update_invalid_reports_count(authority_index);
        scorer.update_invalid_reports_count(authority_index);

        // After update
        assert_eq!(
            scorer.invalid_reports_count[0_usize].load(Ordering::Relaxed),
            0
        );
        assert_eq!(
            scorer.invalid_reports_count[1_usize].load(Ordering::Relaxed),
            2
        );
        assert_eq!(
            scorer.invalid_reports_count[2_usize].load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn test_update_scores() {
        let voting_power = vec![2, 5, 20];
        let protocol_config = mock_protocol_config(ConsensusChoice::Mysticeti);
        let scorer = Scorer::new(voting_power, &protocol_config);

        // Before calling update_scores, all scores should be MAX_SCORE
        for score in scorer.current_scores.iter() {
            assert_eq!(score.load(Ordering::Relaxed), MAX_SCORE,);
        }

        // Set some reports for testing
        let reports_and_authorities = vec![
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![5, 0, 0],
                    faulty_blocks_unprovable: vec![0, 0, 0],
                    missing_proposals: vec![0, 0, 0],
                    equivocations: vec![0, 0, 0],
                }),
                0_u32,
            ),
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![0, 10, 0],
                    faulty_blocks_unprovable: vec![0, 0, 0],
                    missing_proposals: vec![0, 0, 0],
                    equivocations: vec![0, 0, 0],
                }),
                1_u32,
            ),
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![0, 0, 15],
                    faulty_blocks_unprovable: vec![0, 0, 0],
                    missing_proposals: vec![0, 0, 0],
                    equivocations: vec![5, 0, 0],
                }),
                2_u32,
            ),
        ];

        scorer.set_reports_for_tests(&reports_and_authorities);

        // Call the method
        scorer.update_scores();

        let expected_score = vec![0, 65536, 45876];
        // After calling update_scores, scores should be updated
        let actual_score = scorer
            .current_scores
            .iter()
            .map(|value| value.load(Ordering::Relaxed))
            .collect::<Vec<u64>>();
        assert_eq!(actual_score, expected_score);
    }

    #[test]
    fn test_calculate_median_report() {
        let reports_and_voting_power = vec![(
            VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                faulty_blocks_provable: vec![7, 8, 9],
                faulty_blocks_unprovable: vec![10, 11, 12],
                missing_proposals: vec![4, 5, 6],
                equivocations: vec![1, 2, 3],
            }),
            10_u64,
        )];
        let median_report = calculate_median_report(&reports_and_voting_power);

        assert_eq!(
            median_report,
            MisbehaviorsV1 {
                faulty_blocks_provable: vec![7, 8, 9],
                faulty_blocks_unprovable: vec![10, 11, 12],
                missing_proposals: vec![4, 5, 6],
                equivocations: vec![1, 2, 3]
            }
        );

        let reports_and_voting_power = vec![
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![7, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 3],
                }),
                20_u64,
            ),
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![70, 80, 90],
                    faulty_blocks_unprovable: vec![100, 110, 120],
                    missing_proposals: vec![40, 50, 60],
                    equivocations: vec![10, 20, 30],
                }),
                10_u64,
            ),
        ];

        let median_report = calculate_median_report(&reports_and_voting_power);

        assert_eq!(
            median_report,
            MisbehaviorsV1 {
                faulty_blocks_provable: vec![7, 8, 9],
                faulty_blocks_unprovable: vec![10, 11, 12],
                missing_proposals: vec![4, 5, 6],
                equivocations: vec![1, 2, 3]
            }
        );

        let reports_and_voting_power = vec![
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![1, 8, 9],
                    faulty_blocks_unprovable: vec![10, 15, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 20, 3],
                }),
                10_u64,
            ),
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![7, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 0],
                }),
                10_u64,
            ),
            (
                VersionedMisbehaviorReport::new_v1(MisbehaviorsV1 {
                    faulty_blocks_provable: vec![6, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 22, 6],
                    equivocations: vec![1, 2, 30],
                }),
                10_u64,
            ),
        ];

        let median_report = calculate_median_report(&reports_and_voting_power);

        assert_eq!(
            median_report,
            MisbehaviorsV1 {
                faulty_blocks_provable: vec![6, 8, 9],
                faulty_blocks_unprovable: vec![10, 11, 12],
                missing_proposals: vec![4, 5, 6],
                equivocations: vec![1, 2, 3]
            }
        );
    }

    #[test]
    fn test_calculate_scores_v1() {
        let parameters = ParametersV1 {
            allowances: MisbehaviorsV1 {
                faulty_blocks_provable: 1,
                faulty_blocks_unprovable: 2,
                missing_proposals: 1000,
                equivocations: 0,
            },
            maximums: MisbehaviorsV1 {
                faulty_blocks_provable: 5,
                faulty_blocks_unprovable: 10,
                missing_proposals: 5000,
                equivocations: 1,
            },
            weights: MisbehaviorsV1 {
                faulty_blocks_provable: SCALE_FACTOR * 30 / 100,
                faulty_blocks_unprovable: SCALE_FACTOR * 10 / 100,
                missing_proposals: SCALE_FACTOR * 35 / 100,
                equivocations: 1,
            },
        };

        let median_reports = MisbehaviorsV1 {
            faulty_blocks_provable: vec![6, 7, 8],
            faulty_blocks_unprovable: vec![9, 10, 11],
            missing_proposals: vec![3, 4, 5],
            equivocations: vec![0, 1, 2],
        };

        let scores = calculate_scores_v1(median_reports, parameters);

        // Check that scores are calculated correctly
        assert_eq!(scores, vec![40142, 0, 0]);
    }
}
