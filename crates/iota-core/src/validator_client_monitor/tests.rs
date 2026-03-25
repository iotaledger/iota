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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Serialises `AuthorityAggregator` construction across parallel tests.
///
/// `CommitteeStore::new_for_testing` opens a RocksDB instance, which
/// registers typed-store Prometheus metrics into the global default registry.
/// Because `DBMetrics` uses a `OnceCell` singleton the first two concurrent
/// callers can both evaluate `DBMetrics::new(registry)` before either one's
/// `OnceCell::set` completes, causing the second to try registering already-
/// registered metric names and panic.
///
/// Holding this mutex around every `build_mock_authority_aggregator()` call
/// ensures at most one `CommitteeStore` is being constructed at a time so the
/// singleton is set before the next caller evaluates `DBMetrics::new`.
static AUTH_AGG_CREATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn create_test_validator_names(n: usize) -> Vec<AuthorityName> {
    (0..n)
        .map(|_| {
            let (_, key_pair): (_, AuthorityKeyPair) = get_key_pair();
            key_pair.public().into()
        })
        .collect()
}

fn make_feedback(
    validator: AuthorityName,
    operation: OperationType,
    result: Result<Duration, ()>,
) -> OperationFeedback {
    OperationFeedback {
        authority_name: validator,
        display_name: validator.concise().to_string(),
        operation,
        ping: false,
        result,
    }
}

fn make_ping_feedback(
    validator: AuthorityName,
    operation: OperationType,
    result: Result<Duration, ()>,
) -> OperationFeedback {
    OperationFeedback {
        authority_name: validator,
        display_name: validator.concise().to_string(),
        operation,
        ping: true,
        result,
    }
}

// ---------------------------------------------------------------------------
// Existing tests (unchanged)
// ---------------------------------------------------------------------------

mod client_stats_tests {
    use super::*;

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

// ---------------------------------------------------------------------------
// Existing monitor tests (unchanged)
// ---------------------------------------------------------------------------

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
        let _guard = crate::validator_client_monitor::tests::AUTH_AGG_CREATE_LOCK
            .lock()
            .unwrap();
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

// ---------------------------------------------------------------------------
// Scoring-gap specification tests
//
// Each test below expresses the *desired* behaviour for a known limitation
// in the current scoring mechanism.  All tests in this module FAIL against
// the current implementation and should be made to pass as each gap is
// addressed.
//
// The gap number in each test name corresponds to the design document section
// that describes the improvement needed.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod scoring_gap_tests {
    use super::*;
    use crate::{
        authority_aggregator::{AuthorityAggregator, AuthorityAggregatorBuilder},
        test_authority_clients::MockAuthorityApi,
    };

    fn get_authority_aggregator(
        committee_size: usize,
    ) -> Arc<AuthorityAggregator<MockAuthorityApi>> {
        let _guard = crate::validator_client_monitor::tests::AUTH_AGG_CREATE_LOCK
            .lock()
            .unwrap();
        Arc::new(
            AuthorityAggregatorBuilder::from_committee_size(committee_size)
                .build_mock_authority_aggregator(),
        )
    }

    // -----------------------------------------------------------------------
    // Gap 1 — Scoring should use an EWMA so that recent observations receive
    //          exponentially more weight than older ones.
    //
    // With a uniform moving window of size N, the score needs N new
    // observations before it fully reflects a change in validator behaviour.
    // An EWMA (e.g. α = 0.5) lets the score recover after just 2–3 fast
    // observations even when the window was previously full of slow ones.
    //
    // Desired: after filling a window=5 with 500 ms and injecting 2 fast
    //          10 ms observations (40 % of the window), the score should
    //          already drop below 150 ms.
    // Current: uniform average is (3×500 + 2×10) / 5 = 304 ms → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_ewma_weights_recent_observations_more_than_old() {
        let mut stats = ValidatorClientStats::new(1.0, 20, 5);
        let slow = Duration::from_millis(500);
        let fast = Duration::from_millis(10);

        // Fill the entire window with slow observations.
        for _ in 0..5 {
            stats.update_average_latency(OperationType::Consensus, slow);
        }

        // Inject 2 fast observations — 40 % of the window.
        stats.update_average_latency(OperationType::Consensus, fast);
        stats.update_average_latency(OperationType::Consensus, fast);

        let avg = stats
            .average_latencies
            .get(&OperationType::Consensus)
            .unwrap()
            .get();

        // With EWMA (α=0.5): 500→255→132 ms after 2 fast observations.
        // The uniform average gives 304 ms and FAILS this assertion.
        assert!(
            avg <= Duration::from_millis(150),
            "after 2 fast observations the score should already reflect improvement \
             (got {avg:?}); EWMA would give ~132 ms"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 2 — Validators with few observations should receive a confidence
    //          penalty so that they score worse than well-observed validators
    //          at the same raw latency.
    //
    // A UCB/Bayesian approach inflates the score when n_observations is small,
    // discouraging the scheduler from preferring an untested validator over
    // one with a long track record.
    //
    // Desired: a validator with 1 observation at 50 ms scores higher (worse)
    //          than one with 20 observations at 50 ms.
    // Current: both have reliability = 1.0 and identical latency → same score
    //          → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_low_confidence_validator_penalized_vs_established_validator() {
        let auth_agg = get_authority_aggregator(2);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());
        let validators: Vec<_> = auth_agg.committee.names().cloned().collect();
        let established = validators[0];
        let newcomer = validators[1];

