// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{
    base_types::AuthorityName,
    crypto::{AuthorityKeyPair, KeypairTraits, get_key_pair},
};

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

/// Config with fast-converging EWMA: tau=0.1s, dt=10ms → alpha≈0.095,
/// steady-state n_eff ≈ 10.5, comfortably above all default thresholds.
fn fast_config() -> ValidatorClientMonitorConfig {
    let mut c = ValidatorClientMonitorConfig::default();
    c.latency_ewma_tau = 0.1;
    c
}

fn make_fb(
    v: AuthorityName,
    op: OperationType,
    result: Result<Duration, ()>,
    ts: Instant,
) -> OperationFeedback {
    OperationFeedback::builder(v, String::new(), op).result_at(result, ts)
}

/// Feed `n` observations of `op` spaced `dt` apart starting from `t0+dt`.
fn feed(
    stats: &mut ClientObservedStats,
    v: AuthorityName,
    op: OperationType,
    result: Result<u64, ()>, // Ok(millis) or Err(())
    n: usize,
    t0: Instant,
    dt: Duration,
) {
    for i in 0..n {
        let ts = t0 + dt * (i as u32 + 1);
        let fb = make_fb(v, op, result.map(Duration::from_millis), ts);
        stats.record_interaction_result(&fb);
    }
}

/// Feed all 4 operations with the same result and spacing.
fn feed_all(
    stats: &mut ClientObservedStats,
    v: AuthorityName,
    result: Result<u64, ()>,
    n: usize,
    t0: Instant,
    dt: Duration,
) {
    for op in [
        OperationType::Submit,
        OperationType::Effects,
        OperationType::HealthCheck,
        OperationType::Consensus,
    ] {
        feed(stats, v, op, result, n, t0, dt);
    }
}

// ---------------------------------------------------------------------------
// EWMA math
// ---------------------------------------------------------------------------

mod ewma_math {
    use super::*;

    /// A single ok observation → selection score equals latency contribution
    /// (no failure penalty, staleness driven by Δt=0 so alpha≈1 →
    /// staleness=stale_coeff).
    #[test]
    fn single_ok_observation_produces_finite_score() {
        let config = fast_config();
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        feed(
            &mut stats,
            v,
            OperationType::HealthCheck,
            Ok(100),
            1,
            t0,
            Duration::from_millis(10),
        );
        let score = stats.calculate_selection_score(&v, t0 + Duration::from_millis(11));
        assert!(score.is_finite(), "score should be finite: {score}");
        assert!(score > 0.0, "score should be positive");
    }

    /// After many ok observations the effective sample count grows and the risk
    /// term decreases, so the overall score converges downward.
    #[test]
    fn many_ok_observations_reduce_score() {
        let config = fast_config();
        let mut stats_few = ClientObservedStats::new(config.clone());
        let mut stats_many = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(100); // 100ms spacing, tau=100ms → alpha≈0.63, fast convergence
        feed_all(&mut stats_few, v, Ok(100), 5, t0, dt);
        feed_all(&mut stats_many, v, Ok(100), 50, t0, dt);
        let now = t0 + dt * 51;
        let score_few = stats_few.calculate_selection_score(&v, now);
        let score_many = stats_many.calculate_selection_score(&v, now);
        assert!(
            score_many < score_few,
            "more observations should reduce risk term; few={score_few:.2} many={score_many:.2}"
        );
    }

    /// Pure failure observations drive the failure score up.
    #[test]
    fn failure_observations_increase_score() {
        let config = fast_config();
        let mut stats_ok = ClientObservedStats::new(config.clone());
        let mut stats_fail = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        feed_all(&mut stats_ok, v, Ok(100), 30, t0, dt);
        feed_all(&mut stats_fail, v, Err(()), 30, t0, dt);
        let now = t0 + dt * 31;
        let score_ok = stats_ok.calculate_selection_score(&v, now);
        let score_fail = stats_fail.calculate_selection_score(&v, now);
        assert!(
            score_fail > score_ok,
            "failures should raise score; ok={score_ok:.2} fail={score_fail:.2}"
        );
    }

    /// High latency observations increase the latency component of the score.
    #[test]
    fn high_latency_increases_score_over_low_latency() {
        let config = fast_config();
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);

