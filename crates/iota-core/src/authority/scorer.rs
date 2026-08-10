// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use arc_swap::ArcSwap;
use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{
    MisbehaviorObservations, MisbehaviorObservationsV1, MisbehaviorObservationsV2,
};

use crate::authority::authority_per_epoch_store::report_aggregator::ReportAggregator;

/// Must match MAX_SCORE in validator_set.move in iota-framework.
pub(crate) const MAX_SCORE: u64 = u16::MAX as u64 + 1;
/// Fixed-point scale used when combining weighted minor scores before dividing
/// back down to [0, MAX_SCORE]. Chosen as 2^16 so that MAX_SCORE * SCALE_FACTOR
/// fits in a u64 without overflow.
const SCALE_FACTOR: u64 = 2_u64.pow(16);

type VotingPower = u64;

/// Published score state for the current epoch. Owns the snapshot readers see
/// and the voting power table; the actual scoring math lives in
/// `VersionedScorer` and its per-version structs (`ScorerV1`, ...).
pub struct Scoreboard {
    // Published as a single `Arc<Vec<u64>>` so readers always see a consistent
    // snapshot across all authorities (no torn reads mixing old and new scores).
    current_scores: ArcSwap<Vec<u64>>,
    // The voting power of each authority in the committee.
    voting_power: Vec<u64>,
    // The active scorer version; carries its own parameters and implementation.
    scorer: VersionedScorer,
}

impl Scoreboard {
    pub fn new(voting_power: Vec<u64>, protocol_config: &ProtocolConfig) -> Self {
        let committee_size = voting_power.len();
        let scorer = VersionedScorer::from_protocol(protocol_config);
        let current_scores = ArcSwap::from_pointee(vec![MAX_SCORE; committee_size]);

        Self {
            current_scores,
            voting_power,
            scorer,
        }
    }

    /// Recomputes all authority scores from the aggregated reports in the
    /// `ReportAggregator` and atomically publishes the new vector.
    pub(crate) fn update_scores(&self, aggregator: &ReportAggregator) {
        if let Some(scores) = self.scorer.update_scores(&self.voting_power, aggregator) {
            // Single pointer swap publishes the whole vector; checkpoint readers
            // never observe a mix of old and new scores.
            self.current_scores.store(Arc::new(scores));
        }
    }

    pub(crate) fn current_scores(&self) -> Vec<u64> {
        self.current_scores.load().as_ref().clone()
    }
}

/// Versioned scoring engine. Each variant is a self-contained struct that
/// owns its parameters and the math for the matching `MisbehaviorObservations`
/// version. `Scoreboard` delegates here and stores the result.
///
/// Per-version variants are tied to the corresponding observations variant:
/// `V1(ScorerV1)` consumes only `MisbehaviorObservationsV1`. The unwrap from
/// the `MisbehaviorObservations` enum happens here at the dispatch boundary,
/// so per-version scorer impls take typed inputs and never re-match.
enum VersionedScorer {
    V1(ScorerV1),
    V2(ScorerV2),
}

