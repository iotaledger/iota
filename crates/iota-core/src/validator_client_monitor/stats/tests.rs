use std::time::Duration;

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::crypto::{AuthorityKeyPair, KeypairTraits, get_key_pair};
use rand::{SeedableRng, rngs::SmallRng};

use super::*;

// ─── helpers ──────────────────────────────────────────────────────────────

fn make_names(n: usize) -> Vec<AuthorityName> {
    (0..n)
        .map(|_| {
            let (_, kp): (_, AuthorityKeyPair) = get_key_pair();
            kp.public().into()
        })
        .collect()
}

fn ok_fb(v: AuthorityName, op: OperationType, ms: u64, t: Instant) -> OperationFeedback {
    OperationFeedback::builder(v, "v".into(), op).ok_at(Duration::from_millis(ms), t)
}

fn err_fb(v: AuthorityName, op: OperationType, t: Instant) -> OperationFeedback {
    OperationFeedback::builder(v, "v".into(), op).err_at(t)
}

fn rng(seed: u64) -> SmallRng {
    SmallRng::seed_from_u64(seed)
}

/// Config with tau=0.1 s: a 10 ms observation interval gives alpha ≈ 0.095,
/// n_eff_ss ≈ 10.5 — comfortably above exclusion_min_n_eff=5.0.
fn fast_config() -> ValidatorClientMonitorConfig {
    let mut c = ValidatorClientMonitorConfig::default();
    c.latency_ewma_tau = 0.1;
    c
}

/// Record `n` observations for `op` on a bare `ValidatorClientStats`.
/// Timestamps are spaced 10 ms apart starting 10 ms after `start`.
fn feed_vs(
    vs: &mut ValidatorClientStats,
    v: AuthorityName,
    op: OperationType,
    result: Result<u64, ()>,
    n: usize,
    start: Instant,
    config: &ValidatorClientMonitorConfig,
) {
    for i in 0..n {
        let t = start + Duration::from_millis(i as u64 * 10 + 10);
        let fb = match result {
            Ok(ms) => ok_fb(v, op, ms, t),
            Err(()) => err_fb(v, op, t),
        };
        vs.record_interaction_result(&fb, config);
    }
}

/// Record `n` observations for all four ops on a bare `ValidatorClientStats`.
fn feed_all_vs(
    vs: &mut ValidatorClientStats,
    v: AuthorityName,
    result: Result<u64, ()>,
    n: usize,
    start: Instant,
    config: &ValidatorClientMonitorConfig,
) {
    for op in [
        OperationType::Submit,
        OperationType::Effects,
        OperationType::HealthCheck,
        OperationType::Consensus,
    ] {
        feed_vs(vs, v, op, result, n, start, config);
    }
}

/// Record `n` observations for `op` through a `ClientObservedStats`.
fn feed(
    stats: &mut ClientObservedStats,
    v: AuthorityName,
    op: OperationType,
    result: Result<u64, ()>,
    n: usize,
    start: Instant,
) {
    for i in 0..n {
        let t = start + Duration::from_millis(i as u64 * 10 + 10);
        let fb = match result {
            Ok(ms) => ok_fb(v, op, ms, t),
            Err(()) => err_fb(v, op, t),
        };
        stats.record_interaction_result(&fb);
    }
}

/// Record `n` observations for all four ops through a `ClientObservedStats`.
fn feed_all(
    stats: &mut ClientObservedStats,
    v: AuthorityName,
    result: Result<u64, ()>,
    n: usize,
    start: Instant,
) {
    for op in [
        OperationType::Submit,
        OperationType::Effects,
        OperationType::HealthCheck,
        OperationType::Consensus,
    ] {
        feed(stats, v, op, result, n, start);
    }
}

// ─── EWMA unit tests ──────────────────────────────────────────────────────

#[test]
fn test_ewma_first_success() {
    let e = Ewma::first_value(Ok(0.1));
    assert_eq!(e.mean, 0.1);
    assert_eq!(e.variance, 0.0);
    assert_eq!(e.failure, 0.0);
    assert_eq!(e.weight, 1.0);
    assert_eq!(e.count, 1);
}

#[test]
fn test_ewma_first_failure() {
    let e = Ewma::first_value(Err(6.0));
    assert_eq!(e.mean, 6.0);
    assert_eq!(e.failure, 1.0);
    assert_eq!(e.weight, 1.0);
    assert_eq!(e.count, 1);
}

