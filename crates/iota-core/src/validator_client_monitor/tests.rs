// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{
    base_types::AuthorityName,
    crypto::{AuthorityKeyPair, KeypairTraits, get_key_pair},
};
use tokio::time::Instant;

use super::{OperationFeedback, OperationType};
use crate::validator_client_monitor::stats::ClientObservedStats;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn gen_validator() -> AuthorityName {
    let (_, kp): (_, AuthorityKeyPair) = get_key_pair();
    kp.public().into()
}

fn gen_validators(n: usize) -> Vec<AuthorityName> {
    (0..n).map(|_| gen_validator()).collect()
}

fn make_fb(
    v: AuthorityName,
    op: OperationType,
    result: Result<Duration, ()>,
    ts: Instant,
) -> OperationFeedback {
    OperationFeedback::builder(v, String::new(), op).result_at(result, ts)
}

fn now() -> Instant {
    // `Instant::now()` requires a tokio runtime when run in simtests.
    // The tests in this module only need some value without touching runtime.
    Instant::from_std(std::time::Instant::now())
}

/// Feed all 4 operations interleaved at each timestep.
///
/// Operations must be interleaved (not sequential per-op) because
/// `record_interaction_result` calls `performance_score` internally with
/// `feedback.timestamp`, querying all op EWMAs. If op B starts at t0 while
/// op A was last updated at t0+n*dt, the assert `now >= last_update` fails.
fn feed_all(
    stats: &mut ClientObservedStats,
    v: AuthorityName,
    result: Result<u64, ()>,
    n: usize,
    t0: Instant,
    dt: Duration,
) {
    for i in 0..n {
        let ts = t0 + dt * (i as u32 + 1);
        for op in [
            OperationType::Submit,
            OperationType::Effects,
            OperationType::HealthCheck,
            OperationType::Consensus,
        ] {
            let fb = make_fb(v, op, result.map(Duration::from_millis), ts);
            stats.record_interaction_result(&fb);
        }
    }
}

// ---------------------------------------------------------------------------
// Scoring: record_interaction_result return value
// ---------------------------------------------------------------------------

mod scoring {
    use super::*;

    /// Good observations (low latency, no failures) produce a lower
    /// exploitation score than failures. Lower exploitation score = better
    /// performance.
    #[test]
    fn good_observations_score_lower_than_failures() {
        let config = ValidatorClientMonitorConfig::default();
        let mut stats_ok = ClientObservedStats::new(config.clone());
        let mut stats_fail = ClientObservedStats::new(config);
        let v = gen_validator();
        let t0 = now();
        let dt = Duration::from_millis(100);

        let mut last_ok = (0.0f64, 0.0f64);
        let mut last_fail = (0.0f64, 0.0f64);
        for i in 0..10u32 {
            let ts = t0 + dt * (i + 1);
            last_ok = stats_ok.record_interaction_result(&make_fb(
                v,
                OperationType::Consensus,
                Ok(Duration::from_millis(100)),
                ts,
            ));
            last_fail = stats_fail.record_interaction_result(&make_fb(
                v,
                OperationType::Consensus,
                Err(()),
                ts,
            ));
        }

        assert!(
            last_ok.0 < last_fail.0,
            "good observations should yield lower exploitation score; ok={:.2} fail={:.2}",
            last_ok.0,
            last_fail.0
        );
    }

