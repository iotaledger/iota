// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use arc_swap::{ArcSwap, ArcSwapOption};
use iota_protocol_config::ProtocolConfig;
use iota_types::messages_consensus::{
    MisbehaviorObservations, MisbehaviorObservationsV1, MisbehaviorObservationsV2,
};

use crate::authority::authority_per_epoch_store::{
    ConsensusStats, ConsensusStatsV2, report_aggregator::ReportAggregator,
};

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
    // Set once, in the commit that completes the EndOfPublish quorum, by a
    // scorer version that scores there.
    final_scores: ArcSwapOption<Vec<u64>>,
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
            final_scores: ArcSwapOption::empty(),
            voting_power,
            scorer,
        }
    }

    /// Recomputes all authority scores from the aggregated reports in the
    /// `ReportAggregator` and atomically publishes the new vector. Scorer
    /// version 3 publishes nothing here; it scores through
    /// [`Self::finalize_scores()`].
    pub(crate) fn update_scores(&self, aggregator: &ReportAggregator) {
        if let Some(scores) = self.scorer.update_scores(&self.voting_power, aggregator) {
            // Single pointer swap publishes the whole vector; checkpoint readers
            // never observe a mix of old and new scores.
            self.current_scores.store(Arc::new(scores));
        }
    }

    /// Computes the scores of a scorer version that scores once, in the
    /// commit that completes the EndOfPublish quorum: from the reports up to
    /// and including that commit, and the consensus stats after it. `round` is
    /// the leader round of that commit. Publishes and returns the scores;
    /// returns `None` for other scorer versions.
    ///
    /// # Panics
    ///
    /// For scorer version 3, if `stats` are not `ConsensusStatsV2`, or if
    /// `round` is 0.
    pub(crate) fn finalize_scores(
        &self,
        aggregator: &ReportAggregator,
        quorum_threshold: u64,
        stats: &ConsensusStats,
        round: u64,
    ) -> Option<Vec<u64>> {
        let VersionedScorer::V3(scorer) = &self.scorer else {
            return None;
        };
        let ConsensusStats::V2(stats) = stats else {
            panic!("scorer version 3 needs the absence counts of `ConsensusStatsV2`");
        };
        let reports: Vec<Option<Arc<MisbehaviorObservations>>> = aggregator.reports();
        let reports_v2: Vec<Option<&MisbehaviorObservationsV2>> = reports
            .iter()
            .map(|report| {
                report.as_deref().map(|observations| match observations {
                    MisbehaviorObservations::V2(o) => o,
                    other @ MisbehaviorObservations::V1(_) => panic!(
                        "V3 scorer received {other:?} — the aggregator only \
                         accepts the epoch's report version"
                    ),
                })
            })
            .collect();
        let scores = scorer.final_scores(
            &self.voting_power,
            quorum_threshold,
            &reports_v2,
            stats,
            round,
        );
        self.final_scores.store(Some(Arc::new(scores.clone())));
        Some(scores)
    }

    /// Publishes final scores computed before a restart.
    pub(crate) fn restore_final_scores(&self, scores: Vec<u64>) {
        self.final_scores.store(Some(Arc::new(scores)));
    }

    /// The latest published scores: the entry at position `i` is the score of
    /// the authority whose [`Committee::authority_index()`] is `i`.
    ///
    /// [`Committee::authority_index()`]: iota_types::committee::Committee::authority_index
    pub(crate) fn current_scores(&self) -> Vec<u64> {
        match self.final_scores.load_full() {
            Some(scores) => scores.as_ref().clone(),
            None => self.current_scores.load().as_ref().clone(),
        }
    }

    /// The scores for the change-epoch transaction, in the order of
    /// [`Self::current_scores()`].
    ///
    /// # Panics
    ///
    /// If the scorer version scores in the commit that completes the
    /// EndOfPublish quorum and has not done so yet. That commit comes before
    /// the last checkpoint of the epoch.
    pub(crate) fn scores_for_change_epoch(&self) -> Vec<u64> {
        match &self.scorer {
            VersionedScorer::V3(_) => self
                .final_scores
                .load_full()
                .expect("scorer version 3 scores in the EndOfPublish quorum commit")
                .as_ref()
                .clone(),
            VersionedScorer::V1(_) | VersionedScorer::V2(_) => self.current_scores(),
        }
    }
}

