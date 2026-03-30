// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Randomized simulator for the validator scoring and selection system.
//!
//! The simulator models a committee of validators (4–10) and a set of fullnodes
//! (1–5).  Each fullnode maintains its own independent `ClientObservedStats`.
//! Simulated time advances in discrete health-check rounds; within each round
//! a configurable number of transactions are submitted.
//!
//! The tests in this module verify three properties:
//!
//! 1. **Scoring optimality** – the default configuration parameters outperform
//!    deliberately bad alternatives (wrong `latency_ewma_tau`, too-aggressive
//!    or too-conservative `exclusion_min_n_eff`, etc.).
//!
//! 2. **Network liveness** – the system continues to deliver transactions
//!    across the full range of random committee compositions (honest, slow,
//!    unreliable validators).
//!
//! 3. **Byzantine resistance** – selective-Byzantine and bait-and-switch
//!    validators are detected and excluded; traffic is redirected to honest
//!    peers.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{
    base_types::AuthorityName,
    crypto::{AuthorityKeyPair, KeypairTraits, get_key_pair},
};
use rand::{Rng, SeedableRng, rngs::SmallRng};

use super::{OperationFeedback, OperationType};
use crate::validator_client_monitor::stats::ClientObservedStats;

// ─── Simulated clock ────────────────────────────────────────────────────────

/// Monotonically advancing virtual clock.
///
/// All `Instant` values fed into `ClientObservedStats` are derived from this
/// clock so that EWMA time-decay reflects simulated time, not wall time.
struct SimClock {
    base: Instant,
    elapsed: Duration,
}

impl SimClock {
    fn new() -> Self {
        Self {
            base: Instant::now(),
            elapsed: Duration::ZERO,
        }
    }

    fn now(&self) -> Instant {
        self.base + self.elapsed
    }

    fn advance(&mut self, dt: Duration) {
        self.elapsed += dt;
    }
}

// ─── Validator behaviour ─────────────────────────────────────────────────────

/// Per-validator behavioural model used by the simulator.
#[derive(Clone, Debug)]
enum Behaviour {
    /// Responds correctly at normal latency.
    Honest,
    /// Always passes HealthCheck but fails Submit and Consensus.
    SelectivelyByzantine,
    /// Honest until `warm_up` of simulated time has elapsed, then becomes
    /// selectively Byzantine.  Models a validator that deliberately builds
    /// EWMA trust before attacking.
    BaitAndSwitch { warm_up: Duration },
    /// Correct but slow: every response takes `factor × base_latency`.
    Slow { factor: f64 },
    /// Each individual operation independently fails with probability
    /// `fail_rate`.  Models a flaky validator, not a deliberate attacker.
    Unreliable { fail_rate: f64 },
}

/// Base latencies in milliseconds, indexed by `OperationType as usize`.
///
/// Order: Submit=0, Effects=1, HealthCheck=2, Consensus=3.
const BASE_MS: [u64; 4] = [150, 1_200, 80, 700];

#[derive(Clone)]
struct SimValidator {
    name: AuthorityName,
    behaviour: Behaviour,
}

impl SimValidator {
    fn new(name: AuthorityName, behaviour: Behaviour) -> Self {
        Self { name, behaviour }
    }

    /// Returns the validator's simulated response to `op`.
    ///
    /// `elapsed` is the simulated time since the simulation started; it is
    /// used by `BaitAndSwitch` to decide when to flip.
    /// `rng` provides ±20 % latency jitter.
    fn respond(
        &self,
        op: OperationType,
        elapsed: Duration,
        rng: &mut impl Rng,
    ) -> Result<Duration, ()> {
        let base_ms = BASE_MS[op as usize];
        let jitter: f64 = 0.8 + rng.gen::<f64>() * 0.4; // uniform [0.8, 1.2]

        let fails_work = match &self.behaviour {
            Behaviour::Honest => false,
            Behaviour::SelectivelyByzantine => true,
            Behaviour::BaitAndSwitch { warm_up } => elapsed > *warm_up,
            Behaviour::Slow { factor } => {
                let ms = (base_ms as f64 * factor * jitter) as u64;
                return Ok(Duration::from_millis(ms));
            }
            Behaviour::Unreliable { fail_rate } => rng.gen_bool(*fail_rate),
        };

        let latency = Duration::from_millis((base_ms as f64 * jitter) as u64);
        if fails_work {
            // Selective attack: HealthCheck passes, all work operations fail.
            match op {
                OperationType::HealthCheck => Ok(latency),
                _ => Err(()),
            }
        } else {
            Ok(latency)
        }
    }