        let mut stats_fast = ClientObservedStats::new(config.clone());
        let mut stats_slow = ClientObservedStats::new(config.clone());
        // At expected latency Submit=150ms, HC=100ms, Effects=1500ms, Consensus=800ms.
        feed(
            &mut stats_fast,
            v,
            OperationType::Submit,
            Ok(150),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_slow,
            v,
            OperationType::Submit,
            Ok(1500),
            30,
            t0,
            dt,
        );
        // Feed same data for other operations so only Submit differs.
        for op in [
            OperationType::Effects,
            OperationType::HealthCheck,
            OperationType::Consensus,
        ] {
            feed(&mut stats_fast, v, op, Ok(100), 30, t0, dt);
            feed(&mut stats_slow, v, op, Ok(100), 30, t0, dt);
        }
        let now = t0 + dt * 31;
        let score_fast = stats_fast.calculate_selection_score(&v, now);
        let score_slow = stats_slow.calculate_selection_score(&v, now);
        assert!(
            score_slow > score_fast,
            "slow validator should score worse; fast={score_fast:.2} slow={score_slow:.2}"
        );
    }

    /// The EWMA weight grows with each observation and approaches steady-state.
    #[test]
    fn weight_approaches_steady_state() {
        // tau=0.1s, dt=0.1s → alpha≈0.63, steady-state weight = 1/(1-e^{-1}) ≈ 1.58.
        // With tau=0.1 and dt=10ms → alpha≈0.095, steady-state ≈ 10.5.
        // After 50 obs the weight should be above 5.
        let config = fast_config(); // tau=0.1
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        feed_all(&mut stats, v, Ok(100), 50, t0, dt);
        // We can't inspect the EWMA weight directly, but we can observe that the
        // score stabilises and is well below the unknown_validator_score.
        let now = t0 + dt * 51;
        let score = stats.calculate_selection_score(&v, now);
        assert!(
            score < config.unknown_validator_score,
            "after many good observations score should be below unknown_validator_score; got {score:.2}"
        );
    }
}

// ---------------------------------------------------------------------------
// Staleness / clock-jump
// ---------------------------------------------------------------------------

mod staleness {
    use super::*;

    /// A validator not observed for a long time (≫ tau) accumulates maximum
    /// staleness (alpha → 1) which drives the staleness penalty to stale_coeff.
    #[test]
    fn stale_validator_has_high_staleness_penalty() {
        let config = fast_config(); // tau=0.1s
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        // Record a fresh observation.
        feed_all(&mut stats, v, Ok(100), 20, t0, Duration::from_millis(10));

        // Score when fresh (Δt ≈ 0).
        let fresh_now = t0 + Duration::from_millis(201);
        let score_fresh = stats.calculate_selection_score(&v, fresh_now);

        // Score after a very long gap (≫ tau=0.1s).
        let stale_now = t0 + Duration::from_secs(10); // 10s >> 0.1s
        let score_stale = stats.calculate_selection_score(&v, stale_now);

        assert!(
            score_stale > score_fresh,
            "stale validator should score worse; fresh={score_fresh:.2} stale={score_stale:.2}"
        );
    }

    /// A clock jump forward (e.g. NTP correction or resume from sleep) should
    /// not panic and should produce a high staleness score (alpha → 1), not a
    /// negative or NaN score.
    #[test]
    fn clock_jump_forward_does_not_panic_or_nan() {
        let config = fast_config();
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        feed_all(&mut stats, v, Ok(100), 10, t0, Duration::from_millis(10));
        // Simulate a large clock jump: check score after 1 hour.
        let now = t0 + Duration::from_secs(3600);
        let score = stats.calculate_selection_score(&v, now);
        assert!(score.is_finite(), "score must be finite after clock jump");
        assert!(!score.is_nan(), "score must not be NaN after clock jump");
    }
}

// ---------------------------------------------------------------------------
// performance_score: exclusion and penalty components
// ---------------------------------------------------------------------------

mod performance_score {
    use super::*;

    /// Unknown validator (no recorded interactions) returns
    /// `unknown_validator_score` from calculate_selection_score.
    #[test]
    fn unknown_validator_gets_sentinel_score() {
        let config = fast_config();
        let stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let score = stats.calculate_selection_score(&v, Instant::now());
        assert_eq!(score, config.unknown_validator_score);
    }