impl VersionedScorer {
    fn from_protocol(protocol_config: &ProtocolConfig) -> Self {
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => Self::V1(ScorerV1::v1_parameters()),
            Some(2) => Self::V2(ScorerV2::v2_parameters()),
            Some(version) => panic!("Unsupported scorer version {version}"),
        }
    }

    /// Computes the new score vector. Returns `None` when no reports have been
    /// received yet (the published vector is left untouched in that case).
    ///
    /// All aggregated reports match the scorer's version: the aggregator is
    /// built from the same `scorer_version` and rejects other wire variants,
    /// so the destructure below panics rather than skipping mismatches.
    fn update_scores(
        &self,
        voting_power: &[u64],
        aggregator: &ReportAggregator,
    ) -> Option<Vec<u64>> {
        let reporters = aggregator.reporters_with_voting_power(voting_power);
        if reporters.is_empty() {
            return None;
        }
        match self {
            Self::V1(scorer) => {
                let reporters_v1: Vec<(&MisbehaviorObservationsV1, VotingPower)> = reporters
                    .iter()
                    .map(|(observations, vp)| match observations.as_ref() {
                        MisbehaviorObservations::V1(o) => (o, *vp),
                        other => panic!(
                            "V1 scorer received {other:?} — the aggregator only \
                             accepts the epoch's report version"
                        ),
                    })
                    .collect();
                let median = scorer.median_report(voting_power, &reporters_v1);
                Some(scorer.score_from_median(voting_power, &median))
            }
            Self::V2(scorer) => {
                let reporters_v2: Vec<(&MisbehaviorObservationsV2, VotingPower)> = reporters
                    .iter()
                    .map(|(observations, vp)| match observations.as_ref() {
                        MisbehaviorObservations::V2(o) => (o, *vp),
                        other => panic!(
                            "V2 scorer received {other:?} — the aggregator only \
                             accepts the epoch's report version"
                        ),
                    })
                    .collect();
                let median = scorer.median_report(voting_power, &reporters_v2);
                Some(scorer.score_from_median(voting_power, &median))
            }
        }
    }
}

/// V1 scoring parameters and implementation. Field order mirrors
/// `MisbehaviorObservationsV1` so the score loop can iterate
/// `(row, params)` pairs without an indirection through `Misbehavior`.
struct ScorerV1 {
    faulty_blocks_provable: MetricParams,
    faulty_blocks_unprovable: MetricParams,
    missing_proposals: MetricParams,
    equivocations: MetricParams,
    /// `SCALE_FACTOR - sum(minor weights)`. Pre-multiplied by `MAX_SCORE` to
    /// produce the per-authority initial value before the weighted-minor
    /// accumulation loop.
    baseline_score: u64,
}

impl ScorerV1 {
    fn v1_parameters() -> Self {
        // 1 provable faulty block is allowed without punishment, to account
        // for honest mistakes / protocol edge cases.
        let faulty_blocks_provable = MetricParams {
            allowance: 1,
            maximum: 5,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 30 / 100,
            },
        };
        // 2 unprovable faulty blocks allowed without punishment; less severe
        // than provable.
        let faulty_blocks_unprovable = MetricParams {
            allowance: 2,
            maximum: 10,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 10 / 100,
            },
        };
        // ~3% of consensus rounds in an epoch allowed; ~10% leads to zero
        // score.
        let missing_proposals = MetricParams {
            allowance: 48_000,
            maximum: 160_000,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 35 / 100,
            },
        };
        // Equivocations: any occurrence collapses the score to zero.
        let equivocations = MetricParams {
            allowance: 0,
            maximum: 1,
            kind: MetricKind::Major,
        };

        let baseline_score = validate_params_and_baseline(&[
            &faulty_blocks_provable,
            &faulty_blocks_unprovable,
            &missing_proposals,
            &equivocations,
        ]);

        Self {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
            baseline_score,
        }
    }

    /// `(row in median, params)` pairs for the explicit named-field iteration
    /// used by both phases of the score formula. Adding a metric to
    /// `MisbehaviorObservationsV1` / `ScorerV1` makes this array a
    /// missing-field error, forcing the new metric to be wired in.
    fn metric_pairs<'a>(
        &'a self,
        median: &'a MisbehaviorObservationsV1,
    ) -> [(&'a [u64], &'a MetricParams); 4] {
        [
            (&median.faulty_blocks_provable, &self.faulty_blocks_provable),
            (
                &median.faulty_blocks_unprovable,
                &self.faulty_blocks_unprovable,
            ),
            (&median.missing_proposals, &self.missing_proposals),
            (&median.equivocations, &self.equivocations),
        ]
    }

    /// Calculates the weighted median across all reporters for each metric and
    /// authority. Caller is responsible for ensuring `reporters` is non-empty.
    fn median_report(
        &self,
        voting_power: &[u64],
        reporters: &[(&MisbehaviorObservationsV1, VotingPower)],
    ) -> MisbehaviorObservationsV1 {
        debug_assert!(
            !reporters.is_empty(),
            "median_report requires at least one reporter"
        );
        let committee_size = voting_power.len();
        let median_for = |select: &dyn Fn(&MisbehaviorObservationsV1) -> &[u64]| {
            let rows: Vec<(&[u64], VotingPower)> = reporters
                .iter()
                .map(|(counts, vp)| (select(counts), *vp))
                .collect();
            weighted_median_rows(committee_size, &rows)
        };

        MisbehaviorObservationsV1 {
            faulty_blocks_provable: median_for(&|c| &c.faulty_blocks_provable),
            faulty_blocks_unprovable: median_for(&|c| &c.faulty_blocks_unprovable),
            missing_proposals: median_for(&|c| &c.missing_proposals),
            equivocations: median_for(&|c| &c.equivocations),
        }
    }

    /// Given the median report, produces the per-authority final score vector.
    fn score_from_median(
        &self,
        voting_power: &[u64],
        median: &MisbehaviorObservationsV1,
    ) -> Vec<u64> {
        score_from_metric_pairs(
            &self.metric_pairs(median),
            voting_power.len(),
            self.baseline_score,
        )
    }
}