    /// Returns true if this validator is currently in a Byzantine state.
    fn is_byzantine_now(&self, elapsed: Duration) -> bool {
        match &self.behaviour {
            Behaviour::Honest | Behaviour::Slow { .. } | Behaviour::Unreliable { .. } => false,
            Behaviour::SelectivelyByzantine => true,
            Behaviour::BaitAndSwitch { warm_up } => elapsed > *warm_up,
        }
    }
}

// ─── Per-fullnode metrics ────────────────────────────────────────────────────

/// Transaction-level counters for one fullnode.
#[derive(Default)]
struct TxMetrics {
    submitted: u64,
    succeeded: u64,
    /// How many Submit attempts were directed at each validator (across
    /// retries).
    traffic: HashMap<AuthorityName, u64>,
}

impl TxMetrics {
    fn success_rate(&self) -> f64 {
        if self.submitted == 0 {
            1.0
        } else {
            self.succeeded as f64 / self.submitted as f64
        }
    }

    fn byzantine_traffic_fraction(&self, byzantine: &[AuthorityName]) -> f64 {
        let total: u64 = self.traffic.values().sum();
        if total == 0 {
            return 0.0;
        }
        let to_byz: u64 = byzantine.iter().filter_map(|n| self.traffic.get(n)).sum();
        to_byz as f64 / total as f64
    }
}

// ─── Fullnode ────────────────────────────────────────────────────────────────

struct SimFullnode {
    stats: ClientObservedStats,
    metrics: TxMetrics,
}

impl SimFullnode {
    fn new(config: ValidatorClientMonitorConfig) -> Self {
        Self {
            stats: ClientObservedStats::new(config),
            metrics: TxMetrics::default(),
        }
    }
}

// ─── Simulation ──────────────────────────────────────────────────────────────

/// Parameters that remain constant across a simulation run.
#[derive(Clone)]
struct SimParams {
    /// Number of transactions to attempt per health-check round.
    tx_per_round: usize,
    /// Maximum number of validator retries before a transaction is abandoned.
    max_retries: usize,
}

impl Default for SimParams {
    fn default() -> Self {
        Self {
            tx_per_round: 10,
            max_retries: 3,
        }
    }
}

struct Simulation {
    validators: Vec<SimValidator>,
    fullnodes: Vec<SimFullnode>,
    clock: SimClock,
    hc_interval: Duration,
    params: SimParams,
}

impl Simulation {
    fn new(
        validators: Vec<SimValidator>,
        num_fullnodes: usize,
        config: ValidatorClientMonitorConfig,
        params: SimParams,
    ) -> Self {
        let hc_interval = config.health_check_interval;
        let fullnodes = (0..num_fullnodes)
            .map(|_| SimFullnode::new(config.clone()))
            .collect();
        Self {
            validators,
            fullnodes,
            clock: SimClock::new(),
            hc_interval,
            params,
        }
    }

    /// Run `num_rounds` health-check rounds.
    ///
    /// Each round:
    ///   1. All fullnodes perform a health-check against every validator.
    ///   2. `tx_per_round` transactions are evenly spread across the interval,
    ///      each from a randomly chosen fullnode.
    fn run(&mut self, num_rounds: usize, rng: &mut impl Rng) {
        // Space transactions evenly inside each HC interval.
        let tx_dt = self.hc_interval / (self.params.tx_per_round as u32 + 1);
        let n_fullnodes = self.fullnodes.len();

        for _ in 0..num_rounds {
            self.do_health_checks(rng);
            // A small fixed offset to ensure HC and first TX have distinct timestamps.
            self.clock.advance(Duration::from_millis(50));

            for _ in 0..self.params.tx_per_round {
                self.clock.advance(tx_dt);
                let fn_idx = rng.gen_range(0..n_fullnodes);
                self.do_transaction(fn_idx, rng);
            }
        }
    }

    // ─── Health checks ───────────────────────────────────────────────────────

    fn do_health_checks(&mut self, rng: &mut impl Rng) {
        let now = self.clock.now();
        let elapsed = self.clock.elapsed;

        // Gather all validator responses before touching fullnodes to satisfy
        // the borrow checker (self.validators vs self.fullnodes).
        let hc_results: Vec<(AuthorityName, Result<Duration, ()>)> = self
            .validators
            .iter()
            .map(|v| (v.name, v.respond(OperationType::HealthCheck, elapsed, rng)))
            .collect();

        for fn_ in &mut self.fullnodes {
            for &(name, result) in &hc_results {
                let fb =
                    OperationFeedback::builder(name, String::new(), OperationType::HealthCheck)
                        .result_at(result, now);
                fn_.stats.record_interaction_result(&fb);
            }
        }
    }

    // ─── Transaction submission ──────────────────────────────────────────────