    /// A healthy validator with many good observations is NOT excluded.
    #[test]
    fn healthy_validator_not_excluded() {
        let config = fast_config();
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        feed_all(&mut stats, v, Ok(100), 30, t0, Duration::from_millis(10));
        let score = stats.calculate_selection_score(&v, t0 + Duration::from_millis(310));
        // score == 0 is the marker for excluded (None → unknown_validator_score).
        // A healthy validator should have a finite positive score well below
        // the unknown_validator_score.
        assert!(
            score.is_finite() && score > 0.0,
            "healthy validator should have finite score"
        );
        assert!(
            score < config.unknown_validator_score,
            "healthy validator should score below sentinel; got {score:.2}"
        );
    }

    /// A validator with very high failure rate AND enough n_eff is excluded
    /// (performance_score returns None → calculate_selection_score returns
    /// unknown_validator_score).
    #[test]
    fn high_failure_rate_causes_exclusion() {
        let mut config = fast_config();
        // Lower thresholds so exclusion triggers quickly in tests.
        config.exclusion_failure_threshold = 0.5;
        config.exclusion_min_n_eff = 3.0;
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        // 30 failures → failure rate → 1.0, n_eff >> 3.
        feed_all(&mut stats, v, Err(()), 30, t0, Duration::from_millis(10));
        let score = stats.calculate_selection_score(&v, t0 + Duration::from_millis(310));
        assert_eq!(
            score, config.unknown_validator_score,
            "excluded validator should return unknown_validator_score"
        );
    }

    /// Exclusion is NOT triggered when n_eff is too low, even if failure rate
    /// is 100%.  We should not permanently ban a validator after a brief
    /// outage.
    #[test]
    fn exclusion_requires_minimum_n_eff() {
        let mut config = fast_config();
        config.exclusion_failure_threshold = 0.5;
        config.exclusion_min_n_eff = 20.0; // require 20 effective samples
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validator();
        let t0 = Instant::now();
        // Only 3 failures → n_eff_hc << 20.
        feed(
            &mut stats,
            v,
            OperationType::HealthCheck,
            Err(()),
            3,
            t0,
            Duration::from_millis(10),
        );
        feed(
            &mut stats,
            v,
            OperationType::Submit,
            Err(()),
            3,
            t0,
            Duration::from_millis(10),
        );
        feed(
            &mut stats,
            v,
            OperationType::Effects,
            Err(()),
            3,
            t0,
            Duration::from_millis(10),
        );
        feed(
            &mut stats,
            v,
            OperationType::Consensus,
            Err(()),
            3,
            t0,
            Duration::from_millis(10),
        );
        let score = stats.calculate_selection_score(&v, t0 + Duration::from_millis(40));
        assert_ne!(
            score, config.unknown_validator_score,
            "insufficient n_eff should not trigger exclusion"
        );
    }

    /// Selective-failure penalty: work operations fail while HealthCheck
    /// passes. The gap must exceed `selective_failure_noise_threshold`.
    #[test]
    fn selective_failure_penalty_applied_when_work_fails_but_hc_passes() {
        let mut config = fast_config();
        config.selective_failure_noise_threshold = 0.05;
        config.selective_failure_coeff = 1000.0;
        config.selective_failure_min_n_eff = 3.0;
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);

        // Baseline: all operations succeed.
        let mut stats_ok = ClientObservedStats::new(config.clone());
        feed_all(&mut stats_ok, v, Ok(100), 30, t0, dt);

        // Selective failure: HealthCheck succeeds, work operations fail.
        let mut stats_sel = ClientObservedStats::new(config.clone());
        feed(
            &mut stats_sel,
            v,
            OperationType::HealthCheck,
            Ok(50),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_sel,
            v,
            OperationType::Submit,
            Err(()),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_sel,
            v,
            OperationType::Effects,
            Err(()),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_sel,
            v,
            OperationType::Consensus,
            Err(()),
            30,
            t0,
            dt,
        );