/// The scores in committee member index order: the entry at position `p` is
/// the score of the validator whose committee member index is `p`. This is
/// how `validator_set::advance_epoch()` reads the `scores` vector of the
/// change-epoch transaction. `scores_by_authority_index` is as
/// [`Scoreboard::current_scores()`] returns it;
/// `committee_member_indices_by_authority_index` is as
/// [`EpochStartSystemState::authority_indices_to_committee_member_indices()`]
/// returns it.
///
/// # Panics
///
/// If `scores_by_authority_index` and
/// `committee_member_indices_by_authority_index` differ in length.
///
/// [`EpochStartSystemState::authority_indices_to_committee_member_indices()`]: iota_types::iota_system_state::epoch_start_iota_system_state::EpochStartSystemState::authority_indices_to_committee_member_indices
pub(crate) fn scores_by_committee_member_index(
    scores_by_authority_index: &[u64],
    committee_member_indices_by_authority_index: &[usize],
) -> Vec<u64> {
    assert_eq!(
        scores_by_authority_index.len(),
        committee_member_indices_by_authority_index.len(),
        "there should be one score per committee member"
    );

    // `committee_member_indices_by_authority_index` is a permutation, so
    // every entry is written exactly once.
    let mut scores_by_committee_member_index = vec![0; scores_by_authority_index.len()];
    for (&score, &committee_member_index) in scores_by_authority_index
        .iter()
        .zip(committee_member_indices_by_authority_index)
    {
        scores_by_committee_member_index[committee_member_index] = score;
    }

    scores_by_committee_member_index
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
    /// Reads `MisbehaviorObservationsV2` and scores once, through
    /// [`Scoreboard::finalize_scores()`].
    V3(ScorerV3),
}

impl VersionedScorer {
    fn from_protocol(protocol_config: &ProtocolConfig) -> Self {
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => Self::V1(ScorerV1::v1_parameters()),
            Some(2) => Self::V2(ScorerV2::v2_parameters()),
            Some(3) => Self::V3(ScorerV3::v3_parameters(u64::from(
                protocol_config.gc_depth(),
            ))),
            Some(version) => panic!("Unsupported scorer version {version}"),
        }
    }

    /// Computes the new score vector. Returns `None` when no reports have been
    /// received yet, and always for version 3, which scores only through
    /// [`Scoreboard::finalize_scores()`] (the published vector is left
    /// untouched in that case).
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
                        other @ MisbehaviorObservations::V2(_) => panic!(
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
                        other @ MisbehaviorObservations::V1(_) => panic!(
                            "V2 scorer received {other:?} — the aggregator only \
                             accepts the epoch's report version"
                        ),
                    })
                    .collect();
                let median = scorer.median_report(voting_power, &reporters_v2);
                Some(scorer.score_from_median(voting_power, &median))
            }
            // Scores only through `Scoreboard::finalize_scores()`.
            Self::V3(_) => None,
        }
    }
}

/// V1 scoring parameters and implementation. Every category is a
/// `MetricKind::Minor` except `equivocations`, the sole
/// `MetricKind::Major`. Field order mirrors `MisbehaviorObservationsV1` so
/// the score loop can iterate `(row, params)` pairs without an indirection
/// through `Misbehavior`.
struct ScorerV1 {
    faulty_blocks_provable: MetricParams,
    faulty_blocks_unprovable: MetricParams,
    missing_proposals: MetricParams,
    equivocations: MetricParams,
    /// The unallocated remainder of the weight budget
    /// (`SCALE_FACTOR - sum(minor weights)`), i.e. the fraction of `MAX_SCORE`
    /// an authority keeps even with every minor metric maxed out. Only a
    /// major metric can take the score below it.
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
    /// used by both phases of the score formula. Both structs are
    /// destructured, so adding a metric to `MisbehaviorObservationsV1` /
    /// `ScorerV1` is a missing-field error here, forcing the new metric to
    /// be wired in.
    fn metric_pairs<'a>(
        &'a self,
        median: &'a MisbehaviorObservationsV1,
    ) -> [(&'a [u64], &'a MetricParams); 4] {
        let MisbehaviorObservationsV1 {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        } = median;
        let Self {
            faulty_blocks_provable: provable_params,
            faulty_blocks_unprovable: unprovable_params,
            missing_proposals: missing_params,
            equivocations: equivocation_params,
            baseline_score: _,
        } = self;
        [
            (faulty_blocks_provable.as_slice(), provable_params),
            (faulty_blocks_unprovable.as_slice(), unprovable_params),
            (missing_proposals.as_slice(), missing_params),
            (equivocations.as_slice(), equivocation_params),
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
        let weighted_median_for = |select: &dyn Fn(&MisbehaviorObservationsV1) -> &[u64]| {
            let rows: Vec<(&[u64], VotingPower)> = reporters
                .iter()
                .map(|(counts, vp)| (select(counts), *vp))
                .collect();
            weighted_median_rows(committee_size, &rows)
        };

        MisbehaviorObservationsV1 {
            faulty_blocks_provable: weighted_median_for(&|c| &c.faulty_blocks_provable),
            faulty_blocks_unprovable: weighted_median_for(&|c| &c.faulty_blocks_unprovable),
            missing_proposals: weighted_median_for(&|c| &c.missing_proposals),
            equivocations: weighted_median_for(&|c| &c.equivocations),
        }
    }

    /// Given the median report, produces the per-authority final score vector.
    fn score_from_median(
        &self,
        voting_power: &[u64],
        median: &MisbehaviorObservationsV1,
    ) -> Vec<u64> {
        score_from_medians(
            &self.metric_pairs(median),
            voting_power.len(),
            self.baseline_score,
        )
    }
}