    /// Simulates one client transaction from fullnode `fn_idx`.
    ///
    /// Models the `TransactionDriver` flow:
    ///   Submit  → (if ok) Effects from all validators
    ///           → (if ok) Consensus on the submitted validator
    ///
    /// On Consensus failure the driver retries with the next candidate from
    /// `select_shuffled_preferred_validators`, up to `max_retries`.
    fn do_transaction(&mut self, fn_idx: usize, rng: &mut impl Rng) {
        let now = self.clock.now();
        let elapsed = self.clock.elapsed;

        // Collect validator names before borrowing fullnodes.
        let names: Vec<AuthorityName> = self.validators.iter().map(|v| v.name).collect();

        // Selection order (Phase 1 + Phase 2).  The block ends the borrow on
        // self.fullnodes before we mutably borrow it below.
        let mut candidates: Vec<AuthorityName> = {
            self.fullnodes[fn_idx]
                .stats
                .select_shuffled_preferred_validators(names.iter(), now, &mut *rng)
                .into_iter()
                .cloned()
                .collect()
        };

        // Append any validator not in the initial selection as a last-resort
        // fallback (mirrors the driver's "try all" behaviour).
        for &n in &names {
            if !candidates.contains(&n) {
                candidates.push(n);
            }
        }

        self.fullnodes[fn_idx].metrics.submitted += 1;
        let max_retries = self.params.max_retries.min(candidates.len());
        let mut succeeded = false;

        'attempts: for &target_name in candidates.iter().take(max_retries) {
            let vi = self
                .validators
                .iter()
                .position(|v| v.name == target_name)
                .unwrap();

            *self.fullnodes[fn_idx]
                .metrics
                .traffic
                .entry(target_name)
                .or_insert(0) += 1;

            // ── Submit ─────────────────────────────────────────────────────
            // Borrow self.validators immutably; result is owned → borrow ends.
            let sub_res = self.validators[vi].respond(OperationType::Submit, elapsed, rng);
            let sub_fb =
                OperationFeedback::builder(target_name, String::new(), OperationType::Submit)
                    .result_at(sub_res, now);
            self.fullnodes[fn_idx]
                .stats
                .record_interaction_result(&sub_fb);

            if sub_res.is_err() {
                continue 'attempts;
            }

            // ── Effects (all validators) ────────────────────────────────────
            // Collect all responses first so that the self.validators borrow
            // is fully released before we start borrowing self.fullnodes.
            let eff_results: Vec<(AuthorityName, Result<Duration, ()>)> = self
                .validators
                .iter()
                .map(|v| (v.name, v.respond(OperationType::Effects, elapsed, rng)))
                .collect();

            for (v_name, eff_res) in eff_results {
                let eff_fb =
                    OperationFeedback::builder(v_name, String::new(), OperationType::Effects)
                        .result_at(eff_res, now);
                self.fullnodes[fn_idx]
                    .stats
                    .record_interaction_result(&eff_fb);
            }

            // ── Consensus (submitted validator) ────────────────────────────
            let con_res = self.validators[vi].respond(OperationType::Consensus, elapsed, rng);
            let con_fb =
                OperationFeedback::builder(target_name, String::new(), OperationType::Consensus)
                    .result_at(con_res, now);
            self.fullnodes[fn_idx]
                .stats
                .record_interaction_result(&con_fb);

            if con_res.is_ok() {
                succeeded = true;
                break 'attempts;
            }
            // Consensus failure → retry with next candidate.
        }

        if succeeded {
            self.fullnodes[fn_idx].metrics.succeeded += 1;
        }
    }

    // ─── Aggregate metrics ───────────────────────────────────────────────────

    fn overall_success_rate(&self) -> f64 {
        let submitted: u64 = self.fullnodes.iter().map(|f| f.metrics.submitted).sum();
        let succeeded: u64 = self.fullnodes.iter().map(|f| f.metrics.succeeded).sum();
        if submitted == 0 {
            1.0
        } else {
            succeeded as f64 / submitted as f64
        }
    }

    /// Fraction of all Submit attempts that were directed at
    /// currently-Byzantine validators.
    fn byzantine_traffic_fraction(&self) -> f64 {
        let elapsed = self.clock.elapsed;
        let byz: Vec<AuthorityName> = self
            .validators
            .iter()
            .filter(|v| v.is_byzantine_now(elapsed))
            .map(|v| v.name)
            .collect();
        if byz.is_empty() {
            return 0.0;
        }
        let total: u64 = self
            .fullnodes
            .iter()
            .flat_map(|f| f.metrics.traffic.values())
            .sum();
        let to_byz: u64 = self
            .fullnodes
            .iter()
            .flat_map(|f| f.metrics.traffic.iter())
            .filter(|(n, _)| byz.contains(n))
            .map(|(_, c)| *c)
            .sum();
        if total == 0 {
            0.0
        } else {
            to_byz as f64 / total as f64
        }
    }

    /// Per-validator traffic fractions (for distribution assertions).
    fn traffic_fractions(&self) -> HashMap<AuthorityName, f64> {
        let total: u64 = self
            .fullnodes
            .iter()
            .flat_map(|f| f.metrics.traffic.values())
            .sum();
        let mut per_validator: HashMap<AuthorityName, u64> = HashMap::new();
        for fn_ in &self.fullnodes {
            for (&name, &count) in &fn_.metrics.traffic {
                *per_validator.entry(name).or_insert(0) += count;
            }
        }
        per_validator
            .into_iter()
            .map(|(n, c)| {
                (
                    n,
                    if total == 0 {
                        0.0
                    } else {
                        c as f64 / total as f64
                    },
                )
            })
            .collect()
    }

    /// Reset transaction metrics on all fullnodes (for before/after
    /// comparisons).
    fn reset_metrics(&mut self) {
        for fn_ in &mut self.fullnodes {
            fn_.metrics = TxMetrics::default();
        }
    }
}