    /// High latency observations produce a higher exploitation score than low
    /// latency.
    #[test]
    fn high_latency_scores_worse_than_low_latency() {
        let config = ValidatorClientMonitorConfig::default();
        let mut stats_fast = ClientObservedStats::new(config.clone());
        let mut stats_slow = ClientObservedStats::new(config);
        let v = gen_validator();
        let t0 = now();
        let dt = Duration::from_millis(100);

        let mut last_fast = (0.0f64, 0.0f64);
        let mut last_slow = (0.0f64, 0.0f64);
        for i in 0..10u32 {
            let ts = t0 + dt * (i + 1);
            last_fast = stats_fast.record_interaction_result(&make_fb(
                v,
                OperationType::Submit,
                Ok(Duration::from_millis(50)),
                ts,
            ));
            last_slow = stats_slow.record_interaction_result(&make_fb(
                v,
                OperationType::Submit,
                Ok(Duration::from_millis(5000)),
                ts,
            ));
        }

        assert!(
            last_fast.0 < last_slow.0,
            "fast validator should score lower; fast={:.2} slow={:.2}",
            last_fast.0,
            last_slow.0
        );
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

mod selection {
    use super::*;

    /// Empty committee returns empty result without panicking.
    #[test]
    fn empty_committee_returns_empty() {
        let stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
        let result = stats.select_shuffled_preferred_validators(
            std::iter::empty::<&AuthorityName>(),
            now(),
            rand::thread_rng(),
        );
        assert!(result.is_empty());
    }

    /// A single validator is always returned.
    #[test]
    fn single_validator_returned() {
        let mut stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
        let v = gen_validator();
        let t0 = now();
        feed_all(&mut stats, v, Ok(100), 5, t0, Duration::from_millis(100));
        let now = t0 + Duration::from_millis(600);
        let result =
            stats.select_shuffled_preferred_validators([&v].into_iter(), now, rand::thread_rng());
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0], v);
    }

    /// All validators are returned, with the better-scoring one ranked first.
    /// With default config (exploitation_group_share=10%,
    /// exploration_group_share=10%), a 2-validator committee produces group
    /// sizes of 0, so both fall in the "rest" group that is sorted
    /// deterministically by combined score.
    #[test]
    fn better_validator_ranked_first() {
        let config = ValidatorClientMonitorConfig {
            exploitation_group_share: 1,
            exploration_group_share: 0,
            ..Default::default()
        };
        let mut stats = ClientObservedStats::new(config);
        let v_good = gen_validator();
        let v_bad = gen_validator();
        let t0 = now();
        let dt = Duration::from_millis(100);
        feed_all(&mut stats, v_good, Ok(50), 20, t0, dt);
        feed_all(&mut stats, v_bad, Ok(5000), 20, t0, dt);
        let now = t0 + dt * 21;
        let validators = [v_good, v_bad];
        let result =
            stats.select_shuffled_preferred_validators(validators.iter(), now, rand::thread_rng());
        assert_eq!(result.len(), 2);
        assert_eq!(
            *result[0], v_good,
            "faster validator should be ranked first"
        );
    }

    /// When all validators are unknown (no prior observations), all are still
    /// returned.
    #[test]
    fn all_unknown_validators_returned() {
        let stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
        let v = gen_validators(5);
        let result =
            stats.select_shuffled_preferred_validators(v.iter(), now(), rand::thread_rng());
        assert_eq!(
            result.len(),
            v.len(),
            "all unknown validators should be returned"
        );
    }
}

// ---------------------------------------------------------------------------
// Data management
// ---------------------------------------------------------------------------

mod data_management {
    use super::*;

    /// retain_validators removes validators not in the provided set.
    #[test]
    fn retain_validators_removes_stale() {
        let mut stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
        let v = gen_validators(4);
        let t0 = now();
        for vi in &v {
            feed_all(&mut stats, *vi, Ok(100), 5, t0, Duration::from_millis(100));
        }
        assert_eq!(stats.num_validators(), 4);
        stats.retain_validators(v[..2].iter());
        assert_eq!(stats.num_validators(), 2);
        assert!(stats.has_validator(&v[0]));
        assert!(stats.has_validator(&v[1]));
        assert!(!stats.has_validator(&v[2]));
        assert!(!stats.has_validator(&v[3]));
    }

    /// Each distinct validator with any recorded interaction appears exactly
    /// once.
    #[test]
    fn unique_validator_count_tracked_correctly() {
        let mut stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
        let v = gen_validators(5);
        let t0 = now();
        let dt = Duration::from_millis(100);
        // v[i] gets i+1 observations; v[4] is last updated at t0 + 5*dt.
        for (i, vi) in v.iter().enumerate() {
            feed_all(&mut stats, *vi, Ok(100), i + 1, t0, dt);
        }
        assert_eq!(stats.num_validators(), 5);
        // More observations for an existing validator must not change the count.
        // Start after t0 + 5*dt so timestamps remain non-decreasing for v[0].
        let t1 = t0 + dt * 6;
        feed_all(&mut stats, v[0], Ok(100), 10, t1, dt);
        assert_eq!(stats.num_validators(), 5);
    }
}