#[test]
fn test_ewma_mean_converges_to_constant_input() {
    let target = 0.5_f64;
    let alpha = 0.2;
    let mut e = Ewma::first_value(Ok(target));
    for _ in 0..200 {
        e.update(Ok(target), alpha);
    }
    assert!(
        (e.mean - target).abs() < 1e-6,
        "mean must converge to {target}, got {}",
        e.mean
    );
    assert!(
        e.variance < 1e-10,
        "variance must converge to 0 for constant input, got {}",
        e.variance
    );
}

#[test]
fn test_ewma_weight_approaches_steady_state() {
    // Steady-state weight = 1/alpha (geometric series limit).
    let alpha = 0.2;
    let expected_ss = 1.0 / alpha; // 5.0
    let mut e = Ewma::first_value(Ok(1.0));
    for _ in 0..200 {
        e.update(Ok(1.0), alpha);
    }
    assert!(
        (e.weight - expected_ss).abs() < 0.05,
        "weight must approach {expected_ss}, got {}",
        e.weight
    );
}

#[test]
fn test_ewma_failure_rate_decays_after_recovery() {
    // 100 failures then 300 successes: failure EWMA must decay to near 0.
    let alpha = 0.1;
    let mut e = Ewma::first_value(Err(6.0));
    for _ in 0..100 {
        e.update(Err(6.0), alpha);
    }
    for _ in 0..300 {
        e.update(Ok(0.1), alpha);
    }
    assert!(
        e.failure < 0.01,
        "failure rate must decay after many successes, got {}",
        e.failure
    );
    assert!(
        (e.mean - 0.1).abs() < 1e-5,
        "mean must recover after failures, got {}",
        e.mean
    );
}

#[test]
fn test_time_decay_alpha_at_zero_one_tau_and_large_dt() {
    let now = Instant::now();
    let td = TimeDecayEwma::first_value(Ok(0.1), now);
    let tau = 10.0;

    // Zero Δt: dt clamps to 1e-9, alpha is tiny but strictly positive.
    let alpha_zero = td.alpha(now, tau);
    assert!(alpha_zero > 0.0, "alpha must be positive at Δt=0");
    assert!(
        alpha_zero < 1e-7,
        "alpha must be tiny at Δt=0, got {alpha_zero}"
    );

    // Δt = τ: alpha = 1 − 1/e ≈ 0.6321.
    let alpha_one_tau = td.alpha(now + Duration::from_secs(10), tau);
    let expected = 1.0 - (-1.0_f64).exp();
    assert!(
        (alpha_one_tau - expected).abs() < 1e-9,
        "alpha at Δt=τ must equal 1−1/e, got {alpha_one_tau}"
    );

    // Δt >> τ (clock jump / wakeup from sleep): alpha must approach 1.
    let alpha_large = td.alpha(now + Duration::from_secs(1000), tau);
    assert!(
        alpha_large > 0.9999,
        "alpha must approach 1 for large Δt, got {alpha_large}"
    );
}

// ─── LogLatencyEwma edge cases ────────────────────────────────────────────

#[test]
fn test_log_latency_ewma_near_zero_latency_stays_finite() {
    // Latency below the 1e-9 clamp must not produce −∞.
    let now = Instant::now();
    let mut lle = LogLatencyEwma::new();
    lle.update(Ok(1e-15), now, 10.0);
    let (score, _, _, _) = lle.stats(2.0, now, 10.0).unwrap();
    assert!(
        score.is_finite(),
        "score must be finite for near-zero latency"
    );
    assert!(score > 0.0, "score must be positive");
}

#[test]
fn test_log_latency_ewma_large_latency_stays_finite() {
    let now = Instant::now();
    let mut lle = LogLatencyEwma::new();
    lle.update(Ok(1_000.0), now, 10.0); // 1 000-second latency
    let (score, _, _, _) = lle.stats(2.0, now, 10.0).unwrap();
    assert!(score.is_finite(), "score must be finite for huge latency");
}

// ─── Clock-jump / staleness ───────────────────────────────────────────────