// ─── Test helpers ────────────────────────────────────────────────────────────

fn gen_name() -> AuthorityName {
    let (_, kp): (_, AuthorityKeyPair) = get_key_pair();
    kp.public().into()
}

fn make_sim(
    behaviours: Vec<Behaviour>,
    num_fullnodes: usize,
    config: ValidatorClientMonitorConfig,
    params: SimParams,
) -> Simulation {
    let validators = behaviours
        .into_iter()
        .map(|b| SimValidator::new(gen_name(), b))
        .collect();
    Simulation::new(validators, num_fullnodes, config, params)
}

fn default_params() -> SimParams {
    SimParams {
        tx_per_round: 10,
        max_retries: 3,
    }
}

fn config_with_tau(tau_secs: f64) -> ValidatorClientMonitorConfig {
    let mut c = ValidatorClientMonitorConfig::default();
    c.latency_ewma_tau = tau_secs;
    c
}

fn config_with_exclusion_n_eff(n: f64) -> ValidatorClientMonitorConfig {
    let mut c = ValidatorClientMonitorConfig::default();
    c.exclusion_min_n_eff = n;
    c.selective_failure_min_n_eff = n;
    c
}

// ─── Property 1: Scoring optimality ─────────────────────────────────────────

/// With tau=60s (default) the Byzantine validator reaches n_eff >
/// exclusion_min_n_eff=5 after ~8 health-check rounds and is excluded.  With
/// tau=10s n_eff_ss ≈ 1.58, which is permanently below the threshold →
/// Byzantine is never excluded.
#[test]
fn sim_default_tau_excludes_byzantine_faster_than_tau_10s() {
    let behaviours = vec![
        Behaviour::SelectivelyByzantine,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    // No retries: makes Byzantine impact on success rate clearly visible.
    let params = SimParams {
        tx_per_round: 10,
        max_retries: 1,
    };
    let num_rounds = 40; // 400s simulated

    for seed in 0..8u64 {
        let (sr_default, byz_frac_default) = {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut sim = make_sim(behaviours.clone(), 2, config_with_tau(60.0), params.clone());
            sim.run(num_rounds, &mut rng);
            (sim.overall_success_rate(), sim.byzantine_traffic_fraction())
        };

        let (sr_bad_tau, byz_frac_bad_tau) = {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut sim = make_sim(behaviours.clone(), 2, config_with_tau(10.0), params.clone());
            sim.run(num_rounds, &mut rng);
            (sim.overall_success_rate(), sim.byzantine_traffic_fraction())
        };

        // With tau=60s the Byzantine validator is excluded; its traffic fraction
        // should converge lower than with tau=10s (where it is never excluded).
        assert!(
            byz_frac_default <= byz_frac_bad_tau + 0.05,
            "seed={seed}: tau=60s should route less traffic to Byzantine; \
             default_byz={byz_frac_default:.3} tau10_byz={byz_frac_bad_tau:.3}"
        );
        // And overall success rate should be at least as good.
        assert!(
            sr_default >= sr_bad_tau - 0.05,
            "seed={seed}: tau=60s should achieve ≥ success rate vs tau=10s; \
             default={sr_default:.3} tau10={sr_bad_tau:.3}"
        );
    }
}

/// A too-aggressive exclusion_min_n_eff (1.0) prematurely excludes an
/// Unreliable (but non-Byzantine) validator whose occasional failures look like
/// exclusion triggers, starving the committee of a potentially useful peer.
/// The default n_eff=5.0 waits for statistically reliable evidence.
#[test]
fn sim_default_exclusion_n_eff_more_stable_than_aggressive() {
    // A validator that fails 20% of the time should NOT be permanently excluded;
    // it is still useful for routing traffic when the better validators are slow.
    let behaviours = vec![
        Behaviour::Unreliable { fail_rate: 0.20 },
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let params = SimParams {
        tx_per_round: 10,
        max_retries: 1,
    };

    // Measure throughput (success rate) across many seeds.
    let mut better_or_equal = 0u64;
    let trials = 10u64;

    for seed in 0..trials {
        let sr_default = {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut sim = make_sim(
                behaviours.clone(),
                2,
                ValidatorClientMonitorConfig::default(),
                params.clone(),
            );
            sim.run(30, &mut rng);
            sim.overall_success_rate()
        };
        let sr_aggressive = {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut sim = make_sim(
                behaviours.clone(),
                2,
                config_with_exclusion_n_eff(1.0),
                params.clone(),
            );
            sim.run(30, &mut rng);
            sim.overall_success_rate()
        };
        if sr_default >= sr_aggressive - 0.02 {
            better_or_equal += 1;
        }
    }

    assert!(
        better_or_equal >= trials * 6 / 10,
        "default exclusion_min_n_eff should be ≥ aggressive in ≥60% of trials; \
         got {better_or_equal}/{trials}"
    );
}

/// An over-conservative exclusion threshold (n_eff=50) never excludes any
/// validator, including clearly Byzantine ones — resulting in worse
/// steady-state success rate than the default (n_eff=5).
#[test]
fn sim_default_exclusion_n_eff_better_than_too_conservative() {
    let behaviours = vec![
        Behaviour::SelectivelyByzantine,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let params = SimParams {
        tx_per_round: 10,
        max_retries: 1,
    };

    for seed in 0..8u64 {
        let sr_default = {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut sim = make_sim(
                behaviours.clone(),
                2,
                ValidatorClientMonitorConfig::default(),
                params.clone(),
            );
            sim.run(40, &mut rng);
            sim.overall_success_rate()
        };
        let sr_conservative = {
            let mut rng = SmallRng::seed_from_u64(seed);
            // n_eff=50 → Byzantine never accumulates enough HC samples to be excluded
            // within a practical simulation window.
            let mut sim = make_sim(
                behaviours.clone(),
                2,
                config_with_exclusion_n_eff(50.0),
                params.clone(),
            );
            sim.run(40, &mut rng);
            sim.overall_success_rate()
        };

        assert!(
            sr_default >= sr_conservative - 0.05,
            "seed={seed}: default exclusion_min_n_eff should achieve ≥ success rate \
             vs overly conservative n_eff=50; default={sr_default:.3} cons={sr_conservative:.3}"
        );
    }
}

// ─── Property 2: Network liveness ───────────────────────────────────────────

/// All-honest committees always achieve near-perfect success rates.
#[test]
fn sim_all_honest_network_is_fully_live() {
    for seed in 0..8u64 {
        let mut rng = SmallRng::seed_from_u64(seed);
        let committee_size = 4 + (seed as usize % 5); // 4–8
        let behaviours = vec![Behaviour::Honest; committee_size];
        let num_fullnodes = 1 + (seed as usize % 3); // 1–3
        let mut sim = make_sim(
            behaviours,
            num_fullnodes,
            ValidatorClientMonitorConfig::default(),
            SimParams {
                tx_per_round: 20,
                max_retries: 1,
            },
        );
        sim.run(20, &mut rng);
        let sr = sim.overall_success_rate();
        assert!(
            sr > 0.95,
            "all-honest committee should have >95% success rate; seed={seed} sr={sr:.3}"
        );
    }
}

/// Committees mixing slow and unreliable (but non-Byzantine) validators stay
/// live: retries compensate for occasional failures.
#[test]
fn sim_mixed_honest_committee_stays_live() {
    let behaviours = vec![
        Behaviour::Slow { factor: 4.0 },
        Behaviour::Unreliable { fail_rate: 0.25 },
        Behaviour::Unreliable { fail_rate: 0.15 },
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let params = SimParams {
        tx_per_round: 10,
        max_retries: 3,
    };

    for seed in 0..8u64 {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut sim = make_sim(
            behaviours.clone(),
            2,
            ValidatorClientMonitorConfig::default(),
            params.clone(),
        );
        sim.run(30, &mut rng);
        let sr = sim.overall_success_rate();
        assert!(
            sr > 0.80,
            "mixed honest committee should achieve >80% success rate; seed={seed} sr={sr:.3}"
        );
    }
}

/// Randomised committee composition: up to 1 Byzantine validator, random
/// proportions of slow/unreliable peers, random number of fullnodes.
/// Liveness must hold across the full parameter space.
#[test]
fn sim_random_committee_stays_live() {
    for seed in 0..12u64 {
        let mut rng = SmallRng::seed_from_u64(seed ^ 0xDEAD_BEEF);

        let committee_size = rng.gen_range(4..=8usize);
        let num_byzantine = usize::from(rng.gen_bool(0.5)); // 0 or 1
        let remaining = committee_size - num_byzantine;
        let num_slow = rng.gen_range(0..=(remaining / 2));
        let num_unreliable = rng.gen_range(0..=((remaining - num_slow) / 2));
        let num_honest = remaining - num_slow - num_unreliable;

        let mut behaviours: Vec<Behaviour> = Vec::new();
        behaviours.extend(vec![Behaviour::SelectivelyByzantine; num_byzantine]);
        behaviours.extend(vec![Behaviour::Slow { factor: 3.0 }; num_slow]);
        behaviours.extend(vec![
            Behaviour::Unreliable { fail_rate: 0.20 };
            num_unreliable
        ]);
        behaviours.extend(vec![Behaviour::Honest; num_honest]);

        let num_fullnodes = rng.gen_range(1..=3usize);
        let params = SimParams {
            tx_per_round: 10,
            max_retries: 3,
        };

        let mut sim_rng = SmallRng::seed_from_u64(seed.wrapping_mul(6_364_136_223_846_793_005));
        let mut sim = make_sim(
            behaviours.clone(),
            num_fullnodes,
            ValidatorClientMonitorConfig::default(),
            params,
        );
        sim.run(30, &mut sim_rng);

        let sr = sim.overall_success_rate();
        // With up to 1 Byzantine validator and 3 retries, success rate must
        // stay above 70% (scoring eventually detects and excludes the Byzantine).
        assert!(
            sr > 0.70,
            "seed={seed}: random committee should maintain liveness; \
             size={committee_size} byz={num_byzantine} slow={num_slow} \
             unreliable={num_unreliable} fn={num_fullnodes} sr={sr:.3}"
        );
    }
}

/// With no retries and a large committee, the system still routes the majority
/// of transactions to honest validators once the scoring system warms up.
#[test]
fn sim_no_retry_scoring_directs_traffic_to_honest_validators() {
    let behaviours = vec![
        Behaviour::SelectivelyByzantine,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let params = SimParams {
        tx_per_round: 10,
        max_retries: 1,
    };

    for seed in 0..6u64 {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut sim = make_sim(
            behaviours.clone(),
            2,
            ValidatorClientMonitorConfig::default(),
            params.clone(),
        );
        // Warm-up: let scoring build enough n_eff.
        sim.run(10, &mut rng);
        sim.reset_metrics();
        // Steady-state measurement.
        sim.run(20, &mut rng);

        let byz_frac = sim.byzantine_traffic_fraction();
        let sr = sim.overall_success_rate();

        // After warm-up the Byzantine should be mostly excluded.
        assert!(
            byz_frac < 0.15,
            "seed={seed}: Byzantine traffic fraction in steady state should be <15%; got {byz_frac:.3}"
        );
        // And success rate should be high since traffic goes to honest validators.
        assert!(
            sr > 0.80,
            "seed={seed}: success rate in steady state should be >80%; got {sr:.3}"
        );
    }
}

// ─── Property 3: Byzantine resistance ───────────────────────────────────────

/// A selectively-Byzantine validator (pass HC, fail work) is detected via the
/// selective-failure penalty and subsequently excluded.
/// After exclusion, traffic fraction to the Byzantine validator drops.
#[test]
fn sim_selective_byzantine_detected_and_excluded() {
    let behaviours = vec![
        Behaviour::SelectivelyByzantine,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let params = SimParams {
        tx_per_round: 5,
        max_retries: 2,
    };

    for seed in 0..8u64 {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut sim = make_sim(
            behaviours.clone(),
            2,
            ValidatorClientMonitorConfig::default(),
            params.clone(),
        );

        // Phase 1: warm-up (8 rounds ≈ 80s of simulated time).
        // During this phase n_eff grows toward exclusion_min_n_eff=5.
        sim.run(8, &mut rng);
        let warm_byz_frac = sim.byzantine_traffic_fraction();
        sim.reset_metrics();

        // Phase 2: steady state (20 rounds ≈ 200s).
        // By now n_hc > exclusion_min_n_eff; Byzantine validator should be excluded.
        sim.run(20, &mut rng);
        let steady_byz_frac = sim.byzantine_traffic_fraction();
        let sr = sim.overall_success_rate();

        assert!(
            steady_byz_frac <= warm_byz_frac + 0.05 || steady_byz_frac < 0.15,
            "seed={seed}: Byzantine traffic should not grow in steady state; \
             warm={warm_byz_frac:.3} steady={steady_byz_frac:.3}"
        );
        assert!(
            sr > 0.75,
            "seed={seed}: success rate should be >75% after Byzantine exclusion; \
             got {sr:.3}"
        );
    }
}

/// A bait-and-switch validator builds EWMA trust during a warm-up window, then
/// flips to selective failure.  The scoring system must detect the flip and
/// redirect traffic within a reasonable number of health-check rounds.
#[test]
fn sim_bait_and_switch_detected_after_flip() {
    // Flip happens at t=80s (after 8 rounds of warm-up at hc_interval=10s).
    let behaviours = vec![
        Behaviour::BaitAndSwitch {
            warm_up: Duration::from_secs(80),
        },
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let params = SimParams {
        tx_per_round: 5,
        max_retries: 2,
    };

    for seed in 0..8u64 {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut sim = make_sim(
            behaviours.clone(),
            2,
            ValidatorClientMonitorConfig::default(),
            params.clone(),
        );

        // Run through warm-up (8 rounds = 80s).  The validator behaves honestly.
        sim.run(8, &mut rng);
        let sr_honest_phase = sim.overall_success_rate();
        sim.reset_metrics();

        // Flip happens here.  Run 25 more rounds to allow detection and recovery.
        sim.run(25, &mut rng);
        let byz_frac_after = sim.byzantine_traffic_fraction();
        let sr_after_flip = sim.overall_success_rate();

        // Honest phase should have been clean.
        assert!(
            sr_honest_phase > 0.90,
            "seed={seed}: success rate during honest phase should be >90%; \
             got {sr_honest_phase:.3}"
        );
        // After detection the Byzantine validator should receive little traffic.
        assert!(
            byz_frac_after < 0.25,
            "seed={seed}: Byzantine traffic should drop after detection; \
             got {byz_frac_after:.3}"
        );
        // And transactions should succeed again (retried via honest validators).
        assert!(
            sr_after_flip > 0.70,
            "seed={seed}: success rate should recover after detection; \
             got {sr_after_flip:.3}"
        );
    }
}

/// A slow (but non-Byzantine) validator is ranked worse by scoring than honest
/// validators.  In a large committee the slow validator's exploration slot
/// competes for a first-try probability much lower than equal share, so it
/// receives strictly less traffic than each honest peer in steady state.
#[test]
fn sim_slow_validator_deprioritised_by_scoring() {
    // Use 7 validators so that Phase 1 covers the top 2–3 honest validators
    // while the slow validator only appears via exploration (1 slot).
    // With max_retries=1 only the first candidate (from a pool of ~3) is tried,
    // giving the slow validator roughly 1/3 share vs honest getting 2/3.
    // That is still less than equal share (1/7 ≈ 14%).
    let mut behaviours = vec![Behaviour::Honest; 7];
    behaviours[0] = Behaviour::Slow { factor: 8.0 };
    let params = SimParams {
        tx_per_round: 10,
        max_retries: 1,
    };

    let seeds = 8u64;
    let mut wins = 0u64; // seeds where slow_frac < avg_honest_frac

    for seed in 0..seeds {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut sim = make_sim(
            behaviours.clone(),
            2,
            ValidatorClientMonitorConfig::default(),
            params.clone(),
        );
        // Warm-up.
        sim.run(20, &mut rng);
        sim.reset_metrics();
        // Steady-state.
        sim.run(30, &mut rng);

        let slow_name = sim.validators[0].name;
        let fracs = sim.traffic_fractions();
        let slow_frac = fracs.get(&slow_name).copied().unwrap_or(0.0);
        // Average fraction across honest validators.
        let honest_fracs: Vec<f64> = sim.validators[1..]
            .iter()
            .map(|v| fracs.get(&v.name).copied().unwrap_or(0.0))
            .collect();
        let avg_honest = honest_fracs.iter().sum::<f64>() / honest_fracs.len() as f64;

        if slow_frac < avg_honest {
            wins += 1;
        }
    }

    // The slow validator should receive less traffic than the average honest
    // validator in most seeds.
    assert!(
        wins >= seeds * 6 / 10,
        "slow validator should get less traffic than avg honest in ≥60% of seeds; \
         got {wins}/{seeds}"
    );
}

/// The all-Byzantine fallback path (candidates.is_empty()) must not panic and
/// must attempt at least some transactions — the system fails gracefully rather
/// than crashing.
#[test]
fn sim_all_byzantine_fallback_does_not_panic() {
    let behaviours = vec![
        Behaviour::SelectivelyByzantine,
        Behaviour::SelectivelyByzantine,
        Behaviour::SelectivelyByzantine,
        Behaviour::SelectivelyByzantine,
    ];
    let mut rng = SmallRng::seed_from_u64(0xF00D);
    let mut sim = make_sim(
        behaviours,
        2,
        ValidatorClientMonitorConfig::default(),
        SimParams {
            tx_per_round: 5,
            max_retries: 2,
        },
    );
    // 30 rounds ensures the exclusion threshold is well exceeded and the
    // fallback path (choose from excluded pool) is exercised on every transaction.
    sim.run(30, &mut rng);

    let total_attempted: u64 = sim.fullnodes.iter().map(|f| f.metrics.submitted).sum();
    assert!(
        total_attempted > 0,
        "simulator must attempt transactions even with an all-Byzantine committee"
    );
    // All transactions fail (Byzantine committee), but the system stays alive.
    assert_eq!(
        sim.overall_success_rate(),
        0.0,
        "no transaction can succeed against an all-Byzantine committee"
    );
}

/// Traffic is distributed among multiple honest validators (not concentrated on
/// one).  `preferred_group_delta=0.02` forces a group of similar-scoring
/// validators to share traffic.
#[test]
fn sim_traffic_spread_across_honest_validators() {
    let behaviours = vec![
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let mut rng = SmallRng::seed_from_u64(42);
    let mut sim = make_sim(
        behaviours,
        3,
        ValidatorClientMonitorConfig::default(),
        SimParams {
            tx_per_round: 20,
            max_retries: 1,
        },
    );
    sim.run(30, &mut rng);

    let fracs = sim.traffic_fractions();
    // No single validator should monopolise more than 60% of traffic.
    let max_frac = fracs.values().cloned().fold(0.0f64, f64::max);
    assert!(
        max_frac < 0.60,
        "no single validator should receive >60% of traffic; max={max_frac:.3}"
    );
    // All 5 validators should receive at least some traffic.
    let validators_with_traffic = fracs.values().filter(|&&f| f > 0.0).count();
    assert_eq!(
        validators_with_traffic,
        sim.validators.len(),
        "every validator should receive some traffic"
    );
}

/// A committee with multiple Byzantine validators (but still ≤ 1/3 of total
/// stake equivalent) is handled: honest validators take over after detection.
#[test]
fn sim_minority_byzantine_validators_excluded_individually() {
    // 2 Byzantine out of 7 total (≈28%).
    let behaviours = vec![
        Behaviour::SelectivelyByzantine,
        Behaviour::SelectivelyByzantine,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let params = SimParams {
        tx_per_round: 10,
        max_retries: 3,
    };

    for seed in 0..6u64 {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut sim = make_sim(
            behaviours.clone(),
            3,
            ValidatorClientMonitorConfig::default(),
            params.clone(),
        );
        // Warm-up.
        sim.run(10, &mut rng);
        sim.reset_metrics();
        // Steady-state.
        sim.run(30, &mut rng);

        let byz_frac = sim.byzantine_traffic_fraction();
        let sr = sim.overall_success_rate();

        assert!(
            byz_frac < 0.15,
            "seed={seed}: 2-of-7 Byzantine traffic fraction should drop below 15%; \
             got {byz_frac:.3}"
        );
        assert!(
            sr > 0.75,
            "seed={seed}: success rate should recover with 2-of-7 Byzantine; \
             got {sr:.3}"
        );
    }
}

/// Scoring consistency across fullnodes: when the same committee is observed by
/// multiple independent fullnodes, they should all converge to excluding the
/// same Byzantine validator.
#[test]
fn sim_multiple_fullnodes_independently_exclude_byzantine() {
    let behaviours = vec![
        Behaviour::SelectivelyByzantine,
        Behaviour::Honest,
        Behaviour::Honest,
        Behaviour::Honest,
    ];
    let mut rng = SmallRng::seed_from_u64(77);
    let mut sim = make_sim(
        behaviours,
        4, // 4 independent fullnodes
        ValidatorClientMonitorConfig::default(),
        SimParams {
            tx_per_round: 5,
            max_retries: 2,
        },
    );
    sim.run(30, &mut rng);

    let byz_name = sim.validators[0].name;
    let elapsed = sim.clock.elapsed;

    // Every fullnode should have recorded the Byzantine validator with a high
    // failure rate (or zero recent traffic — i.e. excluded).
    let mut excluded_by_all = true;
    for (i, fn_) in sim.fullnodes.iter().enumerate() {
        let byz_traffic = fn_.metrics.traffic.get(&byz_name).copied().unwrap_or(0);
        let total_traffic: u64 = fn_.metrics.traffic.values().sum();
        let byz_frac = if total_traffic == 0 {
            0.0
        } else {
            byz_traffic as f64 / total_traffic as f64
        };
        if byz_frac > 0.30 {
            excluded_by_all = false;
            eprintln!(
                "  fullnode {i}: Byzantine traffic fraction = {byz_frac:.3} (elapsed={elapsed:?})"
            );
        }
    }
    assert!(
        excluded_by_all,
        "all fullnodes should route ≤30% traffic to Byzantine validator after 30 rounds"
    );
}