/// V2 scoring parameters and implementation for
/// `MisbehaviorObservationsV2`. Same math as `ScorerV1` with a fifth
/// category: invalid bundle parts, carved out of the V1 unprovable weight so
/// the total penalty budget stays the same. Every category is a
/// `MetricKind::Minor` except `equivocations`, the sole `MetricKind::Major`.
/// Field order mirrors `MisbehaviorObservationsV2` so the score loop can
/// iterate `(row, params)` pairs without an indirection through
/// `Misbehavior`.
struct ScorerV2 {
    faulty_blocks_provable: MetricParams,
    faulty_blocks_unprovable: MetricParams,
    missing_proposals: MetricParams,
    equivocations: MetricParams,
    invalid_bundle_parts: MetricParams,
    /// The unallocated remainder of the weight budget
    /// (`SCALE_FACTOR - sum(minor weights)`), i.e. the fraction of `MAX_SCORE`
    /// an authority keeps even with every minor metric maxed out. Only a
    /// major metric can take the score below it.
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
    /// used by both phases of the score formula. Both structs are
    /// destructured, so adding a metric to `MisbehaviorObservationsV2` /
    /// `ScorerV2` is a missing-field error here, forcing the new metric to
    /// be wired in.
    fn metric_pairs<'a>(
        &'a self,
        median: &'a MisbehaviorObservationsV2,
    ) -> [(&'a [u64], &'a MetricParams); 5] {
        let MisbehaviorObservationsV2 {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
            invalid_bundle_parts,
        } = median;
        let Self {
            faulty_blocks_provable: provable_params,
            faulty_blocks_unprovable: unprovable_params,
            missing_proposals: missing_params,
            equivocations: equivocation_params,
            invalid_bundle_parts: bundle_params,
            baseline_score: _,
        } = self;
        [
            (faulty_blocks_provable.as_slice(), provable_params),
            (faulty_blocks_unprovable.as_slice(), unprovable_params),
            (missing_proposals.as_slice(), missing_params),
            (equivocations.as_slice(), equivocation_params),
            (invalid_bundle_parts.as_slice(), bundle_params),
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
        let weighted_median_for = |select: &dyn Fn(&MisbehaviorObservationsV2) -> &[u64]| {
            let rows: Vec<(&[u64], VotingPower)> = reporters
                .iter()
                .map(|(counts, vp)| (select(counts), *vp))
                .collect();
            weighted_median_rows(committee_size, &rows)
        };

        MisbehaviorObservationsV2 {
            faulty_blocks_provable: weighted_median_for(&|c| &c.faulty_blocks_provable),
            faulty_blocks_unprovable: weighted_median_for(&|c| &c.faulty_blocks_unprovable),
            missing_proposals: weighted_median_for(&|c| &c.missing_proposals),
            equivocations: weighted_median_for(&|c| &c.equivocations),
            invalid_bundle_parts: weighted_median_for(&|c| &c.invalid_bundle_parts),
        }
    }

    /// Given the median report, produces the per-authority final score vector.
    fn score_from_median(
        &self,
        voting_power: &[u64],
        median: &MisbehaviorObservationsV2,
    ) -> Vec<u64> {
        score_from_medians(
            &self.metric_pairs(median),
            voting_power.len(),
            self.baseline_score,
        )
    }
}

/// Shares of the epoch's rounds are in parts per million.
const PARTS_PER_MILLION: u64 = 1_000_000;

/// V3 scoring parameters and implementation, for `MisbehaviorObservationsV2`
/// reports and the consensus stats of the commit that completes the
/// EndOfPublish quorum.
///
/// The score is the lower of the two credits for the share of rounds an
/// authority missed, times the score from the reported faults, so an
/// authority absent for the whole epoch scores 0.
struct ScorerV3 {
    /// Credit for the share of the epoch's rounds in absences, in parts per
    /// million.
    absent_rounds: Thresholds,
    /// Credit for the share of the epoch's rounds without a committed header,
    /// in parts per million.
    rounds_without_committed_header: Thresholds,
    faulty_blocks_provable: MetricParams,
    /// `faulty_blocks_unprovable` plus `invalid_bundle_parts`, summed per
    /// reporter.
    sending_peer_faults: MetricParams,
    equivocations: MetricParams,
    /// The unallocated remainder of the weight budget, as in `ScorerV1`.
    baseline_score: u64,
    /// No commit adds a header more than this many rounds below its leader
    /// round.
    gc_depth: u64,
}