#[test]
fn test_clock_jump_staleness_spike() {
    // After a clock jump (e.g. wakeup from sleep), alpha(Δt >> τ) → 1 and
    // the staleness term must spike to near stale_coeff.
    // Uses the default tau=60s with a 3 600 s (1 h) jump.
    let config = ValidatorClientMonitorConfig::default();
    let now = Instant::now();
    let names = make_names(1);
    let v = names[0];
    let mut vs = ValidatorClientStats::new();
    feed_all_vs(&mut vs, v, Ok(100), 20, now, &config);

    let after_jump = now + Duration::from_secs(3_600);
    let (exploitation, _) = vs.performance_score(80, after_jump, &config).unwrap();

    assert!(exploitation.is_finite());
    assert!(
        exploitation > config.stale_coeff * 0.9,
        "staleness must spike after 1 h gap; exploitation={exploitation}"
    );
}

// ─── performance_score unit tests ─────────────────────────────────────────

#[test]
fn test_performance_score_fresh_validator_not_excluded() {
    // A brand-new ValidatorClientStats must not be excluded.
    // exploration must be 0 when total_observations=0 (ln 1 = 0).
    let config = ValidatorClientMonitorConfig::default();
    let vs = ValidatorClientStats::new();
    let result = vs.performance_score(0, Instant::now(), &config);
    assert!(result.is_some(), "fresh validator must not be excluded");
    let (exploitation, exploration) = result.unwrap();
    assert!(exploitation.is_finite());
    assert_eq!(
        exploration, 0.0,
        "exploration must be 0 when total_observations=0"
    );
}

#[test]
fn test_performance_score_exclusion_needs_enough_hc_samples() {
    // 100% HC failures but fewer than exclusion_min_n_eff effective samples
    // must NOT trigger exclusion.
    // With tau=0.1 s and dt=10 ms (alpha≈0.095): after 3 obs n_eff≈2.7 < 5.0.
    let mut config = fast_config();
    config.exclusion_min_n_eff = 5.0;
    config.exclusion_failure_threshold = 0.7;

    let names = make_names(1);
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();
    feed_vs(
        &mut vs,
        names[0],
        OperationType::HealthCheck,
        Err(()),
        3,
        now,
        &config,
    );

    let result = vs.performance_score(3, now + Duration::from_millis(40), &config);
    assert!(
        result.is_some(),
        "must not be excluded with only ~2.7 effective HC samples"
    );
}

#[test]
fn test_performance_score_exclusion_triggers_after_enough_hc_failures() {
    // After enough HC failures to exceed exclusion_min_n_eff and push
    // the failure rate above the threshold, the validator must be excluded.
    // With tau=0.1 s and dt=10 ms: after 30 obs n_eff≈10.5 >> 3.0.
    let mut config = fast_config();
    config.exclusion_min_n_eff = 3.0;
    config.exclusion_failure_threshold = 0.7;

    let names = make_names(1);
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();
    feed_vs(
        &mut vs,
        names[0],
        OperationType::HealthCheck,
        Err(()),
        30,
        now,
        &config,
    );

    let result = vs.performance_score(30, now + Duration::from_millis(310), &config);
    assert!(
        result.is_none(),
        "must be excluded after many HC failures with enough samples"
    );
}

#[test]
fn test_performance_score_exclusion_any_op_can_trigger() {
    // f_max covers ALL operations; a high Submit failure rate should also
    // trigger exclusion even when HealthCheck is healthy.
    let mut config = fast_config();
    config.exclusion_min_n_eff = 3.0;
    config.exclusion_failure_threshold = 0.7;

    let names = make_names(1);
    let v = names[0];
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();
    feed_vs(&mut vs, v, OperationType::Submit, Err(()), 30, now, &config);
    feed_vs(
        &mut vs,
        v,
        OperationType::HealthCheck,
        Ok(100),
        30,
        now,
        &config,
    );

    let result = vs.performance_score(60, now + Duration::from_millis(310), &config);
    assert!(
        result.is_none(),
        "high Submit failure rate must also trigger exclusion"
    );
}

#[test]
fn test_performance_score_impossibly_high_threshold_never_excluded() {
    // Setting exclusion_failure_threshold above 1.0 makes it unreachable.
    let mut config = fast_config();
    config.exclusion_failure_threshold = 1.1;
    config.exclusion_min_n_eff = 3.0;

    let names = make_names(1);
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();
    feed_vs(
        &mut vs,
        names[0],
        OperationType::HealthCheck,
        Err(()),
        30,
        now,
        &config,
    );

    let result = vs.performance_score(30, now + Duration::from_millis(310), &config);
    assert!(
        result.is_some(),
        "unreachable threshold must prevent exclusion"
    );
}