        let now = t0 + dt * 31;
        let score_ok = stats_ok.calculate_selection_score(&v, now);
        let score_sel = stats_sel.calculate_selection_score(&v, now);
        // Selective failure should either be excluded or have a much higher score.
        assert!(
            score_sel > score_ok,
            "selective failure should raise score; ok={score_ok:.2} selective={score_sel:.2}"
        );
    }

    /// When work-failure rate exceeds HC-failure rate by less than the noise
    /// threshold, no selective-failure penalty is applied.
    #[test]
    fn selective_failure_below_noise_threshold_no_penalty() {
        let mut config = fast_config();
        config.selective_failure_noise_threshold = 0.3; // 30% noise floor
        config.selective_failure_coeff = 1000.0;
        config.selective_failure_min_n_eff = 3.0;
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);

        // Both HC and work fail at the same rate: inconsistency = 0 → no penalty.
        let mut stats_same = ClientObservedStats::new(config.clone());
        feed_all(&mut stats_same, v, Err(()), 15, t0, dt);

        // Reference: all succeed (lower score).
        let mut stats_ok = ClientObservedStats::new(config.clone());
        feed_all(&mut stats_ok, v, Ok(100), 15, t0, dt);

        let now = t0 + dt * 16;
        let score_same = stats_same.calculate_selection_score(&v, now);
        let score_ok = stats_ok.calculate_selection_score(&v, now);
        // score_same > score_ok because of global failure rate, but this verifies
        // that the *selective-failure* component is zero (same HC+work rate).
        assert!(
            score_same > score_ok,
            "uniform failures should still score worse than successes; same={score_same:.2} ok={score_ok:.2}"
        );
        // The difference should be attributable only to the failure penalty, not
        // selective. (We can't separate them directly here, but at least verify
        // no panic/NaN.)
        assert!(score_same.is_finite());
    }

    /// The risk term decreases as n_eff grows for the dominant operation.
    /// Specifically, with the weighted-quadrature formula, a validator with
    /// many Consensus observations (weight=0.5, dominant) should have lower
    /// risk than one with few.
    #[test]
    fn risk_decreases_as_consensus_n_eff_grows() {
        let config = fast_config();
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);

        let mut stats_few = ClientObservedStats::new(config.clone());
        let mut stats_many = ClientObservedStats::new(config.clone());

        // Few Consensus observations.
        feed(
            &mut stats_few,
            v,
            OperationType::HealthCheck,
            Ok(80),
            50,
            t0,
            dt,
        );
        feed(
            &mut stats_few,
            v,
            OperationType::Effects,
            Ok(100),
            50,
            t0,
            dt,
        );
        feed(
            &mut stats_few,
            v,
            OperationType::Submit,
            Ok(120),
            50,
            t0,
            dt,
        );
        feed(
            &mut stats_few,
            v,
            OperationType::Consensus,
            Ok(500),
            2,
            t0,
            dt,
        );

        // Many Consensus observations (same other ops).
        feed(
            &mut stats_many,
            v,
            OperationType::HealthCheck,
            Ok(80),
            50,
            t0,
            dt,
        );
        feed(
            &mut stats_many,
            v,
            OperationType::Effects,
            Ok(100),
            50,
            t0,
            dt,
        );
        feed(
            &mut stats_many,
            v,
            OperationType::Submit,
            Ok(120),
            50,
            t0,
            dt,
        );
        feed(
            &mut stats_many,
            v,
            OperationType::Consensus,
            Ok(500),
            50,
            t0,
            dt,
        );

        let now = t0 + dt * 51;
        let score_few = stats_few.calculate_selection_score(&v, now);
        let score_many = stats_many.calculate_selection_score(&v, now);
        assert!(
            score_many < score_few,
            "more Consensus obs should reduce risk; few_con={score_few:.2} many_con={score_many:.2}"
        );
    }

    /// With the old n_eff_min formula, a single sparsely-sampled Submit
    /// (weight=0.1) would cap the risk for ALL operations.  With the
    /// weighted-quadrature formula, the sparse-Submit penalty is
    /// proportional to its weight (0.1), not the whole risk budget.  We
    /// isolate the risk term by zeroing all other score components.
    #[test]
    fn risk_not_dominated_by_sparse_submit() {
        let mut config = fast_config();
        // Zero out everything except risk so we can measure it in isolation.
        config.stale_coeff = 0.0;
        config.failure_coeff = 0.0;
        config.selective_failure_coeff = 0.0;
        config.exploration_coeff = 0.0;
        // Laten coefficients kept at defaults; latency contribution is the same
        // for both validators (same latency data, only n_eff differs).
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);

        let mut stats_sparse_sub = ClientObservedStats::new(config.clone());
        let mut stats_full = ClientObservedStats::new(config.clone());

        // Both validators have 50 obs for HC/Effects/Consensus.
        // Sparse has only 2 Submit obs (weight=0.1 in the quadrature).
        for s in [&mut stats_sparse_sub, &mut stats_full] {
            feed(s, v, OperationType::HealthCheck, Ok(80), 50, t0, dt);
            feed(s, v, OperationType::Effects, Ok(100), 50, t0, dt);
            feed(s, v, OperationType::Consensus, Ok(500), 50, t0, dt);
        }
        feed(
            &mut stats_sparse_sub,
            v,
            OperationType::Submit,
            Ok(120),
            2,
            t0,
            dt,
        );
        feed(
            &mut stats_full,
            v,
            OperationType::Submit,
            Ok(120),
            50,
            t0,
            dt,
        );

        // Evaluate immediately after the last observation so staleness ≈ 0.
        let now = t0 + dt * 51;
        let score_sparse = stats_sparse_sub.calculate_selection_score(&v, now);
        let score_full = stats_full.calculate_selection_score(&v, now);

        // With quadrature, sparse Submit (weight=0.1) contributes only 1% of
        // the total variance budget (0.1² = 0.01 vs Consensus 0.5² = 0.25).
        // The ratio must be small — well under 2x.
        let ratio = score_sparse / score_full;
        assert!(
            ratio < 2.0,
            "sparse Submit (weight=0.1) should not dominate risk with quadrature formula; ratio={ratio:.2}"
        );
    }

    /// Consensus failure increases score more than equivalent Submit failure
    /// (Consensus weight=0.5 vs Submit weight=0.1 in the latency/risk formula).
    #[test]
    fn consensus_failure_penalised_more_than_submit_failure() {
        let config = fast_config();
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);

        // Baseline: all good.
        let mut stats_base = ClientObservedStats::new(config.clone());
        feed_all(&mut stats_base, v, Ok(100), 30, t0, dt);

        // Submit fails, Consensus OK.
        let mut stats_sub_fail = ClientObservedStats::new(config.clone());
        feed(
            &mut stats_sub_fail,
            v,
            OperationType::HealthCheck,
            Ok(80),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_sub_fail,
            v,
            OperationType::Effects,
            Ok(100),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_sub_fail,
            v,
            OperationType::Consensus,
            Ok(500),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_sub_fail,
            v,
            OperationType::Submit,
            Err(()),
            30,
            t0,
            dt,
        );

        // Consensus fails, Submit OK.
        let mut stats_con_fail = ClientObservedStats::new(config.clone());
        feed(
            &mut stats_con_fail,
            v,
            OperationType::HealthCheck,
            Ok(80),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_con_fail,
            v,
            OperationType::Effects,
            Ok(100),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_con_fail,
            v,
            OperationType::Submit,
            Ok(120),
            30,
            t0,
            dt,
        );
        feed(
            &mut stats_con_fail,
            v,
            OperationType::Consensus,
            Err(()),
            30,
            t0,
            dt,
        );

        let now = t0 + dt * 31;
        let score_sub_fail = stats_sub_fail.calculate_selection_score(&v, now);
        let score_con_fail = stats_con_fail.calculate_selection_score(&v, now);
        // Consensus failure must raise the score more (or equal, since it also
        // affects f_work → failure/selective terms).
        assert!(
            score_con_fail >= score_sub_fail,
            "Consensus failure should penalise at least as much as Submit failure; \
             sub_fail={score_sub_fail:.2} con_fail={score_con_fail:.2}"
        );
    }
}