impl ScorerV3 {
    fn v3_parameters(gc_depth: u64) -> Self {
        // Absent for the whole epoch gives no credit.
        let absent_rounds = Thresholds::new(PARTS_PER_MILLION * 7 / 100, PARTS_PER_MILLION);
        // Honest authorities skip some rounds, so the allowance is wider.
        let rounds_without_committed_header =
            Thresholds::new(PARTS_PER_MILLION * 20 / 100, PARTS_PER_MILLION);
        let faulty_blocks_provable = MetricParams {
            allowance: 1,
            maximum: 5,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 30 / 100,
            },
        };
        let sending_peer_faults = MetricParams {
            allowance: 2,
            maximum: 10,
            kind: MetricKind::Minor {
                weight: SCALE_FACTOR * 10 / 100,
            },
        };
        let equivocations = MetricParams {
            allowance: 0,
            maximum: 1,
            kind: MetricKind::Major,
        };

        let baseline_score = validate_params_and_baseline(&[
            &faulty_blocks_provable,
            &sending_peer_faults,
            &equivocations,
        ]);

        Self {
            absent_rounds,
            rounds_without_committed_header,
            faulty_blocks_provable,
            sending_peer_faults,
            equivocations,
            baseline_score,
            gc_depth,
        }
    }

    /// Scores each authority. `reports` holds each authority's merged report,
    /// by reporter index. `stats` are the consensus stats after the commit
    /// that completes the EndOfPublish quorum, and `round` is its leader
    /// round.
    fn final_scores(
        &self,
        voting_power: &[u64],
        quorum_threshold: u64,
        reports: &[Option<&MisbehaviorObservationsV2>],
        stats: &ConsensusStatsV2,
        round: u64,
    ) -> Vec<u64> {
        assert!(round > 0, "a commit has a leader round above 0");
        let committee_size = voting_power.len();
        let provable = quorum_values(voting_power, quorum_threshold, reports, |o, a| {
            o.faulty_blocks_provable[a]
        });
        let sending_peer = quorum_values(voting_power, quorum_threshold, reports, |o, a| {
            o.faulty_blocks_unprovable[a].saturating_add(o.invalid_bundle_parts[a])
        });
        let equivocations = quorum_values(voting_power, quorum_threshold, reports, |o, a| {
            o.equivocations[a]
        });
        let fault_scores = score_from_medians(
            &[
                (provable.as_slice(), &self.faulty_blocks_provable),
                (sending_peer.as_slice(), &self.sending_peer_faults),
                (equivocations.as_slice(), &self.equivocations),
            ],
            committee_size,
            self.baseline_score,
        );

        // No later commit can add a header at or below this round, so the
        // last gap of each authority is closed here.
        let closed_round = round.saturating_sub(self.gc_depth);
        fault_scores
            .into_iter()
            .enumerate()
            .map(|(authority, fault_score)| {
                let last_gap =
                    closed_round.saturating_sub(stats.last_committed_header_round[authority]);
                let absent_rounds = if last_gap >= stats.absent_rounds_min_gap {
                    stats.absent_rounds[authority].saturating_add(last_gap)
                } else {
                    stats.absent_rounds[authority]
                };
                let rounds_without_committed_header =
                    round.saturating_sub(stats.num_committed_header_rounds[authority]);
                let presence = self.absent_rounds.credit(share(absent_rounds, round)).min(
                    self.rounds_without_committed_header
                        .credit(share(rounds_without_committed_header, round)),
                );
                presence * fault_score / MAX_SCORE
            })
            .collect()
    }
}

/// Full credit (`MAX_SCORE`) up to `allowance`, none at `maximum`, linear in
/// between.
struct Thresholds {
    allowance: u64,
    maximum: u64,
}

impl Thresholds {
    fn new(allowance: u64, maximum: u64) -> Self {
        assert!(allowance < maximum, "allowance must be < maximum");
        assert!(
            maximum <= u64::MAX / MAX_SCORE,
            "maximum must be <= u64::MAX / MAX_SCORE to keep arithmetic safe"
        );
        Self { allowance, maximum }
    }

    fn credit(&self, value: u64) -> u64 {
        linear_credit(value, self.allowance, self.maximum, MAX_SCORE)
    }
}

