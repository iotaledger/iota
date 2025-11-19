// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use iota_protocol_config::ProtocolConfig;
use iota_types::{
    messages_consensus::VersionedMisbehaviorReport, scoring_metrics::VersionedScoringMetrics,
};

#[allow(unused)]
const MAX_SCORE: u64 = u64::MAX;

#[expect(dead_code)]
pub struct Scorer {
    pub(crate) current_local_metrics_count: Arc<VersionedScoringMetrics>,
    received_metrics: Vec<VersionedScoringMetrics>,
    metrics_missing_from: Vec<AtomicBool>,
    pub(crate) current_scores: Scores,
    invalid_reports_count: Vec<AtomicU64>,
    voting_power: Vec<u64>,
    version: ScorerVersion,
}

impl Scorer {
    pub fn new(voting_power: Vec<u64>, protocol_config: &ProtocolConfig) -> Self {
        let committee_size = voting_power.len();
        match protocol_config.scorer_version_as_option() {
            None | Some(1) => {
                let current_local_metrics_count = Arc::new(VersionedScoringMetrics::new(
                    committee_size,
                    protocol_config,
                ));

                let (received_metrics, metrics_missing_from, current_scores, invalid_reports_count) =
                    (0..committee_size)
                        .map(|_| {
                            (
                                VersionedScoringMetrics::new(committee_size, protocol_config),
                                AtomicBool::new(true),
                                AtomicU64::new(0),
                                AtomicU64::new(0),
                            )
                        })
                        .collect();

                Self {
                    current_local_metrics_count,
                    received_metrics,
                    metrics_missing_from,
                    current_scores,
                    invalid_reports_count,
                    voting_power,
                    version: ScorerVersion::V1,
                }
            }
            _ => panic!("Unsupported scorer version"),
        }
    }

    pub(crate) fn update_invalid_reports_count(&self, authority: u32) {
        self.invalid_reports_count[authority as usize].fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn update_scores(&self) {
        match self.version {
            ScorerVersion::V1 => self.update_scores_v1(),
        };
    }

    #[expect(dead_code)]
    pub(crate) fn update_received_reports_and_score(
        &self,
        authority: u32,
        report: &VersionedMisbehaviorReport,
    ) {
        self.received_metrics[authority as usize].update_from_report(report);
        self.metrics_missing_from[authority as usize].store(false, Ordering::Relaxed);
        self.update_scores();
    }

    fn update_scores_v1(&self) {
        // Placeholder
    }
}

pub(crate) enum ScorerVersion {
    V1,
}

pub(crate) type Scores = Vec<Score>;
pub(crate) type Score = AtomicU64;

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use iota_protocol_config::{ConsensusChoice, ProtocolConfig};

    use super::*;

    fn mock_protocol_config(consensus_choice: ConsensusChoice) -> ProtocolConfig {
        let mut config = ProtocolConfig::get_for_max_version_UNSAFE();
        config.set_consensus_choice_for_testing(consensus_choice);
        config
    }

    #[test]
    fn test_scorer_initialization() {
        let voting_power = vec![10, 20, 30];
        let committee_size = voting_power.len();
        let protocol_config = mock_protocol_config(ConsensusChoice::Mysticeti);

        let scorer = Scorer::new(voting_power, &protocol_config);

        assert_eq!(scorer.current_scores.len(), committee_size);
        assert_eq!(scorer.invalid_reports_count.len(), committee_size);

        // Add more
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
            scorer.invalid_reports_count[authority_index as usize].load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn test_update_scores() {
        let voting_power = vec![10, 20, 30];

        let protocol_config = mock_protocol_config(ConsensusChoice::Mysticeti);

        let scorer = Scorer::new(voting_power, &protocol_config);

        // Before calling update_scores, all scores should be 0
        for score in scorer.current_scores.iter() {
            assert_eq!(score.load(Ordering::Relaxed), 0);
        }

        // Add logic
    }
}