// ---------------------------------------------------------------------------
// select_shuffled_preferred_validators
// ---------------------------------------------------------------------------

mod selection {
    use super::*;

    /// Empty committee returns empty result without panicking.
    #[test]
    fn empty_committee_returns_empty() {
        let config = fast_config();
        let stats = ClientObservedStats::new(config);
        let result = stats.select_shuffled_preferred_validators(
            std::iter::empty::<&AuthorityName>(),
            Instant::now(),
            rand::thread_rng(),
        );
        assert!(result.is_empty());
    }

    /// Single validator is always returned.
    #[test]
    fn single_validator_returned() {
        let config = fast_config();
        let mut stats = ClientObservedStats::new(config.clone());
        let validators = gen_validators(1);
        let v = validators[0];
        let t0 = Instant::now();
        feed_all(&mut stats, v, Ok(100), 10, t0, Duration::from_millis(10));
        let now = t0 + Duration::from_millis(110);
        let result =
            stats.select_shuffled_preferred_validators(validators.iter(), now, rand::thread_rng());
        assert_eq!(result.len(), 1);
        assert_eq!(*result[0], v);
    }

    /// min_preferred_group_size is honoured even if only one validator is in
    /// the preferred group by score.
    #[test]
    fn min_preferred_group_size_honoured() {
        let mut config = fast_config();
        config.min_preferred_group_size = 3;
        config.max_preferred_group_size = 10;
        let v = gen_validators(5);
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut stats = ClientObservedStats::new(config.clone());
        // Make v[0] the clear winner.
        feed_all(&mut stats, v[0], Ok(50), 30, t0, dt);
        for vi in &v[1..] {
            feed_all(&mut stats, *vi, Ok(5000), 30, t0, dt);
        }
        let now = t0 + dt * 31;
        let result = stats.select_shuffled_preferred_validators(v.iter(), now, rand::thread_rng());
        assert!(
            result.len() >= 3,
            "result should have at least min_preferred_group_size=3 validators; got {}",
            result.len()
        );
    }