#[test]
fn test_performance_score_latency_at_expected_gives_unit_contribution() {
    // When all ops are observed at exactly their expected latency many times
    // (so the log-space EWMA converges), the latency component must be ≈ 1.0.
    // All other scoring terms are zeroed to isolate the latency measurement.
    // With stddev→0 for constant input: score(k) = mean, exp(mean) = latency.
    let mut config = fast_config();
    config.risk_coeff = 0.0;
    config.stale_coeff = 0.0;
    config.failure_coeff = 0.0;
    config.selective_failure_coeff = 0.0;

    let names = make_names(1);
    let v = names[0];
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();

    let n = 100usize;
    let ops_ms = [
        (
            OperationType::Submit,
            (config.expected_latency_submit_secs * 1000.0) as u64,
        ),
        (
            OperationType::Effects,
            (config.expected_latency_effects_secs * 1000.0) as u64,
        ),
        (
            OperationType::HealthCheck,
            (config.expected_latency_healthcheck_secs * 1000.0) as u64,
        ),
        (
            OperationType::Consensus,
            (config.expected_latency_consensus_secs * 1000.0) as u64,
        ),
    ];
    for (op, ms) in ops_ms {
        feed_vs(&mut vs, v, op, Ok(ms), n, now, &config);
    }

    // Evaluate at the exact timestamp of the last observation so Δt≈0 and
    // the staleness term (zeroed anyway) contributes nothing.
    let later = now + Duration::from_millis(n as u64 * 10);
    let (exploitation, _) = vs.performance_score(400, later, &config).unwrap();
    assert!(
        (exploitation - 1.0).abs() < 0.05,
        "latency component at expected latency must be ≈1.0, got {exploitation}"
    );
}

#[test]
fn test_performance_score_staleness_spike_after_long_gap() {
    let config = fast_config();
    let names = make_names(1);
    let v = names[0];
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();
    feed_all_vs(&mut vs, v, Ok(100), 30, now, &config);

    let t_recent = now + Duration::from_millis(305); // 5 ms after last obs
    let t_stale = now + Duration::from_secs(600); // 600 s >> tau=0.1 s

    let (score_recent, _) = vs.performance_score(120, t_recent, &config).unwrap();
    let (score_stale, _) = vs.performance_score(120, t_stale, &config).unwrap();

    assert!(
        score_stale > score_recent,
        "stale score must be worse: recent={score_recent}, stale={score_stale}"
    );
    assert!(
        score_stale > config.stale_coeff * 0.9,
        "stale score must reflect stale_coeff={}, got {score_stale}",
        config.stale_coeff
    );
}

#[test]
fn test_performance_score_selective_failure_penalty() {
    // A validator whose HC succeeds but all work ops fail receives the
    // selective-failure penalty; an honest one does not.
    let mut config = fast_config();
    config.exclusion_failure_threshold = 1.1; // disable exclusion for isolation
    config.selective_failure_noise_threshold = 0.05;
    config.selective_failure_min_n_eff = 3.0;

    let names = make_names(2);
    let (v_honest, v_selective) = (names[0], names[1]);
    let now = Instant::now();

    let mut vs_honest = ValidatorClientStats::new();
    let mut vs_selective = ValidatorClientStats::new();

    feed_all_vs(&mut vs_honest, v_honest, Ok(100), 30, now, &config);

    // Selective: HC passes, all work ops fail.
    feed_vs(
        &mut vs_selective,
        v_selective,
        OperationType::HealthCheck,
        Ok(100),
        30,
        now,
        &config,
    );
    feed_vs(
        &mut vs_selective,
        v_selective,
        OperationType::Submit,
        Err(()),
        30,
        now,
        &config,
    );
    feed_vs(
        &mut vs_selective,
        v_selective,
        OperationType::Effects,
        Err(()),
        30,
        now,
        &config,
    );
    feed_vs(
        &mut vs_selective,
        v_selective,
        OperationType::Consensus,
        Err(()),
        30,
        now,
        &config,
    );

    let later = now + Duration::from_millis(310);
    match vs_selective.performance_score(120, later, &config) {
        None => { /* excluded — also worse than honest, acceptable */ }
        Some((selective_score, _)) => {
            let (honest_score, _) = vs_honest.performance_score(120, later, &config).unwrap();
            assert!(
                selective_score > honest_score,
                "selective-failure validator must score worse: selective={selective_score}, honest={honest_score}"
            );
        }
    }
}