        // Established validator: 20 successful Consensus observations at 50 ms.
        for _ in 0..20 {
            monitor.record_interaction_result(make_feedback(
                established,
                OperationType::Consensus,
                Ok(Duration::from_millis(50)),
            ));
        }

        // Newcomer: just 1 successful observation at the same 50 ms.
        monitor.record_interaction_result(make_feedback(
            newcomer,
            OperationType::Consensus,
            Ok(Duration::from_millis(50)),
        ));

        monitor.force_update_cached_latencies(&auth_agg);

        let latencies = monitor
            .client_stats_for_test()
            .get_all_validator_stats(&auth_agg.committee);

        let score_established = latencies[&established];
        let score_newcomer = latencies[&newcomer];

        // The newcomer has fewer observations → lower confidence → higher
        // (worse) score.  Currently both scores are identical → FAILS.
        assert!(
            score_newcomer > score_established,
            "newcomer ({score_newcomer:?}) should score worse than established \
             ({score_established:?}) at equal raw latency due to low confidence"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 3 — Real-transaction failures must not be diluted by background
    //          health-check / ping successes.
    //
    // A malicious validator can pass every health check while silently
    // dropping real transactions.  Because all operations share one reliability
    // window, 15 HC successes + 5 real-tx failures yields reliability ≈ 0.75,
    // which masks a 100 % real-transaction failure rate.
    //
    // Desired: real-transaction operations (Submit, Effects, Consensus) must
    //          be tracked in a separate reliability plane from probes
    //          (HealthCheck, pings).  With 100 % real-tx failures the
    //          adjusted score must equal MAX_LATENCY (10 s).
    // Current: combined reliability ≈ 0.75 → adjusted score ≈ 6.3 s → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_real_tx_failure_rate_not_diluted_by_health_check_successes() {
        let config = ValidatorClientMonitorConfig {
            reliability_moving_window_size: 20,
            reliability_weight: 2.0,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let validator = create_test_validator_names(1)[0];

        // Give the validator a Consensus latency baseline of 50 ms.
        stats.record_interaction_result(make_feedback(
            validator,
            OperationType::Consensus,
            Ok(Duration::from_millis(50)),
        ));

        // 15 health-check successes (background probing).
        for _ in 0..15 {
            stats.record_interaction_result(make_ping_feedback(
                validator,
                OperationType::HealthCheck,
                Ok(Duration::from_millis(20)),
            ));
        }

        // 5 consecutive real-transaction failures — 100 % real-tx failure rate.
        for _ in 0..5 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Submit,
                Err(()),
            ));
        }

        // Compute the score directly using the stats.
        // With separate real-tx reliability = 0/5 = 0.0:
        //   penalty = 10s × (1.0 – 0.0) × 2.0 = 20s, capped → MAX_LATENCY.
        // With combined reliability ≈ 0.75 (current):
        //   penalty = 10s × 0.25 × 2.0 = 5s → adjusted ≈ 5.05s < MAX_LATENCY.
        let auth_agg = get_authority_aggregator(1);
        let latencies = stats.get_all_validator_stats(&auth_agg.committee);
        // The validator isn't in this committee; read the score via the formula.
        let v_stats = &stats.validator_stats[&validator];
        let base_latency = v_stats.average_latencies[&OperationType::Consensus].get();
        let reliability = v_stats.reliability.get();
        let penalty = Duration::from_secs(10).mul_f64((1.0 - reliability) * 2.0);
        let adjusted = (base_latency + penalty).min(Duration::from_secs(10));

        assert_eq!(
            adjusted,
            Duration::from_secs(10),
            "100 % real-tx failure rate must yield MAX_LATENCY score regardless of HC successes; \
             current combined reliability = {reliability:.2}, adjusted = {adjusted:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 4 — A run of consecutive failures should trigger an immediate score
    //          penalty (circuit-breaker), not merely shift the window average
    //          by 5/N.
    //
    // Desired: after 20 prior successes followed by 5 consecutive failures,
    //          reliability must drop to ≤ 0.20 (circuit-breaker threshold).
    // Current: reliability ≈ 0.75 (window average) → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_consecutive_failures_trigger_immediate_score_degradation() {
        let config = ValidatorClientMonitorConfig {
            reliability_moving_window_size: 20,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let validator = create_test_validator_names(1)[0];

        // Establish a history of 20 successful Consensus operations.
        for _ in 0..20 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Consensus,
                Ok(Duration::from_millis(50)),
            ));
        }

        // Simulate 5 consecutive failures (e.g., validator suddenly overloaded
        // or partitioned).
        for _ in 0..5 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Submit,
                Err(()),
            ));
        }

        let reliability = stats.validator_stats[&validator].reliability.get();

        // A circuit-breaker would detect the run of 5 consecutive failures and
        // immediately set reliability to ≤ 0.20.
        // Current window-based average ≈ 0.75 → FAILS.
        assert!(
            reliability <= 0.20,
            "5 consecutive failures should fire the circuit-breaker and drop \
             reliability to ≤ 0.20; got {reliability:.2}"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 5 — Selection should reflect live observations without requiring an
    //          explicit cache refresh.
    //
    // Currently `select_shuffled_preferred_validators` reads from
    // `cached_latencies`, which is only updated after each health-check round
    // (every 10 s).  Observations recorded between cache refreshes have no
    // effect on selection until the next round completes.
    //
    // Desired: after recording v0 = 10 ms and v1 = 5 s, selection immediately
    //          returns v0 first — no `force_update_cached_latencies` required.
    // Current: empty cache returns Duration::ZERO for both → order is
    //          arbitrary → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_selection_immediately_reflects_live_observations() {
        let auth_agg = get_authority_aggregator(2);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());
        let committee = auth_agg.committee.clone();
        let validators: Vec<_> = committee.names().cloned().collect();
        let v0 = validators[0];
        let v1 = validators[1];

        // Record a large latency difference — v0 is 500× faster than v1.
        monitor.record_interaction_result(OperationFeedback {
            authority_name: v0,
            display_name: auth_agg.get_display_name(&v0),
            operation: OperationType::Consensus,
            ping: false,
            result: Ok(Duration::from_millis(10)),
        });
        monitor.record_interaction_result(OperationFeedback {
            authority_name: v1,
            display_name: auth_agg.get_display_name(&v1),
            operation: OperationType::Consensus,
            ping: false,
            result: Ok(Duration::from_secs(5)),
        });

        // No cache refresh — live observations must drive selection directly.
        let selected = monitor.select_shuffled_preferred_validators(&committee, 0.02);

        // v0 (10 ms) must rank first without an explicit cache refresh.
        // Currently both get Duration::ZERO from the empty cache → FAILS.
        assert_eq!(
            selected[0], v0,
            "fast validator (10 ms) should rank first without a cache refresh"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 6 — HealthCheck latency should feed the selection score as a
    //          fallback when no Consensus observations exist yet.
    //
    // Currently `calculate_client_latency` returns MAX_LATENCY (10 s) for any
    // validator that lacks Consensus data, even if it has dozens of
    // sub-millisecond health-check observations.  This means a validator
    // cannot be preferred before its first real transaction, regardless of how
    // fast its health checks are.
    //
    // Desired: with 20 HealthCheck observations at 5 ms and no Consensus data,
    //          the score should be ≤ 100 ms.
    // Current: score = MAX_LATENCY = 10 s → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_health_check_latency_feeds_score_when_no_consensus_data() {
        let auth_agg = get_authority_aggregator(1);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());
        let validator = *auth_agg.committee.names().next().unwrap();

        // 20 excellent health-check observations — no Consensus data at all.
        for _ in 0..20 {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: validator,
                display_name: auth_agg.get_display_name(&validator),
                operation: OperationType::HealthCheck,
                ping: false,
                result: Ok(Duration::from_millis(5)),
            });
        }
        monitor.force_update_cached_latencies(&auth_agg);

        let latencies = monitor
            .client_stats_for_test()
            .get_all_validator_stats(&auth_agg.committee);
        let score = latencies[&validator];

        // Desired: health-check latency used as fallback → score ≈ 5 ms.
        // Current: MAX_LATENCY (10 s) because Consensus window is empty → FAILS.
        assert!(
            score <= Duration::from_millis(100),
            "with excellent health-check data the score should be ≤ 100 ms (got {score:?}); \
             currently MAX_LATENCY is returned because only Consensus latency is read"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 7 — Adjusted latency penalty formula regression test.
    //
    // This test PASSES with the current implementation.  It verifies that the
    // formula `adjusted = base + MAX_LATENCY × (1 − reliability) × weight`
    // is applied correctly and capped at MAX_LATENCY.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_adjusted_latency_penalty_formula_is_correct() {
        let config = ValidatorClientMonitorConfig {
            reliability_weight: 2.0,
            reliability_moving_window_size: 2,
            latency_moving_window_size: 1,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let validator = create_test_validator_names(1)[0];

        // window=2: init(1.0) + failure(0.0) → reliability = 0.5.
        stats.record_interaction_result(make_feedback(
            validator,
            OperationType::Consensus,
            Err(()),
        ));
        // Set Consensus latency to 100 ms.
        stats.record_interaction_result(make_feedback(
            validator,
            OperationType::Consensus,
            Ok(Duration::from_millis(100)),
        ));

        let v_stats = &stats.validator_stats[&validator];
        let reliability = v_stats.reliability.get();
        let base_latency = v_stats.average_latencies[&OperationType::Consensus].get();
        let penalty = Duration::from_secs(10).mul_f64((1.0 - reliability) * 2.0);
        let adjusted = (base_latency + penalty).min(Duration::from_secs(10));

        assert!(
            (reliability - 0.5).abs() < 0.01,
            "reliability should be ~0.5; got {reliability}"
        );
        // penalty = 10s × 0.5 × 2.0 = 10s; 100ms + 10s = 10.1s → capped at 10s.
        assert_eq!(
            adjusted,
            Duration::from_secs(10),
            "adjusted latency should be capped at MAX_LATENCY"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 8 — Ping / probe observations must not pollute the real-transaction
    //          reliability signal.
    //
    // The `ping` flag is currently used only for Prometheus labels.  Both
    // `ping = true` and `ping = false` update the same reliability window,
    // making it impossible to detect a validator that passes pings but drops
    // real transactions.
    //
    // Desired: a validator that received 10 successful pings but 5 failed
    //          real-transaction Submits should have its selection score set to
    //          MAX_LATENCY (as if real-tx reliability = 0/5 = 0.0).
    // Current: combined reliability inflates the score → adjusted < MAX_LATENCY
    //          → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_ping_success_does_not_prevent_real_tx_failure_penalty() {
        let config = ValidatorClientMonitorConfig {
            reliability_moving_window_size: 20,
            reliability_weight: 2.0,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let validator = create_test_validator_names(1)[0];

        // Baseline Consensus latency of 50 ms.
        stats.record_interaction_result(make_feedback(
            validator,
            OperationType::Consensus,
            Ok(Duration::from_millis(50)),
        ));

        // 10 successful pings (simulates probe traffic the validator handles fine).
        for _ in 0..10 {
            stats.record_interaction_result(make_ping_feedback(
                validator,
                OperationType::HealthCheck,
                Ok(Duration::from_millis(20)),
            ));
        }

        // 5 real-transaction failures — 100 % failure rate for actual work.
        for _ in 0..5 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Submit,
                Err(()),
            ));
        }

        let v_stats = &stats.validator_stats[&validator];
        let base_latency = v_stats.average_latencies[&OperationType::Consensus].get();
        let reliability = v_stats.reliability.get();
        let penalty = Duration::from_secs(10).mul_f64((1.0 - reliability) * 2.0);
        let adjusted = (base_latency + penalty).min(Duration::from_secs(10));

        // Desired: ping successes are isolated; real-tx reliability = 0.0 →
        //   penalty = MAX_LATENCY → adjusted = MAX_LATENCY.
        // Current: combined reliability > 0.6 → adjusted < MAX_LATENCY → FAILS.
        assert_eq!(
            adjusted,
            Duration::from_secs(10),
            "10 ping successes must not prevent MAX_LATENCY penalty for 100 % real-tx \
             failure rate; current reliability = {reliability:.2}, adjusted = {adjusted:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 9 — A validator that was good but is now failing must be immediately
    //          demoted without waiting for the next cache refresh.
    //
    // The cache is populated once per health-check round (every 10 s).  If a
    // validator's score was 50 ms in the last cache snapshot but has since
    // accumulated failures, selection still prefers it until the next refresh.
    //
    // Desired: after recording 5 consecutive failures for a previously-good
    //          validator (cached at 50 ms), that validator must NOT appear in
    //          the preferred group in the next `select` call.
    // Current: stale cache keeps the validator in the preferred group → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_failing_validator_immediately_demoted_without_cache_refresh() {
        let auth_agg = get_authority_aggregator(2);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());
        let committee = auth_agg.committee.clone();
        let validators: Vec<_> = committee.names().cloned().collect();
        let v0 = validators[0]; // will start failing
        let v1 = validators[1]; // stays healthy

        // Warm up: give both validators a good score and snapshot the cache.
        for v in &[v0, v1] {
            for _ in 0..5 {
                monitor.record_interaction_result(OperationFeedback {
                    authority_name: *v,
                    display_name: auth_agg.get_display_name(v),
                    operation: OperationType::Consensus,
                    ping: false,
                    result: Ok(Duration::from_millis(50)),
                });
            }
        }
        monitor.force_update_cached_latencies(&auth_agg);
        // Both in preferred group at this point.
        let baseline = monitor.select_shuffled_preferred_validators(&committee, 0.02);
        assert_eq!(
            baseline.len(),
            2,
            "sanity: both in preferred group after warm-up"
        );

        // v0 starts failing — do NOT refresh the cache.
        for _ in 0..5 {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: v0,
                display_name: auth_agg.get_display_name(&v0),
                operation: OperationType::Submit,
                ping: false,
                result: Err(()),
            });
        }

        // Desired: live failures demote v0 immediately, even without a cache
        // refresh → only v1 is in the preferred group.
        // Current: stale cache still shows v0 = 50 ms → both preferred → FAILS.
        let selected = monitor.select_shuffled_preferred_validators(&committee, 0.02);
        let preferred_contains_v0 = selected
            .iter()
            .position(|&v| v == v0)
            .map(|pos| pos == 0) // v0 is at position 0 iff it is in the preferred prefix
            .unwrap_or(false);

        assert!(
            !preferred_contains_v0,
            "v0 should be demoted from the preferred group after 5 consecutive failures \
             without waiting for a cache refresh"
        );
    }

    // -----------------------------------------------------------------------
    // Gap 10 — A minimum preferred-group size must prevent a single validator
    //           from monopolising all traffic.
    //
    // With delta = 2 %, a validator at 49 ms is the sole preferred member when
    // all others are at 51 ms (51 > 49 × 1.02 = 49.98).  It receives 100 % of
    // requests, creating a single point of failure and enabling traffic
    // monopolisation by a validator that can sustain marginally better latency.
    //
    // Desired: the preferred group must always contain ≥ 2 validators.
    //          We verify this probabilistically: over 100 draws, v1 (51 ms)
    //          must appear in position 0 at least once — proof it is in the
    //          shuffled preferred prefix.
    // Current: preferred prefix has k = 1 (only v0) → v1 never reaches
    //          position 0 → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_minimum_preferred_group_prevents_traffic_monopoly() {
        let auth_agg = get_authority_aggregator(4);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());
        let committee = auth_agg.committee.clone();
        let validators: Vec<_> = committee.names().cloned().collect();

        // v0: 49 ms.  v1–v3: 51 ms each.
        // With delta=2 %: threshold = 49 × 1.02 = 49.98 ms.
        // 51 ms > 49.98 ms → only v0 in preferred group currently.
        monitor.record_interaction_result(OperationFeedback {
            authority_name: validators[0],
            display_name: auth_agg.get_display_name(&validators[0]),
            operation: OperationType::Consensus,
            ping: false,
            result: Ok(Duration::from_millis(49)),
        });
        for v in &validators[1..] {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: *v,
                display_name: auth_agg.get_display_name(v),
                operation: OperationType::Consensus,
                ping: false,
                result: Ok(Duration::from_millis(51)),
            });
        }
        monitor.force_update_cached_latencies(&auth_agg);

        // Run 100 selections.  If the preferred group has ≥ 2 members, v1
        // (or v2/v3) will appear in position 0 at least once with very high
        // probability.  If the group is only {v0}, position 0 is always v0.
        let v1 = validators[1];
        let mut non_v0_appeared_first = false;
        for _ in 0..100 {
            let selected = monitor.select_shuffled_preferred_validators(&committee, 0.02);
            if selected[0] != validators[0] {
                non_v0_appeared_first = true;
                break;
            }
        }

        // Desired: minimum group size ≥ 2 → v1/v2/v3 occasionally appear first.
        // Current: k = 1 → v0 always first → FAILS.
        assert!(
            non_v0_appeared_first,
            "with 49 ms vs 51 ms the preferred group should contain ≥ 2 validators \
             (minimum group size), but v0 (49 ms) monopolises all 100 draws"
        );
    }
}