/// V2 scoring parameters and implementation for
/// `MisbehaviorObservationsV2`. Same math as `ScorerV1` with a fifth
/// category: invalid bundle parts, carved out of the V1 unprovable weight so
/// the total penalty budget stays the same. Field order mirrors
/// `MisbehaviorObservationsV2` so the score loop can iterate `(row, params)`
/// pairs without an indirection through `Misbehavior`.
struct ScorerV2 {
    faulty_blocks_provable: MetricParams,
    faulty_blocks_unprovable: MetricParams,
    missing_proposals: MetricParams,
    equivocations: MetricParams,
    invalid_bundle_parts: MetricParams,
    /// `SCALE_FACTOR - sum(minor weights)`. Pre-multiplied by `MAX_SCORE` to
    /// produce the per-authority initial value before the weighted-minor
    /// accumulation loop.
    baseline_score: u64,
}

impl ScorerV2 {
    fn v2_parameters() -> Self {
        // 1 provable faulty block is allowed without punishment, to account
        // for honest mistakes / protocol edge cases.
        let faulty_blocks_provable = MetricParams {
            allowance: 1,
            maximum: 5,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 30 / 100,
            },
        };
        // The V1 unprovable weight (10%) is split evenly between unprovable
        // block faults and invalid bundle parts; allowances and maximums stay
        // at the V1 values for both.
        let faulty_blocks_unprovable = MetricParams {
            allowance: 2,
            maximum: 10,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 5 / 100,
            },
        };
        let invalid_bundle_parts = MetricParams {
            allowance: 2,
            maximum: 10,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 5 / 100,
            },
        };
        // ~3% of consensus rounds in an epoch allowed; ~10% leads to zero
        // score.
        let missing_proposals = MetricParams {
            allowance: 48_000,
            maximum: 160_000,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 35 / 100,
            },
        };
        // Equivocations: any occurrence collapses the score to zero.
        let equivocations = MetricParams {
            allowance: 0,
            maximum: 1,
            kind: MetricKind::Major,
        };

        let baseline_score = validate_params_and_baseline(&[
            &faulty_blocks_provable,
            &faulty_blocks_unprovable,
            &missing_proposals,
            &equivocations,
            &invalid_bundle_parts,
        ]);

        Self {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
            invalid_bundle_parts,
            baseline_score,
        }
    }

    /// `(row in median, params)` pairs for the explicit named-field iteration
    /// used by both phases of the score formula. Adding a metric to
    /// `MisbehaviorObservationsV2` / `ScorerV2` makes this array a
    /// missing-field error, forcing the new metric to be wired in.
    fn metric_pairs<'a>(
        &'a self,
        median: &'a MisbehaviorObservationsV2,
    ) -> [(&'a [u64], &'a MetricParams); 5] {
        [
            (&median.faulty_blocks_provable, &self.faulty_blocks_provable),
            (
                &median.faulty_blocks_unprovable,
                &self.faulty_blocks_unprovable,
            ),
            (&median.missing_proposals, &self.missing_proposals),
            (&median.equivocations, &self.equivocations),
            (&median.invalid_bundle_parts, &self.invalid_bundle_parts),
        ]
    }

    /// Calculates the weighted median across all reporters for each metric and
    /// authority. Caller is responsible for ensuring `reporters` is non-empty.
    fn median_report(
        &self,
        voting_power: &[u64],
        reporters: &[(&MisbehaviorObservationsV2, VotingPower)],
    ) -> MisbehaviorObservationsV2 {
        debug_assert!(
            !reporters.is_empty(),
            "median_report requires at least one reporter"
        );
        let committee_size = voting_power.len();
        let median_for = |select: &dyn Fn(&MisbehaviorObservationsV2) -> &[u64]| {
            let rows: Vec<(&[u64], VotingPower)> = reporters
                .iter()
                .map(|(counts, vp)| (select(counts), *vp))
                .collect();
            weighted_median_rows(committee_size, &rows)
        };

        MisbehaviorObservationsV2 {
            faulty_blocks_provable: median_for(&|c| &c.faulty_blocks_provable),
            faulty_blocks_unprovable: median_for(&|c| &c.faulty_blocks_unprovable),
            missing_proposals: median_for(&|c| &c.missing_proposals),
            equivocations: median_for(&|c| &c.equivocations),
            invalid_bundle_parts: median_for(&|c| &c.invalid_bundle_parts),
        }
    }

    /// Given the median report, produces the per-authority final score vector.
    fn score_from_median(
        &self,
        voting_power: &[u64],
        median: &MisbehaviorObservationsV2,
    ) -> Vec<u64> {
        score_from_metric_pairs(
            &self.metric_pairs(median),
            voting_power.len(),
            self.baseline_score,
        )
    }
}