#[test]
fn test_performance_score_no_selective_failure_below_noise_threshold() {
    // When the failure-rate gap between work ops and HC is below the noise
    // threshold, the selective-failure penalty must be zero.
    let mut config = fast_config();
    config.risk_coeff = 0.0;
    config.stale_coeff = 0.0;
    config.failure_coeff = 0.0;
    config.selective_failure_coeff = 1_000.0; // large coefficient — any gap would dominate
    config.selective_failure_noise_threshold = 0.5; // gap from all-success is 0 < 0.5

    let names = make_names(1);
    let v = names[0];
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();
    feed_all_vs(&mut vs, v, Ok(100), 30, now, &config);

    let later = now + Duration::from_millis(310);
    let (exploitation, _) = vs.performance_score(120, later, &config).unwrap();
    // With all zero-coeff terms and zero gap, exploitation ≈ latency component
    // only.
    assert!(
        exploitation < 10.0,
        "selective-failure penalty must not fire below noise threshold, got {exploitation}"
    );
}

#[test]
fn test_performance_score_full_failure_rate_is_finite() {
    // f_max = 1.0 makes the failure term = failure_coeff / 1e-2 = 50 000.
    // Must be a large finite number, not NaN or ∞.
    let mut config = fast_config();
    config.exclusion_failure_threshold = 1.1; // keep excluded validator visible

    let names = make_names(1);
    let v = names[0];
    let mut vs = ValidatorClientStats::new();
    let now = Instant::now();
    feed_all_vs(&mut vs, v, Err(()), 30, now, &config);

    let later = now + Duration::from_millis(310);
    let (exploitation, _) = vs.performance_score(120, later, &config).unwrap();
    assert!(
        exploitation.is_finite(),
        "score at 100% failure rate must be finite"
    );
    assert!(
        exploitation > config.failure_coeff,
        "score must reflect high failure penalty, got {exploitation}"
    );
}

#[test]
fn test_performance_score_exploration_is_zero_with_no_total_observations() {
    let config = ValidatorClientMonitorConfig::default();
    let vs = ValidatorClientStats::new();
    let (_, exploration) = vs.performance_score(0, Instant::now(), &config).unwrap();
    assert_eq!(
        exploration, 0.0,
        "exploration = exploration_coeff * sqrt(ln(1)) = 0"
    );
}

#[test]
fn test_performance_score_exploration_grows_with_total_observations() {
    // Under-sampled validator (n_eff_min = 0): exploration ∝ sqrt(ln N).
    let config = ValidatorClientMonitorConfig::default();
    let vs = ValidatorClientStats::new(); // no local observations
    let now = Instant::now();

    let (_, exploration_small) = vs.performance_score(10, now, &config).unwrap();
    let (_, exploration_large) = vs.performance_score(1_000_000, now, &config).unwrap();
    assert!(
        exploration_large > exploration_small,
        "exploration must grow with total_observations: small={exploration_small}, large={exploration_large}"
    );
}

// ─── select_shuffled_preferred_validators tests ───────────────────────────

#[test]
fn test_select_empty_committee_returns_empty() {
    let stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
    let result =
        stats.select_shuffled_preferred_validators(std::iter::empty(), Instant::now(), rng(0));
    assert!(result.is_empty());
}

#[test]
fn test_select_single_validator_always_returned() {
    let stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
    let names = make_names(1);
    let result = stats.select_shuffled_preferred_validators(names.iter(), Instant::now(), rng(0));
    assert_eq!(result, vec![&names[0]]);
}

