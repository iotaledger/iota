// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{
    base_types::{AuthorityName, ConciseableName},
    crypto::{AuthorityKeyPair, KeypairTraits, get_key_pair},
};

use super::*;
use crate::validator_client_monitor::stats::{ClientObservedStats, ValidatorClientStats};

mod client_stats_tests {

    use super::*;

    /// Helper to create test validator names
    fn create_test_validator_names(n: usize) -> Vec<AuthorityName> {
        (0..n)
            .map(|_| {
                let (_, key_pair): (_, AuthorityKeyPair) = get_key_pair();
                key_pair.public().into()
            })
            .collect()
    }

    #[tokio::test]
    async fn test_client_stats_record_success() {
        let config = ValidatorClientMonitorConfig::default();
        let mut stats = ClientObservedStats::new(config);

        let validators = create_test_validator_names(1);
        let validator = validators[0];

        let feedback = OperationFeedback {
            authority_name: validator,
            display_name: validator.concise().to_string(),
            operation: OperationType::Submit,
            ping: false,
            result: Ok(Duration::from_millis(100)),
        };

        stats.record_interaction_result(feedback);

        let validator_stats = stats.validator_stats.get(&validator).unwrap();
        assert_eq!(validator_stats.reliability.get(), 1.0);

        let submit_latency = validator_stats
            .average_latencies
            .get(&OperationType::Submit)
            .unwrap();
        assert_eq!(submit_latency.get(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_client_stats_refresh_validator_set() {
        let config = ValidatorClientMonitorConfig::default();
        let mut stats = ClientObservedStats::new(config);

        let validators = create_test_validator_names(3);

        for validator in &validators {
            stats.record_interaction_result(OperationFeedback {
                authority_name: *validator,
                display_name: validator.concise().to_string(),
                operation: OperationType::Submit,
                ping: false,
                result: Ok(Duration::from_millis(100)),
            });
        }

        assert_eq!(stats.validator_stats.len(), 3);

        let remaining_validators: Vec<_> = validators.iter().take(2).cloned().collect();
        stats.retain_validators(&remaining_validators);

        assert_eq!(stats.validator_stats.len(), 2);
        assert!(stats.validator_stats.contains_key(&validators[0]));
        assert!(stats.validator_stats.contains_key(&validators[1]));
        assert!(!stats.validator_stats.contains_key(&validators[2]));
    }

    #[tokio::test]
    async fn test_validator_stats_update_latency() {
        let mut stats = ValidatorClientStats::new(1.0, 40, 40);

        stats.update_average_latency(OperationType::Submit, Duration::from_millis(100));
        assert_eq!(stats.average_latencies.len(), 1);
        assert_eq!(
            stats
                .average_latencies
                .get(&OperationType::Submit)
                .unwrap()
                .get(),
            Duration::from_millis(100)
        );

        stats.update_average_latency(OperationType::Submit, Duration::from_millis(200));
        let latency = stats
            .average_latencies
            .get(&OperationType::Submit)
            .unwrap()
            .get();

        // With MovingWindow: (100ms + 200ms) / 2 = 150ms
        assert_eq!(latency, Duration::from_millis(150));
    }

    #[tokio::test]
    async fn test_reliability_decay() {
        let config = ValidatorClientMonitorConfig::default();
        let mut stats = ClientObservedStats::new(config);

        let validators = create_test_validator_names(1);
        let validator = validators[0];

        stats.record_interaction_result(OperationFeedback {
            authority_name: validator,
            display_name: validator.concise().to_string(),
            operation: OperationType::Submit,
            ping: false,
            result: Ok(Duration::from_millis(100)),
        });

        let initial_reliability = stats
            .validator_stats
            .get(&validator)
            .unwrap()
            .reliability
            .get();
        assert_eq!(initial_reliability, 1.0);

        stats.record_interaction_result(OperationFeedback {
            authority_name: validator,
            display_name: validator.concise().to_string(),
            operation: OperationType::Submit,
            ping: false,
            result: Err(()),
        });

        let new_reliability = stats
            .validator_stats
            .get(&validator)
            .unwrap()
            .reliability
            .get();
        assert!((new_reliability - (2.0 / 3.0)).abs() < 1e-10);
    }
}

#[cfg(test)]
mod client_monitor_tests {
    use std::collections::HashSet;

    use super::*;
    use crate::{
        authority_aggregator::{AuthorityAggregator, AuthorityAggregatorBuilder},
        test_authority_clients::MockAuthorityApi,
    };

    fn get_authority_aggregator(
        committee_size: usize,
    ) -> Arc<AuthorityAggregator<MockAuthorityApi>> {
        Arc::new(
            AuthorityAggregatorBuilder::from_committee_size(committee_size)
                .build_mock_authority_aggregator(),
        )
    }

    #[tokio::test]
    async fn test_validator_selection_top_k_basic() {
        let auth_agg = get_authority_aggregator(4);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());

        let committee = auth_agg.committee.clone();
        let validators = committee.names().cloned().collect::<Vec<_>>();

        for (i, validator) in validators.iter().enumerate() {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: *validator,
                display_name: auth_agg.get_display_name(validator),
                operation: OperationType::Consensus,
                ping: false,
                result: Ok(Duration::from_millis((i as u64 + 1) * 50)),
            });
        }

        monitor.force_update_cached_latencies(&auth_agg);

        let selected = monitor.select_shuffled_preferred_validators(&committee, 1.0);
        assert_eq!(selected.len(), 4);

        let top_2_positions: HashSet<_> = selected.iter().take(2).cloned().collect();
        assert!(top_2_positions.contains(&validators[0]));
        assert!(top_2_positions.contains(&validators[1]));

        assert_eq!(selected[2], validators[2]);
        assert_eq!(selected[3], validators[3]);
    }
}