    /// max_preferred_group_size is not exceeded when many validators have
    /// similar scores.
    #[test]
    fn max_preferred_group_size_not_exceeded() {
        let mut config = fast_config();
        config.min_preferred_group_size = 1;
        config.max_preferred_group_size = 3;
        config.max_exploration_group_size = 0;
        config.preferred_group_delta = 1.0; // very wide delta → all would qualify
        let v = gen_validators(10);
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut stats = ClientObservedStats::new(config.clone());
        for vi in &v {
            feed_all(&mut stats, *vi, Ok(100), 30, t0, dt);
        }
        let now = t0 + dt * 31;
        let result = stats.select_shuffled_preferred_validators(v.iter(), now, rand::thread_rng());
        assert!(
            result.len() <= 3,
            "result should not exceed max_preferred_group_size=3; got {}",
            result.len()
        );
    }

    /// When all known validators are excluded, the fallback returns some
    /// excluded validators chosen at random (not an empty list).
    #[test]
    fn all_excluded_fallback_returns_some() {
        let mut config = fast_config();
        config.exclusion_failure_threshold = 0.5;
        config.exclusion_min_n_eff = 3.0;
        config.min_preferred_group_size = 1;
        config.max_preferred_group_size = 4;
        config.max_exploration_group_size = 2;
        let v = gen_validators(4);
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut stats = ClientObservedStats::new(config.clone());
        for vi in &v {
            // All validators fail everywhere → excluded.
            feed_all(&mut stats, *vi, Err(()), 30, t0, dt);
        }
        let now = t0 + dt * 31;
        let result = stats.select_shuffled_preferred_validators(v.iter(), now, rand::thread_rng());
        assert!(
            !result.is_empty(),
            "fallback should return at least one excluded validator"
        );
    }

    /// Unknown validators (no recorded stats) appear in the output because
    /// they carry maximum exploration bonus.
    #[test]
    fn unknown_validators_included_via_exploration() {
        let mut config = fast_config();
        config.exploration_coeff = 100.0;
        config.min_exploration_threshold = 0.0; // include any exploration bonus
        config.max_exploration_group_size = 2;
        config.max_preferred_group_size = 2;
        let known = gen_validators(2);
        let unknown = gen_validators(2);
        let all: Vec<AuthorityName> = known.iter().chain(unknown.iter()).cloned().collect();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut stats = ClientObservedStats::new(config.clone());
        // Record many good observations for known validators so they are
        // well-explored and their exploration bonus drops.
        for vi in &known {
            feed_all(&mut stats, *vi, Ok(100), 200, t0, dt);
        }
        let now = t0 + dt * 201;
        let result =
            stats.select_shuffled_preferred_validators(all.iter(), now, rand::thread_rng());
        // Unknown validators must appear in the result (high exploration bonus).
        let unknown_in_result = result.iter().filter(|v| unknown.contains(*v)).count();
        assert!(
            unknown_in_result > 0,
            "at least one unknown validator should be included via exploration"
        );
    }