#[test]
fn test_select_all_excluded_blackout_fallback() {
    // When every committee member is excluded, the fallback returns a
    // random sample from the excluded set (≤ max_exploration_group_size).
    let mut config = fast_config();
    config.exclusion_min_n_eff = 3.0;
    config.exclusion_failure_threshold = 0.7;
    config.max_exploration_group_size = 2;

    let mut stats = ClientObservedStats::new(config.clone());
    let names = make_names(4);
    let now = Instant::now();

    for v in &names {
        feed(&mut stats, *v, OperationType::HealthCheck, Err(()), 30, now);
    }

    let later = now + Duration::from_millis(310);
    let result = stats.select_shuffled_preferred_validators(names.iter(), later, rng(0));

    assert!(
        !result.is_empty(),
        "blackout fallback must return at least one validator"
    );
    assert!(
        result.len() <= config.max_exploration_group_size,
        "blackout fallback must be ≤ max_exploration_group_size={}, got {}",
        config.max_exploration_group_size,
        result.len()
    );
    for v in &result {
        assert!(
            names.contains(v),
            "returned validator must be from the committee"
        );
    }
}

#[test]
fn test_select_excluded_never_appears_when_others_available() {
    let mut config = fast_config();
    config.exclusion_min_n_eff = 3.0;
    config.exclusion_failure_threshold = 0.7;

    let mut stats = ClientObservedStats::new(config.clone());
    let names = make_names(5);
    let now = Instant::now();
    let bad = names[0];

    // Drive names[0] into exclusion.
    feed(
        &mut stats,
        bad,
        OperationType::HealthCheck,
        Err(()),
        30,
        now,
    );
    // Give the others good scores.
    for v in &names[1..] {
        feed_all(&mut stats, *v, Ok(100), 30, now);
    }

    let later = now + Duration::from_millis(310);
    for seed in 0..30u64 {
        let result = stats.select_shuffled_preferred_validators(names.iter(), later, rng(seed));
        assert!(
            !result.contains(&&bad),
            "excluded validator must never appear in output (seed={seed})"
        );
    }
}

#[test]
fn test_select_min_preferred_group_size_respected() {
    let mut config = ValidatorClientMonitorConfig::default();
    config.min_preferred_group_size = 3;
    config.max_preferred_group_size = 10;
    config.preferred_group_delta = 0.0; // strict: only exact ties qualify
    config.max_exploration_group_size = 0;

    // No observations → all validators have identical sentinel score → all tie.
    let stats = ClientObservedStats::new(config.clone());
    let names = make_names(5);
    let result = stats.select_shuffled_preferred_validators(names.iter(), Instant::now(), rng(0));

    assert!(
        result.len() >= config.min_preferred_group_size,
        "result must have ≥ min_preferred_group_size={}, got {}",
        config.min_preferred_group_size,
        result.len()
    );
}

#[test]
fn test_select_max_preferred_group_size_respected() {
    let mut config = ValidatorClientMonitorConfig::default();
    config.min_preferred_group_size = 1;
    config.max_preferred_group_size = 3;
    config.preferred_group_delta = 10.0; // very wide: all candidates qualify
    config.max_exploration_group_size = 0;

    let stats = ClientObservedStats::new(config.clone());
    let names = make_names(10);
    let result = stats.select_shuffled_preferred_validators(names.iter(), Instant::now(), rng(0));

    assert!(
        result.len() <= config.max_preferred_group_size,
        "result must have ≤ max_preferred_group_size={}, got {}",
        config.max_preferred_group_size,
        result.len()
    );
}

#[test]
fn test_select_min_preferred_group_size_capped_by_committee_size() {
    // min_preferred_group_size > committee size must not panic (guards the
    // `candidates[phase1_count..]` slice in the Phase 2 path).
    let mut config = ValidatorClientMonitorConfig::default();
    config.min_preferred_group_size = 10;
    config.max_preferred_group_size = 20;
    config.max_exploration_group_size = 0;

    let stats = ClientObservedStats::new(config);
    let names = make_names(3); // fewer than min_preferred_group_size
    let result = stats.select_shuffled_preferred_validators(names.iter(), Instant::now(), rng(0));

    // All 3 committee members must be returned without panicking.
    assert_eq!(result.len(), 3);
}