/// `value` as parts per million of `total`, capped at the whole. `total` must
/// be above 0.
fn share(value: u64, total: u64) -> u64 {
    let share = u128::from(value) * u128::from(PARTS_PER_MILLION) / u128::from(total);
    share.min(u128::from(PARTS_PER_MILLION)) as u64
}

/// For each authority, the highest value that the other authorities' reports
/// about it reach with `quorum_threshold` voting power: the largest `x` such
/// that reporters holding at least that power all reported at least `x`. 0 if
/// they hold less.
fn quorum_values(
    voting_power: &[u64],
    quorum_threshold: u64,
    reports: &[Option<&MisbehaviorObservationsV2>],
    value_of: impl Fn(&MisbehaviorObservationsV2, usize) -> u64,
) -> Vec<u64> {
    (0..voting_power.len())
        .map(|subject| {
            let mut values: Vec<(u64, u64)> = reports
                .iter()
                .zip(voting_power)
                .enumerate()
                .filter(|(reporter, _)| *reporter != subject)
                .filter_map(|(_, (report, &power))| {
                    report.map(|report| (value_of(report, subject), power))
                })
                .collect();
            values.sort_unstable_by_key(|&(value, _)| std::cmp::Reverse(value));
            let mut power = 0;
            values
                .into_iter()
                .find_map(|(value, reporter_power)| {
                    power += reporter_power;
                    (power >= quorum_threshold).then_some(value)
                })
                .unwrap_or(0)
        })
        .collect()
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

/// Maps per-category `(median row, params)` entries of a median report to the
/// per-authority final score vector.
///
/// A score is an integer in `[0, MAX_SCORE]`. Each minor metric's score is
/// also in `[0, MAX_SCORE]`; the weights satisfy
/// `sum(minor_weights) + baseline_score = SCALE_FACTOR`, so we need
/// `MAX_SCORE * SCALE_FACTOR < 2^64` to avoid overflow (asserted at init).
/// Major metrics are multiplicative (factor 0 or 1).
fn score_from_medians(
    medians_with_params: &[(&[u64], &MetricParams)],
    committee_size: usize,
    baseline_score: u64,
) -> Vec<u64> {
    let mut final_scores = vec![baseline_score * MAX_SCORE; committee_size];

    // Phase 1: weighted-minor accumulation. Values are in
    // [0, MAX_SCORE * SCALE_FACTOR].
    for (row, params) in medians_with_params {
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
    for (row, params) in medians_with_params {
        if matches!(params.kind, MetricKind::Major) {
            for (authority, score) in final_scores.iter_mut().enumerate() {
                *score *= params.metric_to_score(row[authority], 1);
            }
        }
    }

    final_scores
}

/// How a misbehavior category affects the score: graded categories (minor)
/// each erode their own share of a bounded weight budget, so they can never
/// take the score to zero on their own; disqualifying offenses (major) zero
/// the score outright on any occurrence.
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
        linear_credit(value, self.allowance, self.maximum, max_score)
    }
}

/// `max_score` if `value <= allowance`, `0` if `value >= maximum`, and
/// linear in between.
fn linear_credit(value: u64, allowance: u64, maximum: u64, max_score: u64) -> u64 {
    if value <= allowance {
        max_score
    } else if value >= maximum {
        0
    } else {
        // `maximum - allowance > 0` and the multiplication staying in u64
        // are asserted where the parameters are built.
        maximum.saturating_sub(value).saturating_mul(max_score) / maximum.saturating_sub(allowance)
    }
}

#[cfg(test)]
mod tests {
    use iota_protocol_config::ProtocolConfig;
    use iota_types::messages_consensus::{
        MisbehaviorObservations, MisbehaviorObservationsV1, MisbehaviorObservationsV2,
        VersionedMisbehaviorReport,
    };

    use crate::authority::authority_per_epoch_store::{
        ConsensusStats, ConsensusStatsV2,
        misbehavior::MisbehaviorReportVersion,
        report_aggregator::ReportAggregator,
        scorer::{
            MAX_SCORE, Scoreboard, ScorerV1, ScorerV2, ScorerV3, VotingPower, quorum_values,
            scores_by_committee_member_index,
        },
    };

    /// `scores_by_committee_member_index()` places each score at the committee
    /// member index given for its authority index: here the validator with
    /// authority index 0 has committee member index 3, so its score moves from
    /// position 0 to position 3.
    #[test]
    fn scores_move_to_their_committee_member_index() {
        assert_eq!(
            scores_by_committee_member_index(&[0, 100, 200, 300], &[3, 0, 1, 2]),
            vec![100, 200, 300, 0]
        );
    }