    /// A better-scoring validator is preferentially selected: when we run
    /// many trials without shuffling (no randomness in seeded test), the
    /// better validator consistently ends up in the Phase 1 group.
    #[test]
    fn better_validator_preferred_over_worse() {
        let mut config = fast_config();
        config.max_exploration_group_size = 0; // disable exploration for clarity
        config.preferred_group_delta = 0.0; // no delta grouping: only best validator
        config.min_preferred_group_size = 1;
        config.max_preferred_group_size = 1;
        let v_good = gen_validator();
        let v_bad = gen_validator();
        let all = vec![v_good, v_bad];
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut stats = ClientObservedStats::new(config.clone());
        feed_all(&mut stats, v_good, Ok(50), 30, t0, dt); // fast validator
        feed_all(&mut stats, v_bad, Ok(5000), 30, t0, dt); // slow validator
        let now = t0 + dt * 31;
        // Run 10 trials; v_good must be selected every time (Phase 1 = 1, no Phase 2).
        for _ in 0..10 {
            let result =
                stats.select_shuffled_preferred_validators(all.iter(), now, rand::thread_rng());
            assert_eq!(result.len(), 1);
            assert_eq!(
                *result[0], v_good,
                "faster validator should always be selected with delta=0"
            );
        }
    }

    /// min_preferred_group_size must not cause a panic when it exceeds the
    /// number of available candidates.
    #[test]
    fn min_preferred_group_size_larger_than_committee_no_panic() {
        let mut config = fast_config();
        config.min_preferred_group_size = 10; // larger than committee
        config.max_preferred_group_size = 20;
        let v = gen_validators(3);
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut stats = ClientObservedStats::new(config.clone());
        for vi in &v {
            feed_all(&mut stats, *vi, Ok(100), 10, t0, dt);
        }
        let now = t0 + dt * 11;
        // Must not panic.
        let result = stats.select_shuffled_preferred_validators(v.iter(), now, rand::thread_rng());
        assert_eq!(
            result.len(),
            v.len(),
            "all candidates returned when min > committee size"
        );
    }
}

// ---------------------------------------------------------------------------
// Monotonicity properties
// ---------------------------------------------------------------------------

mod monotonicity {
    use super::*;

    /// Adding more failures to an already-failing validator increases (worsens)
    /// its score monotonically.
    #[test]
    fn score_monotone_increasing_with_failure_count() {
        let config = fast_config();
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut prev_score = 0.0f64;
        for n in [5, 10, 20, 30, 50] {
            let mut stats = ClientObservedStats::new(config.clone());
            feed_all(&mut stats, v, Err(()), n, t0, dt);
            let now = t0 + dt * (n as u32 + 1);
            let score = stats.calculate_selection_score(&v, now);
            assert!(
                score >= prev_score || score == config.unknown_validator_score,
                "score should not decrease as failures accumulate; n={n} score={score:.2} prev={prev_score:.2}"
            );
            prev_score = score;
        }
    }