#[test]
fn test_select_best_validator_always_in_phase1() {
    // With preferred_group_delta=0 and max_preferred_group_size=1, only
    // the best-scoring validator is selected.
    let mut config = fast_config();
    config.preferred_group_delta = 0.0;
    config.min_preferred_group_size = 1;
    config.max_preferred_group_size = 1;
    config.max_exploration_group_size = 0;
    config.exclusion_failure_threshold = 1.1; // keep all validators visible

    let mut stats = ClientObservedStats::new(config.clone());
    let names = make_names(5);
    let now = Instant::now();
    let best = names[0];

    feed_all(&mut stats, best, Ok(50), 30, now); // 50 ms
    for v in &names[1..] {
        feed_all(&mut stats, *v, Ok(3_000), 30, now); // 3 000 ms
    }

    let later = now + Duration::from_millis(305); // 5 ms after last obs → staleness≈0
    for seed in 0..20u64 {
        let result = stats.select_shuffled_preferred_validators(names.iter(), later, rng(seed));
        assert!(
            result.contains(&&best),
            "best validator must always be selected with delta=0 (seed={seed})"
        );
    }
}

#[test]
fn test_select_unknown_validator_sampled_via_phase2() {
    // A validator with no observations must be explored via Phase 2 once
    // enough total_observations have been collected:
    //   exploration_unknown = exploration_coeff * sqrt(ln(N+1))
    // With N=400 (4 ops × 50 obs × 2 known validators): ≈ 49 >
    // min_exploration_threshold=20.
    let mut config = fast_config();
    config.min_preferred_group_size = 2;
    config.max_preferred_group_size = 2;
    config.max_exploration_group_size = 1;

    let mut stats = ClientObservedStats::new(config.clone());
    let names = make_names(3);
    let now = Instant::now();
    let unknown = names[2]; // never observed

    for v in &names[..2] {
        feed_all(&mut stats, *v, Ok(100), 50, now);
    }

    // Evaluate 5 ms after the last observation to keep staleness small.
    let later = now + Duration::from_millis(505);
    let result = stats.select_shuffled_preferred_validators(names.iter(), later, rng(0));

    assert!(
        result.contains(&&unknown),
        "unknown validator must always be selected via Phase 2 when exploration bonus > threshold"
    );
}

#[test]
fn test_select_phase2_disabled_by_high_threshold() {
    // With min_exploration_threshold = f64::MAX no validator qualifies for
    // Phase 2, so the result size is bounded by max_preferred_group_size.
    let mut config = ValidatorClientMonitorConfig::default();
    config.min_preferred_group_size = 1;
    config.max_preferred_group_size = 2;
    config.max_exploration_group_size = 5;
    config.min_exploration_threshold = f64::MAX;

    let stats = ClientObservedStats::new(config.clone());
    let names = make_names(10);
    let result = stats.select_shuffled_preferred_validators(names.iter(), Instant::now(), rng(0));

    assert!(
        result.len() <= config.max_preferred_group_size,
        "with Phase 2 disabled result must be ≤ max_preferred_group_size={}, got {}",
        config.max_preferred_group_size,
        result.len()
    );
}

#[test]
fn test_select_no_duplicate_validators() {
    let mut config = ValidatorClientMonitorConfig::default();
    config.min_preferred_group_size = 3;
    config.max_preferred_group_size = 5;
    config.max_exploration_group_size = 3;
    config.min_exploration_threshold = 0.0; // all qualify for Phase 2

    let stats = ClientObservedStats::new(config);
    let names = make_names(10);
    let result = stats.select_shuffled_preferred_validators(names.iter(), Instant::now(), rng(0));

    let unique: std::collections::HashSet<*const AuthorityName> =
        result.iter().map(|v| *v as *const _).collect();
    assert_eq!(
        unique.len(),
        result.len(),
        "selection result must not contain duplicate validators"
    );
}

#[test]
fn test_select_deterministic_with_same_seed() {
    let stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
    let names = make_names(8);
    let now = Instant::now();

    let r1 = stats.select_shuffled_preferred_validators(names.iter(), now, rng(42));
    let r2 = stats.select_shuffled_preferred_validators(names.iter(), now, rng(42));
    assert_eq!(r1, r2, "same RNG seed must produce identical output");
}

#[test]
fn test_select_different_seeds_produce_varied_orderings() {
    let stats = ClientObservedStats::new(ValidatorClientMonitorConfig::default());
    let names = make_names(8);
    let now = Instant::now();

    let orderings: std::collections::HashSet<Vec<*const AuthorityName>> = (0..50u64)
        .map(|seed| {
            stats
                .select_shuffled_preferred_validators(names.iter(), now, rng(seed))
                .iter()
                .map(|v| *v as *const _)
                .collect()
        })
        .collect();

    assert!(
        orderings.len() > 1,
        "shuffling must produce different orderings across different RNG seeds"
    );
}