    /// `scores_by_committee_member_index()` panics when the scores and the
    /// committee member indices differ in length: here one score is missing.
    #[test]
    #[should_panic(expected = "there should be one score per committee member")]
    fn scores_must_cover_the_committee() {
        scores_by_committee_member_index(&[0, 100, 200], &[0, 1, 2, 3]);
    }

    fn mock_protocol_config_with_scorer(version: u16) -> ProtocolConfig {
        let mut config = ProtocolConfig::get_for_max_version_UNSAFE();
        config.set_scorer_version_for_testing(version);
        config
    }

    fn mock_protocol_config() -> ProtocolConfig {
        mock_protocol_config_with_scorer(2)
    }

    fn mock_protocol_config_scorer_v1() -> ProtocolConfig {
        mock_protocol_config_with_scorer(1)
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
                other @ MisbehaviorObservations::V2(_) => {
                    panic!("expected V1 observations, got {other:?}")
                }
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
        // Committee of 3, voting powers [2, 5, 20]. Runs the V2 scorer.
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

    /// Four authorities with equal voting power: the other three reach the
    /// quorum (7500 of 6667), two of them do not.
    const V3_VOTING_POWER: [u64; 4] = [2500; 4];
    const V3_QUORUM: u64 = 6667;
    const V3_ROUND: u64 = 1000;
    const V3_GC_DEPTH: u64 = 10;
    const V3_MIN_GAP: u64 = 50;

    /// Stats in which every authority has a header in every round.
    fn v3_present_stats() -> ConsensusStatsV2 {
        let mut stats = ConsensusStatsV2::new(4, V3_MIN_GAP);
        stats.last_committed_header_round = vec![V3_ROUND; 4];
        stats.num_committed_header_rounds = vec![V3_ROUND; 4];
        stats
    }

    fn v3_scores(
        reports: &[Option<MisbehaviorObservationsV2>],
        stats: &ConsensusStatsV2,
    ) -> Vec<u64> {
        let reports: Vec<Option<&MisbehaviorObservationsV2>> =
            reports.iter().map(Option::as_ref).collect();
        ScorerV3::v3_parameters(V3_GC_DEPTH).final_scores(
            &V3_VOTING_POWER,
            V3_QUORUM,
            &reports,
            stats,
            V3_ROUND,
        )
    }

    /// A report with the given provable, unprovable, equivocation and bundle
    /// part counts about authority 0, and none about the others.
    fn v3_report_about_0(
        provable: u64,
        unprovable: u64,
        equivocations: u64,
        bundle_parts: u64,
    ) -> Option<MisbehaviorObservationsV2> {
        let about_0 = |count| vec![count, 0, 0, 0];
        Some(observations_v2(&[
            about_0(provable),
            about_0(unprovable),
            vec![0; 4],
            about_0(equivocations),
            about_0(bundle_parts),
        ]))
    }

    #[test]
    fn v3_present_authorities_without_reports_score_full() {
        assert_eq!(
            v3_scores(&[None, None, None, None], &v3_present_stats()),
            vec![MAX_SCORE; 4]
        );
    }

    #[test]
    fn v3_absent_for_the_whole_epoch_scores_zero() {
        let mut stats = v3_present_stats();
        stats.last_committed_header_round[0] = 0;
        stats.num_committed_header_rounds[0] = 0;
        assert_eq!(
            v3_scores(&[None, None, None, None], &stats),
            vec![0, MAX_SCORE, MAX_SCORE, MAX_SCORE]
        );

        // Also in an epoch too short for the last gap to close.
        let reports: Vec<Option<&MisbehaviorObservationsV2>> = vec![None; 4];
        let scores = ScorerV3::v3_parameters(V3_GC_DEPTH).final_scores(
            &V3_VOTING_POWER,
            V3_QUORUM,
            &reports,
            &ConsensusStatsV2::new(4, V3_MIN_GAP),
            V3_GC_DEPTH - 1,
        );
        assert_eq!(scores, vec![0; 4]);
    }

    #[test]
    fn v3_presence_falls_with_the_share_of_absent_rounds() {
        // Absent for half the epoch: the absence credit
        // 65536 * (1_000_000 - 500_000) / (1_000_000 - 70_000) = 35234 is
        // below the credit for rounds without a committed header,
        // 65536 * 500_000 / 800_000 = 40960.
        let mut stats = v3_present_stats();
        stats.absent_rounds[0] = 500;
        stats.num_committed_header_rounds[0] = 500;
        assert_eq!(v3_scores(&[None, None, None, None], &stats)[0], 35234);
    }

    #[test]
    fn v3_closes_the_last_gap_at_the_end_of_publish_quorum() {
        // 30 absent rounds are within the allowance of 70. A last gap of
        // `V3_MIN_GAP` rounds up to round 990 adds 50 of them; one round less
        // is too short to count.
        let mut stats = v3_present_stats();
        stats.absent_rounds[0] = 30;
        stats.num_committed_header_rounds[0] = 900;
        stats.last_committed_header_round[0] = V3_ROUND - V3_GC_DEPTH - V3_MIN_GAP;
        // 65536 * (1_000_000 - 80_000) / 930_000 = 64831.
        assert_eq!(v3_scores(&[None, None, None, None], &stats)[0], 64831);
        stats.last_committed_header_round[0] += 1;
        assert_eq!(v3_scores(&[None, None, None, None], &stats)[0], MAX_SCORE);
    }

    #[test]
    fn v3_charges_only_what_a_quorum_of_other_reporters_reached() {
        let stats = v3_present_stats();
        // Provable faults at the maximum leave the baseline and the sending
        // peer weight: (39323 + 6553) = 45876.
        let charged = 45876;

        // Two of the three other reporters are not a quorum.
        let two_high = [
            None,
            v3_report_about_0(5, 0, 0, 0),
            v3_report_about_0(5, 0, 0, 0),
            v3_report_about_0(0, 0, 0, 0),
        ];
        assert_eq!(v3_scores(&two_high, &stats)[0], MAX_SCORE);

        // All three are.
        let three_high = [
            None,
            v3_report_about_0(5, 0, 0, 0),
            v3_report_about_0(5, 0, 0, 0),
            v3_report_about_0(5, 0, 0, 0),
        ];
        assert_eq!(v3_scores(&three_high, &stats)[0], charged);

        // The authority's report about itself is ignored, so it neither
        // completes a quorum nor hides a fault.
        let mut with_self = two_high;
        with_self[0] = v3_report_about_0(5, 0, 0, 0);
        assert_eq!(v3_scores(&with_self, &stats)[0], MAX_SCORE);
        let mut hiding = three_high.clone();
        hiding[0] = v3_report_about_0(0, 0, 0, 0);
        assert_eq!(v3_scores(&hiding, &stats)[0], charged);

        // Missing reports give full credit.
        assert_eq!(
            v3_scores(&[None, three_high[1].clone(), None, None], &stats)[0],
            MAX_SCORE
        );
    }

    #[test]
    fn v3_sums_the_faults_of_the_sending_peer_per_reporter() {
        // Each reporter saw 10 faults, split differently between the two
        // fields, so only the sums reach a quorum. Faults of the sending peer
        // at the maximum leave the baseline and the provable weight:
        // 39323 + 19660 = 58983.
        let reports = [
            None,
            v3_report_about_0(0, 10, 0, 0),
            v3_report_about_0(0, 0, 0, 10),
            v3_report_about_0(0, 5, 0, 5),
        ];
        assert_eq!(v3_scores(&reports, &v3_present_stats())[0], 58983);
    }

    #[test]
    fn v3_equivocation_reached_by_a_quorum_scores_zero() {
        let stats = v3_present_stats();
        let mut reports = [
            None,
            v3_report_about_0(0, 0, 1, 0),
            v3_report_about_0(0, 0, 1, 0),
            v3_report_about_0(0, 0, 1, 0),
        ];
        assert_eq!(v3_scores(&reports, &stats)[0], 0);
        reports[3] = None;
        assert_eq!(v3_scores(&reports, &stats)[0], MAX_SCORE);
    }

    #[test]
    fn v3_scoreboard_publishes_scores_only_at_the_end_of_publish_quorum() {
        let mut config = mock_protocol_config_with_scorer(3);
        config.set_consensus_absent_rounds_min_gap_for_testing(V3_MIN_GAP);
        let scoreboard = Scoreboard::new(V3_VOTING_POWER.to_vec(), &config);
        let aggregator = ReportAggregator::new(MisbehaviorReportVersion::from_protocol(&config), 4);
        let report = VersionedMisbehaviorReport::new_v2(
            iota_types::base_types::AuthorityName::default(),
            0,
            v3_report_about_0(0, 0, 1, 0).unwrap(),
        );
        set_reports(
            &aggregator,
            &[(report.clone(), 1), (report.clone(), 2), (report, 3)],
        );

        scoreboard.update_scores(&aggregator);
        assert_eq!(scoreboard.current_scores(), vec![MAX_SCORE; 4]);

        let scores = scoreboard.finalize_scores(
            &aggregator,
            V3_QUORUM,
            &ConsensusStats::V2(v3_present_stats()),
            V3_ROUND,
        );
        let expected = vec![0, MAX_SCORE, MAX_SCORE, MAX_SCORE];
        assert_eq!(scores, Some(expected.clone()));
        assert_eq!(scoreboard.scores_for_change_epoch(), expected);
    }

    #[test]
    #[should_panic(expected = "scorer version 3 scores in the EndOfPublish quorum commit")]
    fn v3_change_epoch_needs_the_scores_of_the_end_of_publish_quorum() {
        let mut config = mock_protocol_config_with_scorer(3);
        config.set_consensus_absent_rounds_min_gap_for_testing(V3_MIN_GAP);
        Scoreboard::new(V3_VOTING_POWER.to_vec(), &config).scores_for_change_epoch();
    }

    #[test]
    fn earlier_scorers_do_not_score_at_the_end_of_publish_quorum() {
        let scoreboard = mock_scoreboard(V3_VOTING_POWER.to_vec());
        let scores = scoreboard.finalize_scores(
            &mock_aggregator(4),
            V3_QUORUM,
            &ConsensusStats::V2(v3_present_stats()),
            V3_ROUND,
        );
        assert_eq!(scores, None);
    }

    #[test]
    fn v3_quorum_value_needs_the_quorum_of_voting_power() {
        // Authority 0 is reported by authorities with 2000, 3000 and 4000
        // voting power, with 5, 3 and 1 faults.
        let voting_power = [1000, 2000, 3000, 4000];
        let observations: Vec<MisbehaviorObservationsV2> = [5, 3, 1]
            .into_iter()
            .map(|provable| {
                observations_v2(&[
                    vec![provable, 0, 0, 0],
                    vec![0; 4],
                    vec![0; 4],
                    vec![0; 4],
                    vec![0; 4],
                ])
            })
            .collect();
        let mut reports: Vec<Option<&MisbehaviorObservationsV2>> = vec![None];
        reports.extend(observations.iter().map(Some));
        let value_about_0 = |quorum| {
            quorum_values(&voting_power, quorum, &reports, |o, a| {
                o.faulty_blocks_provable[a]
            })[0]
        };
        // 2000 + 3000 reached 3.
        assert_eq!(value_about_0(5000), 3);
        // One more unit of voting power needs the third reporter.
        assert_eq!(value_about_0(5001), 1);
        // All reporters together hold 9000.
        assert_eq!(value_about_0(9000), 1);
        assert_eq!(value_about_0(9001), 0);
    }

    #[test]
    fn v3_saturates_the_sum_of_the_faults_of_the_sending_peer() {
        let reports = [
            None,
            v3_report_about_0(0, u64::MAX, 0, 1),
            v3_report_about_0(0, u64::MAX, 0, 1),
            v3_report_about_0(0, u64::MAX, 0, 1),
        ];
        assert_eq!(v3_scores(&reports, &v3_present_stats())[0], 58983);
    }

    #[test]
    fn v3_presence_counts_short_skips_through_rounds_without_a_committed_header() {
        // 300 skipped rounds in gaps too short to be absences: the credit
        // 65536 * (1_000_000 - 300_000) / 800_000 = 57344 is the lower one.
        let mut stats = v3_present_stats();
        stats.num_committed_header_rounds[0] = 700;
        assert_eq!(v3_scores(&[None, None, None, None], &stats)[0], 57344);
    }

    #[test]
    fn v3_rounds_the_presence_and_the_fault_score_separately() {
        // Presence: 71 absent rounds give 65536 * 929_000 / 930_000 = 65465.
        // Faults: 2 provable and 3 of the sending peer give
        // (39323 * 65536 + 19660 * 49152 + 6553 * 57344) / 65536 = 59801.
        // 65465 * 59801 / 65536 = 59736; one division at the end would give
        // 59737.
        let mut stats = v3_present_stats();
        stats.absent_rounds[0] = 71;
        let reports = [
            None,
            v3_report_about_0(2, 3, 0, 0),
            v3_report_about_0(2, 3, 0, 0),
            v3_report_about_0(2, 3, 0, 0),
        ];
        assert_eq!(v3_scores(&reports, &stats)[0], 59736);
    }

    #[test]
    fn earlier_scorers_pass_their_current_scores_to_the_change_epoch() {
        let scoreboard = mock_scoreboard(vec![10, 20, 30]);
        let aggregator = mock_aggregator(3);
        set_reports(
            &aggregator,
            &[(
                report_v2(&[
                    vec![0; 3],
                    vec![0; 3],
                    vec![0; 3],
                    vec![1, 0, 0],
                    vec![0; 3],
                ]),
                1,
            )],
        );
        scoreboard.update_scores(&aggregator);
        assert_eq!(
            scoreboard.scores_for_change_epoch(),
            scoreboard.current_scores()
        );
        assert_eq!(scoreboard.scores_for_change_epoch()[0], 0);
    }
}