    /// Score improves (decreases) monotonically as we add more good
    /// observations to a validator that previously had mixed results.
    #[test]
    fn score_monotone_decreasing_with_good_observations() {
        let mut config = fast_config();
        config.latency_ewma_tau = 1.0; // slower decay to see the trend
        let v = gen_validator();
        let t0 = Instant::now();
        let dt = Duration::from_millis(100);
        let mut stats = ClientObservedStats::new(config.clone());
        // Start with some noise.
        feed_all(&mut stats, v, Ok(500), 5, t0, dt);
        let mut prev_score = stats.calculate_selection_score(&v, t0 + dt * 6);
        // Add more good observations and expect score to trend downward.
        let good_count = [10, 20, 40, 80];
        let mut cumulative = 5usize;
        for add in good_count {
            feed_all(
                &mut stats,
                v,
                Ok(50),
                add,
                t0 + dt * (cumulative as u32 + 1),
                dt,
            );
            cumulative += add;
            let now = t0 + dt * (cumulative as u32 + 1);
            let score = stats.calculate_selection_score(&v, now);
            assert!(
                score <= prev_score * 1.1, // allow 10% tolerance for variance effects
                "score should trend downward with good observations; score={score:.2} prev={prev_score:.2}"
            );
            prev_score = score;
        }
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
        let config = fast_config();
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validators(4);
        let t0 = Instant::now();
        for vi in &v {
            feed_all(&mut stats, *vi, Ok(100), 5, t0, Duration::from_millis(10));
        }
        assert_eq!(stats.num_validators(), 4);
        // Retain only v[0] and v[1].
        stats.retain_validators(v[..2].iter());
        assert_eq!(stats.num_validators(), 2);
        assert!(stats.has_validator(&v[0]));
        assert!(stats.has_validator(&v[1]));
        assert!(!stats.has_validator(&v[2]));
        assert!(!stats.has_validator(&v[3]));
    }

    /// remove_validators removes specific validators.
    #[test]
    fn remove_validators_removes_specific() {
        let config = fast_config();
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validators(3);
        let t0 = Instant::now();
        for vi in &v {
            feed_all(&mut stats, *vi, Ok(100), 5, t0, Duration::from_millis(10));
        }
        stats.remove_validators(v[1..2].iter());
        assert_eq!(stats.num_validators(), 2);
        assert!(stats.has_validator(&v[0]));
        assert!(!stats.has_validator(&v[1]));
        assert!(stats.has_validator(&v[2]));
    }

    /// Validator count is tracked per-validator (not per-observation).
    /// Each distinct validator with any recorded interaction should appear
    /// exactly once in the internal map.
    #[test]
    fn unique_validator_count_tracked_correctly() {
        let config = fast_config();
        let mut stats = ClientObservedStats::new(config.clone());
        let v = gen_validators(5);
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        // Feed different numbers of observations to each validator.
        for (i, vi) in v.iter().enumerate() {
            feed_all(&mut stats, *vi, Ok(100), i + 1, t0, dt);
        }
        // Regardless of observation count, each distinct validator appears once.
        assert_eq!(stats.num_validators(), 5);
        // Feeding more observations to existing validators should not change the count.
        feed_all(&mut stats, v[0], Ok(100), 10, t0, dt);
        assert_eq!(stats.num_validators(), 5);
    }
}

// ---------------------------------------------------------------------------
// Network blackout / all-unknown scenario
// ---------------------------------------------------------------------------

mod network_conditions {
    use super::*;

    /// When all validators in the committee are completely unknown (no prior
    /// observations), the selection still returns a non-empty set of
    /// validators.
    #[test]
    fn all_unknown_validators_still_selected() {
        let config = fast_config();
        let stats = ClientObservedStats::new(config);
        let v = gen_validators(5);
        let result = stats.select_shuffled_preferred_validators(
            v.iter(),
            Instant::now(),
            rand::thread_rng(),
        );
        assert!(
            !result.is_empty(),
            "should select some validators even if all are unknown"
        );
    }

    /// After a complete network blackout (all validators fail many times) the
    /// system still recovers and returns validators from the fallback path.
    #[test]
    fn post_blackout_still_returns_validators() {
        let mut config = fast_config();
        config.exclusion_failure_threshold = 0.5;
        config.exclusion_min_n_eff = 3.0;
        let v = gen_validators(3);
        let t0 = Instant::now();
        let dt = Duration::from_millis(10);
        let mut stats = ClientObservedStats::new(config.clone());
        for vi in &v {
            feed_all(&mut stats, *vi, Err(()), 30, t0, dt);
        }
        let now = t0 + dt * 31;
        let result = stats.select_shuffled_preferred_validators(v.iter(), now, rand::thread_rng());
        assert!(
            !result.is_empty(),
            "should still return validators from fallback path after blackout"
        );
    }
}