// ─── scoring property / monotonicity tests ────────────────────────────────

#[test]
fn test_score_monotone_in_latency() {
    let mut config = fast_config();
    config.exclusion_failure_threshold = 1.1;

    let names = make_names(2);
    let (v_fast, v_slow) = (names[0], names[1]);
    let now = Instant::now();

    let mut vs_fast = ValidatorClientStats::new();
    let mut vs_slow = ValidatorClientStats::new();
    feed_all_vs(&mut vs_fast, v_fast, Ok(50), 30, now, &config);
    feed_all_vs(&mut vs_slow, v_slow, Ok(2_000), 30, now, &config);

    let later = now + Duration::from_millis(305);
    let fast = vs_fast.calculate_selection_score(120, later, &config);
    let slow = vs_slow.calculate_selection_score(120, later, &config);

    assert!(
        fast < slow,
        "lower latency must give lower (better) score: fast={fast}, slow={slow}"
    );
}

#[test]
fn test_score_monotone_in_failure_rate() {
    let mut config = fast_config();
    config.exclusion_failure_threshold = 1.1;

    let names = make_names(2);
    let (v_reliable, v_flaky) = (names[0], names[1]);
    let now = Instant::now();

    let mut vs_reliable = ValidatorClientStats::new();
    let mut vs_flaky = ValidatorClientStats::new();

    feed_all_vs(&mut vs_reliable, v_reliable, Ok(100), 30, now, &config);

    // ~50% failure rate: alternate ok/err.
    for op in [
        OperationType::Submit,
        OperationType::Effects,
        OperationType::HealthCheck,
        OperationType::Consensus,
    ] {
        for i in 0..30usize {
            let t = now + Duration::from_millis(i as u64 * 10 + 10);
            let fb = if i % 2 == 0 {
                ok_fb(v_flaky, op, 100, t)
            } else {
                err_fb(v_flaky, op, t)
            };
            vs_flaky.record_interaction_result(&fb, &config);
        }
    }

    let later = now + Duration::from_millis(305);
    let reliable = vs_reliable.calculate_selection_score(120, later, &config);
    let flaky = vs_flaky.calculate_selection_score(120, later, &config);

    assert!(
        reliable < flaky,
        "reliable validator must score better: reliable={reliable}, flaky={flaky}"
    );
}

// ─── data-management tests ────────────────────────────────────────────────

#[test]
fn test_retain_validators_removes_stale_entries() {
    let config = ValidatorClientMonitorConfig::default();
    let mut stats = ClientObservedStats::new(config);
    let names = make_names(4);
    let now = Instant::now();

    for v in &names {
        stats.record_interaction_result(&ok_fb(*v, OperationType::HealthCheck, 100, now));
    }
    assert_eq!(stats.num_validators(), 4);

    stats.retain_validators(names[..2].iter());

    assert_eq!(stats.num_validators(), 2);
    assert!(stats.has_validator(&names[0]));
    assert!(stats.has_validator(&names[1]));
    assert!(!stats.has_validator(&names[2]));
    assert!(!stats.has_validator(&names[3]));
}

#[test]
fn test_remove_validators_removes_specific_entries() {
    let config = ValidatorClientMonitorConfig::default();
    let mut stats = ClientObservedStats::new(config);
    let names = make_names(4);
    let now = Instant::now();

    for v in &names {
        stats.record_interaction_result(&ok_fb(*v, OperationType::HealthCheck, 100, now));
    }

    stats.remove_validators(names[1..3].iter());

    assert_eq!(stats.num_validators(), 2);
    assert!(stats.has_validator(&names[0]));
    assert!(!stats.has_validator(&names[1]));
    assert!(!stats.has_validator(&names[2]));
    assert!(stats.has_validator(&names[3]));
}

#[test]
fn test_selection_score_is_finite_after_single_success() {
    // Regression: score must be finite after a single observation.
    let config = ValidatorClientMonitorConfig::default();
    let mut stats = ClientObservedStats::new(config);
    let names = make_names(1);
    let now = Instant::now();

    stats.record_interaction_result(&ok_fb(names[0], OperationType::Submit, 100, now));
    let score = stats.calculate_selection_score(&names[0], now);
    assert!(
        score.is_finite(),
        "score must be finite after a single success"
    );
}