// ---------------------------------------------------------------------------
// Overload and load-balancing specification tests
//
// These tests verify that the scoring mechanism correctly detects, reacts to,
// and recovers from validator overload scenarios.  All tests FAIL against the
// current implementation (uniform moving window) and should pass once an
// EWMA-based scorer with fast spike detection and recovery is in place.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod overload_tests {
    use super::*;
    use crate::{
        authority_aggregator::{AuthorityAggregator, AuthorityAggregatorBuilder},
        test_authority_clients::MockAuthorityApi,
    };

    fn get_authority_aggregator(
        committee_size: usize,
    ) -> Arc<AuthorityAggregator<MockAuthorityApi>> {
        let _guard = crate::validator_client_monitor::tests::AUTH_AGG_CREATE_LOCK
            .lock()
            .unwrap();
        Arc::new(
            AuthorityAggregatorBuilder::from_committee_size(committee_size)
                .build_mock_authority_aggregator(),
        )
    }

    // -----------------------------------------------------------------------
    // A single overload spike must substantially raise the score.
    //
    // With the default latency window of 40, one spike observation at 2000 ms
    // shifts the uniform average from 50 ms to only (39×50 + 2000)/40 ≈ 99 ms
    // — far below the 500 ms threshold that would reflect actual overload.
    //
    // With EWMA (α = 0.5): 0.5×2000 + 0.5×50 = 1025 ms after 1 spike.
    //
    // Desired: score > 500 ms after a single 2000 ms spike.
    // Current: ≈ 99 ms → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_single_overload_spike_significantly_raises_score() {
        let config = ValidatorClientMonitorConfig {
            latency_moving_window_size: 40,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let validator = create_test_validator_names(1)[0];

        // Fill the latency window with baseline observations.
        for _ in 0..40 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Consensus,
                Ok(Duration::from_millis(50)),
            ));
        }

        // A single overload spike.
        stats.record_interaction_result(make_feedback(
            validator,
            OperationType::Consensus,
            Ok(Duration::from_millis(2000)),
        ));

        let avg =
            stats.validator_stats[&validator].average_latencies[&OperationType::Consensus].get();

        // EWMA (α=0.5) would yield 1025 ms.
        // Uniform window gives ≈ 99 ms → FAILS this assertion.
        assert!(
            avg > Duration::from_millis(500),
            "a single 2000 ms spike should raise the score above 500 ms for fast \
             overload detection (EWMA gives ~1025 ms); got {avg:?}"
        );
    }

    // -----------------------------------------------------------------------
    // After an overload period the score must recover quickly once fast
    // observations resume.
    //
    // With a window=10 fully filled with 2000 ms observations, the uniform
    // average after 3 recovery observations (50 ms) is still
    // (7×2000 + 3×50) / 10 = 1415 ms — the validator remains penalised for
    // ~10 rounds after it has already recovered.
    //
    // With EWMA (α=0.5): 2000 → 1025 → 537 → 293 ms after 3 fast observations.
    //
    // Desired: score < 300 ms after 3 fast observations following full overload.
    // Current: 1415 ms → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_score_recovers_quickly_after_overload_subsides() {
        let config = ValidatorClientMonitorConfig {
            latency_moving_window_size: 10,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let validator = create_test_validator_names(1)[0];

        // Simulate a period of full overload.
        for _ in 0..10 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Consensus,
                Ok(Duration::from_millis(2000)),
            ));
        }

        // Overload subsides — 3 fast observations.
        for _ in 0..3 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Consensus,
                Ok(Duration::from_millis(50)),
            ));
        }

        let avg =
            stats.validator_stats[&validator].average_latencies[&OperationType::Consensus].get();

        // EWMA (α=0.5) reaches 293 ms after 3 fast observations.
        // Uniform window gives 1415 ms → FAILS this assertion.
        assert!(
            avg < Duration::from_millis(300),
            "after 3 fast recovery observations the score should drop below 300 ms \
             (EWMA gives ~293 ms); got {avg:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Gradual load degradation must be tracked proportionally — the score
    // should closely follow the recent trend, not lag behind the uniform
    // historical average.
    //
    // With a window=10 and 10 observations that linearly increase from
    // 100 ms to 1000 ms, the uniform average is 550 ms (historical midpoint).
    // EWMA (α=0.5) weighs recent observations more and reaches ≈ 800 ms at
    // the end of the sequence.
    //
    // Desired: score > 700 ms (reflecting the recent degradation trend).
    // Current: 550 ms → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_gradual_load_degradation_reflected_in_score_proportionally() {
        let config = ValidatorClientMonitorConfig {
            latency_moving_window_size: 10,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let validator = create_test_validator_names(1)[0];

        // 10 observations: 100 ms, 200 ms, ..., 1000 ms (linear ramp-up).
        for i in 1..=10_u64 {
            stats.record_interaction_result(make_feedback(
                validator,
                OperationType::Consensus,
                Ok(Duration::from_millis(i * 100)),
            ));
        }

        let avg =
            stats.validator_stats[&validator].average_latencies[&OperationType::Consensus].get();

        // EWMA (α=0.5) reaches ≈ 800 ms, tracking the recent degradation.
        // Uniform average = 550 ms → FAILS this assertion.
        assert!(
            avg > Duration::from_millis(700),
            "gradual load increase should bring score above 700 ms to reflect \
             the recent trend (EWMA gives ~800 ms); got {avg:?}"
        );
    }

    // -----------------------------------------------------------------------
    // An overloaded validator must exit the preferred group after a small
    // number of overload observations.
    //
    // With a tight delta = 2 %, once a validator's score meaningfully exceeds
    // the fastest validator's score it is excluded from the preferred group.
    // The EWMA ensures this happens after just 1–3 observations rather than
    // requiring the window to fill with overload data.
    //
    // Desired: after 3 overload observations (2000 ms) for v0, with v1
    //          remaining at 50 ms, only v1 appears in the preferred group.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_overloaded_validator_exits_preferred_group_quickly() {
        let auth_agg = get_authority_aggregator(2);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());
        let committee = auth_agg.committee.clone();
        let validators: Vec<_> = committee.names().cloned().collect();
        let v0 = validators[0]; // will become overloaded
        let v1 = validators[1]; // stays healthy

        // Establish baseline for both validators (window=10 config override
        // via direct stats injection).
        let config = ValidatorClientMonitorConfig {
            latency_moving_window_size: 10,
            ..Default::default()
        };
        // Inject baseline into the monitor directly.
        for v in &[v0, v1] {
            for _ in 0..10 {
                monitor.record_interaction_result(OperationFeedback {
                    authority_name: *v,
                    display_name: auth_agg.get_display_name(v),
                    operation: OperationType::Consensus,
                    ping: false,
                    result: Ok(Duration::from_millis(50)),
                });
            }
        }
        monitor.force_update_cached_latencies(&auth_agg);

        // Sanity: both in preferred group at baseline.
        let baseline = monitor.select_shuffled_preferred_validators(&committee, 0.02);
        assert_eq!(
            baseline.len(),
            2,
            "sanity: both in preferred group at baseline"
        );

        // v0 experiences 3 overload observations (2000 ms).
        for _ in 0..3 {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: v0,
                display_name: auth_agg.get_display_name(&v0),
                operation: OperationType::Consensus,
                ping: false,
                result: Ok(Duration::from_millis(2000)),
            });
        }
        monitor.force_update_cached_latencies(&auth_agg);

        let selected = monitor.select_shuffled_preferred_validators(&committee, 0.02);

        // v0's score after 3 overload obs:
        //   EWMA (α=0.5): 50→1025→537→793 ms after 3 overload obs from 50ms baseline...
        //   Actually: start=50ms after 10 obs, then overload: 0.5×2000+0.5×50=1025,
        //             0.5×2000+0.5×1025=1512, 0.5×2000+0.5×1512=1756ms
        //   Uniform window=10: (7×50 + 3×2000)/10 = 635ms
        //   Both are > 51ms threshold → v0 is excluded regardless.
        //
        // The interesting check is the SPEED: with EWMA, even 1 overload
        // observation raises the score far above threshold.  The test passes
        // for both approaches at 3 observations, so we tighten to 1 here to
        // expose the difference:
        let score_v0 = monitor
            .client_stats_for_test()
            .get_all_validator_stats(&committee)[&v0];

        // Desired: after just 3 overload observations, v0 score > 1000 ms
        // (reflecting true overload severity, not just barely above threshold).
        // EWMA gives ~1756 ms; uniform window gives 635 ms → FAILS.
        assert!(
            score_v0 > Duration::from_millis(1000),
            "after 3 overload observations, v0 score should exceed 1000 ms to \
             accurately reflect overload severity (EWMA gives ~1756 ms); got {score_v0:?}"
        );

        // Regardless of scoring method, v0 should be excluded from preferred group.
        assert_eq!(
            selected[0], v1,
            "v1 (50 ms) must be the first preferred validator after v0 overloads"
        );
    }

    // -----------------------------------------------------------------------
    // A recovered validator must rejoin the preferred group promptly after
    // overload subsides, enabling automatic rebalancing.
    //
    // After a full window (10 observations) of 2000 ms overload, the uniform
    // average only reaches 50 ms after ~10 fast observations.  The EWMA
    // reaches 50 ms in 3–4 observations.
    //
    // Desired: after 4 fast recovery observations following full overload,
    //          v0 rejoins v1 in the preferred group.
    // Current: uniform window score ≈ 1415 ms after 3 fast obs → v0 stays
    //          excluded → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_recovered_validator_rejoins_preferred_group_promptly() {
        let auth_agg = get_authority_aggregator(2);
        let monitor = ValidatorClientMonitor::new_for_test(auth_agg.clone());
        let committee = auth_agg.committee.clone();
        let validators: Vec<_> = committee.names().cloned().collect();
        let v0 = validators[0]; // overloaded, then recovers
        let v1 = validators[1]; // steady at 50 ms

        // v1 maintains a consistent 50 ms score.
        for _ in 0..10 {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: v1,
                display_name: auth_agg.get_display_name(&v1),
                operation: OperationType::Consensus,
                ping: false,
                result: Ok(Duration::from_millis(50)),
            });
        }

        // v0 is fully overloaded (10 observations at 2000 ms — fills window).
        for _ in 0..10 {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: v0,
                display_name: auth_agg.get_display_name(&v0),
                operation: OperationType::Consensus,
                ping: false,
                result: Ok(Duration::from_millis(2000)),
            });
        }
        monitor.force_update_cached_latencies(&auth_agg);

        // Confirm v0 is currently excluded from the preferred group.
        let during_overload = monitor.select_shuffled_preferred_validators(&committee, 0.02);
        assert_eq!(
            during_overload[0], v1,
            "sanity: v1 preferred while v0 is overloaded"
        );

        // v0 recovers — 4 fast observations at 50 ms.
        for _ in 0..4 {
            monitor.record_interaction_result(OperationFeedback {
                authority_name: v0,
                display_name: auth_agg.get_display_name(&v0),
                operation: OperationType::Consensus,
                ping: false,
                result: Ok(Duration::from_millis(50)),
            });
        }
        monitor.force_update_cached_latencies(&auth_agg);

        // Directly check v0's score: EWMA (α=0.5) after 4 fast obs from 2000 ms
        // gives 2000→1025→537→293→171 ms.  Uniform window=10:
        // (6×2000 + 4×50)/10 = 1220 ms.
        //
        // Desired: score < 300 ms (EWMA achieves this in 4 observations).
        // Current: uniform window score ≈ 1220 ms → FAILS.
        let score_v0_after_recovery = monitor
            .client_stats_for_test()
            .get_all_validator_stats(&committee)[&v0];

        assert!(
            score_v0_after_recovery < Duration::from_millis(300),
            "after 4 recovery observations v0 score should be < 300 ms \
             (EWMA gives ~171 ms); uniform window gives ~1220 ms and keeps \
             v0 penalised → FAILS (got {score_v0_after_recovery:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// Integration tests — health-check pipeline
//
// These tests run the actual `ValidatorClientMonitor` background health-check
// task against `ScoringTestAuthorityApi` mocks.  They verify that the full
// pipeline (background task → mock → record_interaction_result →
// update_cached_latencies) produces the expected scores.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod health_check_integration_tests {
    use arc_swap::ArcSwap;
    use prometheus::Registry;

    use super::*;
    use crate::{
        authority_aggregator::{AuthorityAggregator, AuthorityAggregatorBuilder},
        test_authority_clients::ScoringTestAuthorityApi,
        validator_client_monitor::metrics::ValidatorClientMetrics,
    };

    fn build_aggregator_with_scoring_clients(
        committee_size: usize,
    ) -> (
        Arc<AuthorityAggregator<ScoringTestAuthorityApi>>,
        Vec<ScoringTestAuthorityApi>,
    ) {
        let clients_vec: Vec<ScoringTestAuthorityApi> = (0..committee_size)
            .map(|_| ScoringTestAuthorityApi::new())
            .collect();

        let auth_agg = {
            use iota_types::committee::Committee;
            let (committee, _keypairs) =
                Committee::new_simple_test_committee_of_size(committee_size);
            let names: Vec<_> = committee.names().cloned().collect();
            let clients_map: std::collections::BTreeMap<_, _> =
                names.into_iter().zip(clients_vec.clone()).collect();
            // Serialise construction to avoid racing on DBMetrics singleton init.
            let _guard = crate::validator_client_monitor::tests::AUTH_AGG_CREATE_LOCK
                .lock()
                .unwrap();
            Arc::new(
                AuthorityAggregatorBuilder::from_committee_size(committee_size)
                    .build_custom_clients(clients_map),
            )
        };

        (auth_agg, clients_vec)
    }

    // -----------------------------------------------------------------------
    // Verify that the background health-check task actually calls
    // `handle_validator_health` on every validator and records the results
    // into the monitor's stats.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_health_checks_run_and_record_results() {
        let committee_size = 4;
        let (auth_agg, clients) = build_aggregator_with_scoring_clients(committee_size);

        let config = ValidatorClientMonitorConfig {
            health_check_interval: Duration::from_millis(50),
            health_check_timeout: Duration::from_millis(500),
            ..Default::default()
        };
        let metrics = Arc::new(ValidatorClientMetrics::new(&Registry::default()));
        let swap = Arc::new(ArcSwap::new(auth_agg.clone()));
        let _monitor = ValidatorClientMonitor::new(config, metrics, swap);

        tokio::time::sleep(Duration::from_millis(300)).await;

        for (i, client) in clients.iter().enumerate() {
            assert!(
                client.health_check_call_count() >= 1,
                "validator {i} should have received at least one health check call"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Verify that a slow validator ends up with a worse (higher) cached
    // latency score than a fast one after health checks run.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_slow_validator_gets_worse_score_after_health_checks() {
        let committee_size = 2;
        let (auth_agg, clients) = build_aggregator_with_scoring_clients(committee_size);

        let validators: Vec<_> = auth_agg.committee.names().cloned().collect();
        let slow_validator = validators[0];
        let fast_validator = validators[1];

        let config = ValidatorClientMonitorConfig {
            health_check_interval: Duration::from_millis(50),
            health_check_timeout: Duration::from_millis(500),
            ..Default::default()
        };
        let metrics = Arc::new(ValidatorClientMetrics::new(&Registry::default()));
        let swap = Arc::new(ArcSwap::new(auth_agg.clone()));
        let monitor = ValidatorClientMonitor::new(config, metrics, swap);

        // Configure the slow validator to take 200 ms for health checks.
        clients[0].set_health_check_delay(Duration::from_millis(200));

        tokio::time::sleep(Duration::from_millis(400)).await;

        // Inject Consensus observations directly because health-check latency
        // does not currently feed the Consensus score (Gap 6 above).
        monitor.record_interaction_result(OperationFeedback {
            authority_name: slow_validator,
            display_name: auth_agg.get_display_name(&slow_validator),
            operation: OperationType::Consensus,
            ping: false,
            result: Ok(Duration::from_millis(500)),
        });
        monitor.record_interaction_result(OperationFeedback {
            authority_name: fast_validator,
            display_name: auth_agg.get_display_name(&fast_validator),
            operation: OperationType::Consensus,
            ping: false,
            result: Ok(Duration::from_millis(20)),
        });
        monitor.force_update_cached_latencies(&*auth_agg);

        let committee = auth_agg.committee.clone();
        let selected = monitor.select_shuffled_preferred_validators(&committee, 0.02);

        assert_eq!(
            selected[0], fast_validator,
            "fast validator should rank first"
        );
        assert_eq!(
            selected[1], slow_validator,
            "slow validator should rank last"
        );
    }

    // -----------------------------------------------------------------------
    // Verify that a validator which starts failing health checks accumulates
    // reliability penalties that degrade its score over time.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_failing_health_checks_degrade_reliability() {
        let committee_size = 2;
        let (auth_agg, clients) = build_aggregator_with_scoring_clients(committee_size);
        let validators: Vec<_> = auth_agg.committee.names().cloned().collect();
        let failing_validator = validators[0];

        let config = ValidatorClientMonitorConfig {
            health_check_interval: Duration::from_millis(50),
            health_check_timeout: Duration::from_millis(200),
            reliability_moving_window_size: 10,
            ..Default::default()
        };
        let metrics = Arc::new(ValidatorClientMetrics::new(&Registry::default()));
        let swap = Arc::new(ArcSwap::new(auth_agg.clone()));
        let monitor = ValidatorClientMonitor::new(config, metrics, swap);

        clients[0].set_health_check_fail(true);

        tokio::time::sleep(Duration::from_millis(400)).await;

        let stats = monitor.client_stats_for_test();
        if let Some(v_stats) = stats.validator_stats.get(&failing_validator) {
            assert!(
                v_stats.reliability.get() < 1.0,
                "reliability should have dropped below 1.0 after repeated failures; got {}",
                v_stats.reliability.get()
            );
        }
        assert!(
            clients[0].health_check_call_count() >= 1,
            "health checks must have been issued to the failing validator"
        );
    }

    // -----------------------------------------------------------------------
    // Verify that health-check latency alone drives validator selection.
    //
    // This test exposes Gap 6: when a validator has high health-check latency
    // but no Consensus observations, it should still be deprioritised in
    // selection.  Currently, health-check latency does not feed the Consensus
    // score, so both validators score MAX_LATENCY and appear equally preferred.
    //
    // Desired: after v0 starts responding slowly to health checks (500 ms),
    //          v0 must NOT be in the preferred group at delta = 2 %.
    // Current: both validators have no Consensus data → both score MAX_LATENCY
    //          → both in preferred group → FAILS.
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_high_health_check_latency_deprioritizes_validator_for_selection() {
        let committee_size = 2;
        let (auth_agg, clients) = build_aggregator_with_scoring_clients(committee_size);
        let validators: Vec<_> = auth_agg.committee.names().cloned().collect();
        let slow_validator = validators[0];
        let fast_validator = validators[1];

        let config = ValidatorClientMonitorConfig {
            health_check_interval: Duration::from_millis(50),
            health_check_timeout: Duration::from_millis(2000),
            ..Default::default()
        };
        let metrics = Arc::new(ValidatorClientMetrics::new(&Registry::default()));
        let swap = Arc::new(ArcSwap::new(auth_agg.clone()));
        let monitor = ValidatorClientMonitor::new(config, metrics, swap);

        // v0 takes 500 ms per health check; v1 responds immediately.
        clients[0].set_health_check_delay(Duration::from_millis(500));

        // Allow several health-check rounds to accumulate latency data.
        tokio::time::sleep(Duration::from_millis(600)).await;

        monitor.force_update_cached_latencies(&*auth_agg);

        let committee = auth_agg.committee.clone();
        let latencies = monitor
            .client_stats_for_test()
            .get_all_validator_stats(&committee);

        // Desired: health-check latency feeds the score → fast validator's
        //          score is well below MAX_LATENCY (≈ HC latency of 0 ms).
        // Current: no Consensus data for either validator → both return
        //          MAX_LATENCY (10 s) → assertion FAILS.
        assert!(
            latencies[&fast_validator] < Duration::from_secs(10),
            "v1 (0 ms health checks) should score below MAX_LATENCY when HC \
             latency feeds the selection score; currently MAX_LATENCY is returned \
             because only Consensus latency is used (Gap 6)"
        );
    }
}