/// Init-time invariant checks for a scorer's full parameter set. Cheaper than
/// guarding every score evaluation. Returns the baseline score
/// (`SCALE_FACTOR - sum(minor weights)`).
fn validate_params_and_baseline(metrics: &[&MetricParams]) -> u64 {
    for p in metrics {
        assert!(p.allowance < p.maximum, "allowance must be < maximum");
        assert!(
            p.maximum <= u64::MAX / MAX_SCORE,
            "maximum must be <= u64::MAX / MAX_SCORE to keep arithmetic safe"
        );
        if let MetricKind::Major = p.kind {
            assert!(
                p.allowance == 0 && p.maximum == 1,
                "major metric must have allowance=0 and maximum=1 so any occurrence \
                 collapses the score to zero"
            );
        }
    }

    let minor_weights_sum: u64 = metrics
        .iter()
        .map(|p| match p.kind {
            MetricKind::Minor { weight } => weight,
            MetricKind::Major => 0,
        })
        .sum();
    assert!(
        minor_weights_sum <= SCALE_FACTOR,
        "minor weights sum ({minor_weights_sum}) exceeds SCALE_FACTOR ({SCALE_FACTOR})"
    );
    SCALE_FACTOR - minor_weights_sum
}

/// Weighted median across all reporters for one metric. `rows` pairs each
/// reporter's per-authority counts with that reporter's voting power; the
/// median for an authority is the smallest reported value whose cumulative
/// reporter voting power reaches half of the total. Caller guarantees `rows`
/// is non-empty.
fn weighted_median_rows(committee_size: usize, rows: &[(&[u64], VotingPower)]) -> Vec<u64> {
    // Sum only over reporters, not the full committee — the median is
    // weighted by the voting power of authorities that actually submitted.
    let total_voting_power: VotingPower = rows.iter().map(|(_, vp)| *vp).sum();

    // Reused across all authorities — one allocation total.
    let mut chunk: Vec<(u64, VotingPower)> = Vec::with_capacity(rows.len());

    let mut median_for_metric = Vec::with_capacity(committee_size);
    for authority in 0..committee_size {
        chunk.clear();
        chunk.extend(rows.iter().map(|(counts, vp)| (counts[authority], *vp)));
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
}

/// Maps a median report, given as `(median row, params)` pairs, to the
/// per-authority final score vector.
///
/// A score is an integer in `[0, MAX_SCORE]`. Each minor metric's score is
/// also in `[0, MAX_SCORE]`; the weights satisfy
/// `sum(minor_weights) + baseline_score = SCALE_FACTOR`, so we need
/// `MAX_SCORE * SCALE_FACTOR < 2^64` to avoid overflow (asserted at init).
/// Major metrics are multiplicative (factor 0 or 1).
fn score_from_metric_pairs(
    pairs: &[(&[u64], &MetricParams)],
    committee_size: usize,
    baseline_score: u64,
) -> Vec<u64> {
    let mut final_scores = vec![baseline_score * MAX_SCORE; committee_size];

    // Phase 1: weighted-minor accumulation. Values are in
    // [0, MAX_SCORE * SCALE_FACTOR].
    for (row, params) in pairs {
        if let MetricKind::Minor { weight } = params.kind {
            for (authority, &count) in row.iter().enumerate() {
                final_scores[authority] += params.metric_to_score(count, MAX_SCORE) * weight;
            }
        }
    }

    // Scale down to [0, MAX_SCORE].
    for score in final_scores.iter_mut() {
        *score /= SCALE_FACTOR;
    }

    // Phase 2: multiplicative major-metric gates (factor 0 or 1).
    for (row, params) in pairs {
        if matches!(params.kind, MetricKind::Major) {
            for (authority, score) in final_scores.iter_mut().enumerate() {
                *score *= params.metric_to_score(row[authority], 1);
            }
        }
    }

    final_scores
}

#[derive(Copy, Clone)]
enum MetricKind {
    /// Linear penalty between `allowance` and `maximum`, weighted into the
    /// per-authority score. Sum of all minor weights ≤ SCALE_FACTOR.
    Minor { weight: u64 },
    /// Multiplicative gate: factor is 0 if `value >= maximum`, 1 otherwise.
    Major,
}

#[derive(Copy, Clone)]
struct MetricParams {
    allowance: u64,
    maximum: u64,
    kind: MetricKind,
}

impl MetricParams {
    /// Maps a single misbehavior count to a score in `[0, max_score]`. Returns
    /// `max_score` if `value <= allowance`, `0` if `value >= maximum`, and
    /// linearly interpolates in between. `max_score` is `MAX_SCORE` for the
    /// minor phase and `1` for the major phase.
    fn metric_to_score(&self, value: u64, max_score: u64) -> u64 {
        if value <= self.allowance {
            max_score
        } else if value >= self.maximum {
            0
        } else {
            // `maximum - allowance > 0` and the multiplication staying in u64
            // are guaranteed by assertions in `ScorerV1::v1_parameters`.
            self.maximum.saturating_sub(value).saturating_mul(max_score)
                / self.maximum.saturating_sub(self.allowance)
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
    use iota_types::messages_consensus::{
        MisbehaviorObservations, MisbehaviorObservationsV1, MisbehaviorObservationsV2,
        VersionedMisbehaviorReport,
    };

    use crate::authority::authority_per_epoch_store::{
        misbehavior::MisbehaviorReportVersion,
        report_aggregator::ReportAggregator,
        scorer::{MAX_SCORE, Scoreboard, ScorerV1, ScorerV2, VotingPower},
    };

    fn mock_protocol_config() -> ProtocolConfig {
        ProtocolConfig::get_for_max_version_UNSAFE()
    }

    /// Protocol config pinned to the last protocol version with scorer
    /// version 1, to keep the V1 scoring path covered.
    fn mock_protocol_config_scorer_v1() -> ProtocolConfig {
        ProtocolConfig::get_for_version(ProtocolVersion::new(32), Chain::Testnet)
    }

    fn mock_report_version() -> MisbehaviorReportVersion {
        MisbehaviorReportVersion::from_protocol(&mock_protocol_config())
    }

    fn mock_scoreboard(voting_power: Vec<u64>) -> Scoreboard {
        Scoreboard::new(voting_power, &mock_protocol_config())
    }

    /// Test helper: pull the V1-typed reporter view out of an aggregator the
    /// same way `VersionedScorer::update_scores` does, so `ScorerV1`'s typed
    /// API can be exercised directly. Returns owned arcs so the borrowed
    /// view lives as long as needed.
    fn reporters_v1(
        arcs: &[(std::sync::Arc<MisbehaviorObservations>, VotingPower)],
    ) -> Vec<(&MisbehaviorObservationsV1, VotingPower)> {
        arcs.iter()
            .map(|(arc, vp)| match arc.as_ref() {
                MisbehaviorObservations::V1(o) => (o, *vp),
                other => panic!("expected V1 observations, got {other:?}"),
            })
            .collect()
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

    fn report_v2(raw_counts: &[Vec<u64>; 5]) -> VersionedMisbehaviorReport {
        VersionedMisbehaviorReport::new_v2(
            iota_types::base_types::AuthorityName::default(),
            0,
            observations_v2(raw_counts),
        )
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
        let scorer = mock_scoreboard(voting_power);

        let scores = scorer.current_scores();
        assert_eq!(scores.len(), committee_size);
        assert!(scores.iter().all(|&s| s == MAX_SCORE));
    }

    #[test]
    fn test_update_scores() {
        // Committee of 3, voting powers [2, 5, 20]. Runs the max-version
        // (V2) scorer.
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
        // + bundle = 16387 + 3276 + 22937 + 3276 = 45876
        let voting_power = vec![2, 5, 20];
        let committee_size = voting_power.len();
        let aggregator = mock_aggregator(committee_size);
        let scorer = mock_scoreboard(voting_power);

        assert!(scorer.current_scores().iter().all(|&s| s == MAX_SCORE));

        set_reports(
            &aggregator,
            &[
                (
                    report_v2(&[
                        vec![5, 0, 0],
                        vec![0, 0, 0],
                        vec![0, 0, 0],
                        vec![0, 0, 0],
                        vec![0, 0, 0],
                    ]),
                    0,
                ),
                (
                    report_v2(&[
                        vec![0, 10, 0],
                        vec![0, 0, 0],
                        vec![0, 0, 0],
                        vec![0, 0, 0],
                        vec![0, 0, 0],
                    ]),
                    1,
                ),
                (
                    report_v2(&[
                        vec![0, 0, 15],
                        vec![0, 0, 0],
                        vec![0, 0, 0],
                        vec![5, 0, 0],
                        vec![0, 0, 0],
                    ]),
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
    fn test_update_scores_scorer_v1() {
        // Same scenario as `test_update_scores`, on the pinned scorer-V1
        // protocol version with V1 reports.
        //
        // Authority 2's score differs only in the V1 weight split:
        // baseline + unprovable + missing = 16386 + 6553 + 22937 = 45876.
        let config = mock_protocol_config_scorer_v1();
        let voting_power = vec![2, 5, 20];
        let committee_size = voting_power.len();
        let aggregator = ReportAggregator::new(
            MisbehaviorReportVersion::from_protocol(&config),
            committee_size,
        );
        let scorer = Scoreboard::new(voting_power, &config);

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
        let scorer = ScorerV1::v1_parameters();

        // Single reporter: median equals their own report.
        {
            let aggregator = mock_aggregator(3);
            let voting_power = vec![10, 10, 10];
            aggregator.process_report(
                0,
                &report_v1(&[
                    vec![7, 8, 9],
                    vec![10, 11, 12],
                    vec![4, 5, 6],
                    vec![1, 2, 3],
                ]),
            );
            let arcs = aggregator.reporters_with_voting_power(&voting_power);
            let median = scorer.median_report(&voting_power, &reporters_v1(&arcs));
            assert_eq!(
                median,
                MisbehaviorObservationsV1 {
                    faulty_blocks_provable: vec![7, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 3],
                }
            );
        }

        // Two reporters with 2:1 voting power — the majority reporter's values win.
        // total_vp = 30, threshold = 15. Authority 0 (vp=20) always crosses the
        // threshold first.
        {
            let aggregator = mock_aggregator(3);
            let voting_power = vec![20, 10, 10];
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
            let arcs = aggregator.reporters_with_voting_power(&voting_power);
            let median = scorer.median_report(&voting_power, &reporters_v1(&arcs));
            assert_eq!(
                median,
                MisbehaviorObservationsV1 {
                    faulty_blocks_provable: vec![7, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 3],
                }
            );
        }

        // Three equal reporters — standard weighted median.
        // total_vp = 30, threshold = 15. The middle value wins for each (metric,
        // authority) pair.
        {
            let aggregator = mock_aggregator(3);
            let voting_power = vec![10, 10, 10];
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
            let arcs = aggregator.reporters_with_voting_power(&voting_power);
            let median = scorer.median_report(&voting_power, &reporters_v1(&arcs));
            assert_eq!(
                median,
                MisbehaviorObservationsV1 {
                    faulty_blocks_provable: vec![6, 8, 9],
                    faulty_blocks_unprovable: vec![10, 11, 12],
                    missing_proposals: vec![4, 5, 6],
                    equivocations: vec![1, 2, 3],
                }
            );
        }
    }

    #[test]
    fn test_score_from_median() {
        // V1 parameters:
        //   allowances:      [1, 2, 48_000, 0]
        //   maximums:        [5, 10, 160_000, 1]
        //   minor weights:   [19660, 6553, 22937]  (30%, 10%, 35% of SCALE_FACTOR)
        //   baseline_score:  16386  (SCALE_FACTOR - sum_of_minor_weights)
        //
        // All-zero misbehaviors → every authority gets MAX_SCORE (65536).
        // Derivation: (16386 + 19660 + 6553 + 22937) * MAX_SCORE / SCALE_FACTOR =
        // MAX_SCORE.
        let committee_size = 3;
        let voting_power = vec![10; committee_size];
        let scorer = ScorerV1::v1_parameters();

        assert_eq!(
            scorer.score_from_median(
                &voting_power,
                &MisbehaviorObservationsV1 {
                    faulty_blocks_provable: vec![0, 0, 0],
                    faulty_blocks_unprovable: vec![0, 0, 0],
                    missing_proposals: vec![0, 0, 0],
                    equivocations: vec![0, 0, 0],
                }
            ),
            vec![MAX_SCORE, MAX_SCORE, MAX_SCORE]
        );

        // Authority 0 equivocates (≥ max 1) → major factor = 0 → score = 0.
        assert_eq!(
            scorer.score_from_median(
                &voting_power,
                &MisbehaviorObservationsV1 {
                    faulty_blocks_provable: vec![0, 0, 0],
                    faulty_blocks_unprovable: vec![0, 0, 0],
                    missing_proposals: vec![0, 0, 0],
                    equivocations: vec![1, 0, 0],
                }
            ),
            vec![0, MAX_SCORE, MAX_SCORE]
        );

        // Authority 0 reaches provable-fault maximum (≥ 5) → provable contribution = 0.
        // score = (baseline + unprovable_weight + missing_weight) = 16386 + 6553 +
        // 22937 = 45876.
        assert_eq!(
            scorer.score_from_median(
                &voting_power,
                &MisbehaviorObservationsV1 {
                    faulty_blocks_provable: vec![5, 0, 0],
                    faulty_blocks_unprovable: vec![0, 0, 0],
                    missing_proposals: vec![0, 0, 0],
                    equivocations: vec![0, 0, 0],
                }
            ),
            vec![45876, MAX_SCORE, MAX_SCORE]
        );
    }

    fn observations_v2(raw_counts: &[Vec<u64>; 5]) -> MisbehaviorObservationsV2 {
        MisbehaviorObservationsV2 {
            faulty_blocks_provable: raw_counts[0].clone(),
            faulty_blocks_unprovable: raw_counts[1].clone(),
            missing_proposals: raw_counts[2].clone(),
            equivocations: raw_counts[3].clone(),
            invalid_bundle_parts: raw_counts[4].clone(),
        }
    }

    #[test]
    fn test_calculate_median_report_v2() {
        // Three equal reporters — the middle value wins for each (metric,
        // authority) pair, including the bundle-part row.
        let scorer = ScorerV2::v2_parameters();
        let voting_power = vec![10, 10, 10];

        let reports = [
            observations_v2(&[
                vec![1, 8, 9],
                vec![10, 15, 12],
                vec![4, 5, 6],
                vec![1, 20, 3],
                vec![2, 7, 30],
            ]),
            observations_v2(&[
                vec![7, 8, 9],
                vec![10, 11, 12],
                vec![4, 5, 6],
                vec![1, 2, 0],
                vec![2, 3, 40],
            ]),
            observations_v2(&[
                vec![6, 8, 9],
                vec![10, 11, 12],
                vec![4, 22, 6],
                vec![1, 2, 30],
                vec![9, 3, 50],
            ]),
        ];
        let reporters: Vec<(&MisbehaviorObservationsV2, VotingPower)> =
            reports.iter().map(|o| (o, 10)).collect();

        let median = scorer.median_report(&voting_power, &reporters);
        assert_eq!(
            median,
            observations_v2(&[
                vec![6, 8, 9],
                vec![10, 11, 12],
                vec![4, 5, 6],
                vec![1, 2, 3],
                vec![2, 3, 40],
            ])
        );
    }

    #[test]
    fn test_score_from_median_v2() {
        // V2 parameters:
        //   allowances:      [1, 2, 48_000, 0, 2]
        //   maximums:        [5, 10, 160_000, 1, 10]
        //   minor weights:   [19660, 3276, 22937, 3276]  (30%, 5%, 35%, 5%)
        //   baseline_score:  16387  (SCALE_FACTOR - sum_of_minor_weights)
        let committee_size = 3;
        let voting_power = vec![10; committee_size];
        let scorer = ScorerV2::v2_parameters();

        // All-zero misbehaviors → every authority gets MAX_SCORE.
        assert_eq!(
            scorer.score_from_median(
                &voting_power,
                &observations_v2(&[vec![0; 3], vec![0; 3], vec![0; 3], vec![0; 3], vec![0; 3]])
            ),
            vec![MAX_SCORE, MAX_SCORE, MAX_SCORE]
        );

        // Authority 0 reaches the bundle-part maximum (≥ 10) → its weight
        // drops out: MAX_SCORE - 3276 = 62260.
        assert_eq!(
            scorer.score_from_median(
                &voting_power,
                &observations_v2(&[
                    vec![0; 3],
                    vec![0; 3],
                    vec![0; 3],
                    vec![0; 3],
                    vec![10, 0, 0]
                ])
            ),
            vec![62260, MAX_SCORE, MAX_SCORE]
        );

        // Bundle-part counts within the allowance (≤ 2) are not punished.
        assert_eq!(
            scorer.score_from_median(
                &voting_power,
                &observations_v2(&[
                    vec![0; 3],
                    vec![0; 3],
                    vec![0; 3],
                    vec![0; 3],
                    vec![2, 0, 0]
                ])
            ),
            vec![MAX_SCORE, MAX_SCORE, MAX_SCORE]
        );

        // Equivocation still collapses the score to zero.
        assert_eq!(
            scorer.score_from_median(
                &voting_power,
                &observations_v2(&[
                    vec![0; 3],
                    vec![0; 3],
                    vec![0; 3],
                    vec![1, 0, 0],
                    vec![0; 3]
                ])
            ),
            vec![0, MAX_SCORE, MAX_SCORE]
        );
    }
}
