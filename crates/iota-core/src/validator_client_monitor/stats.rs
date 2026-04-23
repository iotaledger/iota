// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::base_types::AuthorityName;
use rand::seq::SliceRandom;
use tracing::debug;

use crate::validator_client_monitor::{OperationFeedback, OperationType};

/// Coefficient controlling risk score sensitivity.
const RISK_COEFF: f64 = 50.0;

/// Coefficient controlling staleness score sensitivity.
const STALE_COEFF: f64 = 100.0;

/// Coefficient controlling failure score sensitivity.
const FAILURE_COEFF: f64 = 500.0;

/// Coefficient controlling exploration score sensitivity.
const EXPLORATION_COEFF: f64 = 20.0;

/// Latency score coefficient.
///
/// Higher value means more penalty for variability and less confidence in the estimate.
const LATENCY_EWMA_SCORE_COEFF: f64 = 2.0;

/// Base latency score for a validator with no observations.
///
/// Higher than any reasonable latency.
const EMPTY_LATENCY_SCORE: f64 = 10.0;

/// Base performance score for a validator with no observations.
const UNKNOWN_VALIDATOR_SCORE: f64 = 100.0;

/// History decay factor τ for EWMA latency estimation (0 < τ ≤ ∞).
///
/// Lower τ means faster decaying and more weight on recent observations.
const LATENCY_EWMA_TAU: f64 = 60.0;

/// Failure rate above which a validator is excluded from selection
/// entirely (Phase 0).
///
/// Exclusion is only applied once `EXCLUSION_MIN_N_EFF` HealthCheck
/// samples have been collected, so a brief outage at startup does not
/// permanently ban a validator.
const EXCLUSION_FAILURE_THRESHOLD: f64 = 0.7;

/// Coefficient for the selective-failure penalty.
///
/// A validator that passes HealthCheck reliably but fails work operations
/// (Submit / Consensus / Effects) is likely misbehaving selectively.
/// This coefficient scales the resulting additional penalty.
const SELECTIVE_FAILURE_COEFF: f64 = 500.0;

/// Minimum failure-rate gap between work operations and HealthCheck before
/// the selective-failure penalty applies.
///
/// Acts as a noise floor: a gap smaller than this threshold is attributed
/// to statistical variance and does not trigger the penalty.
/// Example: 0.1 means work ops must fail at least 10 percentage points more
/// often than HealthCheck before the inconsistency is flagged.
const SELECTIVE_FAILURE_NOISE_THRESHOLD: f64 = 0.1;

/// Minimum HealthCheck effective sample size (n_eff) required before the
/// selective-failure penalty is applied at full weight.
///
/// The penalty is scaled by `min(1, n_eff_healthcheck / this_value)`,
/// so it is fully suppressed below a small number of HealthCheck
/// observations and ramps up linearly to full strength once this threshold
/// is reached.
const SELECTIVE_FAILURE_MIN_N_EFF: f64 = 5.0;

/// Minimum HealthCheck effective sample size required before the
/// exclusion threshold is enforced.
///
/// Guards against excluding a validator on the basis of a handful of
/// failures that may not be representative.
const EXCLUSION_MIN_N_EFF: f64 = 5.0;

/// Number of exploration slots: how many validators are selected in
/// Phase 2 (from the non-excluded, non-Phase-1 remainder) purely on the
/// basis of their exploration bonus.
const MAX_EXPLORATION_GROUP_SIZE: usize = 1;

/// Minimum exploration threshold: the minimum exploration bonus a validator
/// must have to be considered for selection in Phase 2.
const MIN_EXPLORATION_THRESHOLD: f64 = 20.0;

/// Expected baseline latency for Submit operations (seconds).
/// Used to normalise raw latency scores to a dimensionless ratio
/// (actual / expected), making all operation types directly comparable.
const EXPECTED_LATENCY_SUBMIT_SECS: f64 = 0.15;

/// Expected baseline latency for Effects operations (seconds).
const EXPECTED_LATENCY_EFFECTS_SECS: f64 = 1.5;

/// Expected baseline latency for HealthCheck operations (seconds).
const EXPECTED_LATENCY_HEALTHCHECK_SECS: f64 = 0.1;

/// Expected baseline latency for Consensus operations (seconds).
const EXPECTED_LATENCY_CONSENSUS_SECS: f64 = 0.8;

/// Minimum number of validators that must appear in the shuffled
/// preferred group returned by `select_shuffled_preferred_validators`,
/// preventing a single validator from monopolising all traffic.
const MIN_PREFERRED_GROUP_SIZE: usize = 2;

/// Maximum number of validators that can be included in the preferred group.
const MAX_PREFERRED_GROUP_SIZE: usize = 10;

/// Preferred group size adjustment factor.
const PREFERRED_GROUP_DELTA: f64 = 0.02;

/// Ok(latency in sec) or Err(high failure latency in sec)
type Observation = Result<f64, f64>;

/// Exponential weighted moving average (EWMA) estimator.
///
/// Each new observation is weighted by α and the prior estimate by (1 − α):
/// μ_t ​= α x_t​ + (1−α) μ_{t−1}​
/// σ_t^2​ = (1−α) (σ_{t−1}^2 ​+ α (x_t​−μ_{t−1}​)^2)
#[derive(Clone, Copy, Debug)]
struct Ewma {
    /// Current mean estimate: μ_t.
    mean: f64,
    /// Current variance estimate: σ_t^2.
    variance: f64,
    /// Failure mean estimate.
    failure: f64,
    /// Decayed/effective sample size.
    weight: f64,
}

impl Ewma {
    fn first_value(observation: Observation) -> Self {
        Self {
            mean: observation.unwrap_or_else(|failure| failure),
            variance: 0.0,
            failure: observation.map_or(1.0, |_| 0.0),
            weight: 1.0,
        }
    }

    fn stddev(&self) -> f64 {
        self.variance.sqrt()
    }

    fn update(&mut self, observation: Observation, alpha: f64) {
        let a1 = 1.0 - alpha;
        // treat failures as high latency
        let x = observation.unwrap_or_else(|failure| failure);
        // μ_t ​= α x_t​ + (1−α) μ_{t−1}​ = μ_{t−1}​ + α (x_t​ - μ_{t−1})
        let delta = x - self.mean;
        self.mean += alpha * delta;
        // σ_t^2​ = (1−α) (σ_{t−1}^2 ​+ α (x_t​−μ_{t−1}​)^2) =
        self.variance = a1 * (self.variance + alpha * delta * delta);
        // failures EWMA
        self.failure = a1 * self.failure + observation.map_or(alpha, |_| 0.0);
        // weight is just EWMA of the count
        // w_t = (1 - α) w_{t-1} + 1
        self.weight = a1 * self.weight + 1.0;
    }

    fn score(&self, k: f64) -> f64 {
        // Score = mean + k * stddev.  Higher k means more penalty for
        // variability and less confidence in the estimate.
        self.mean + k * self.stddev()
    }

    fn stats(&self, k: f64) -> (f64, f64, f64) {
        (self.score(k), self.weight, self.failure)
    }
}

/// Time-decayed EWMA-based estimator.
///
/// This is a EWMA estimator with a variable weight α_t (interval between updates):
/// α_t = 1 - exp(-Δt / τ)
#[derive(Clone, Copy, Debug)]
struct TimeDecayEwma {
    /// Base EWMA estimator.
    ewma: Ewma,
    /// Timestamp of the last update: t.
    last_update: Instant,
    /// EWMA of normalized observations interval (Δt/τ).
    /// None until the second observation.
    int_ewma: Option<f64>,
}

impl TimeDecayEwma {
    fn first_value(observation: Observation, now: Instant) -> Self {
        Self {
            ewma: Ewma::first_value(observation),
            last_update: now,
            int_ewma: None,
        }
    }

    fn int_alpha(&self, now: Instant, tau: f64) -> (f64, f64) {
        debug_assert!(now >= self.last_update, "Timestamps must be non-decreasing");
        // α_t = 1 - exp(-Δt / τ)
        // avoid zero Δt, otherwise (α_t = 0) observation won't be updated
        let dt = now.duration_since(self.last_update).as_secs_f64().max(1e-9);
        let int = dt / tau;
        (int, 1.0 - (-int).exp())
    }

    fn update_with_time_decay(&mut self, observation: Observation, now: Instant, tau: f64) {
        let (int, alpha) = self.int_alpha(now, tau);
        // Update the value EWMA with time-derived alpha.
        self.ewma.update(observation, alpha);
        self.last_update = now;
        // Update the interval EWMA with a fixed weight of 0.1.
        const ALPHA: f64 = 0.1;
        self.int_ewma = Some(
            self.int_ewma
                .map_or(int, |prev| (1.0 - ALPHA) * prev + ALPHA * int),
        );
    }

    fn stats(&self, k: f64, now: Instant, tau: f64) -> (f64, f64, f64, f64, Option<f64>) {
        let (score, weight, failure) = self.ewma.stats(k);
        (
            score,
            weight,
            failure,
            self.int_alpha(now, tau).1,
            self.int_ewma,
        )
    }
}

/// Latency estimator is based on time-decayed EWMA.
#[derive(Clone, Copy, Debug, Default)]
struct LatencyEwma {
    inner: Option<TimeDecayEwma>,
}

impl LatencyEwma {
    fn new() -> Self {
        Self { inner: None }
    }

    fn update(&mut self, observation: Observation, timestamp: Instant, tau: f64) {
        if let Some(inner) = &mut self.inner {
            inner.update_with_time_decay(observation, timestamp, tau);
        } else {
            self.inner = Some(TimeDecayEwma::first_value(observation, timestamp));
        }
    }

    fn stats(&self, k: f64, now: Instant, tau: f64) -> Option<(f64, f64, f64, f64, Option<f64>)> {
        self.inner
            .map(|time_decay_ewma| time_decay_ewma.stats(k, now, tau))
    }
}

/// Latency estimator with logarithmic observations.
///
/// Logarithmic observations help smooth out high variability in latency measurements.
/// For regular observations LatencyEwma can be used.
#[derive(Clone, Copy, Debug, Default)]
struct LogLatencyEwma {
    inner: LatencyEwma,
}

impl LogLatencyEwma {
    fn new() -> Self {
        Self {
            inner: LatencyEwma::new(),
        }
    }

    fn update(&mut self, observation: Observation, timestamp: Instant, tau: f64) {
        // Avoid ln(0) by capping at a small positive value.
        let log_observation = observation
            .map(|x| x.max(1e-9).ln())
            .map_err(|x| x.max(1e-9).ln());
        self.inner.update(log_observation, timestamp, tau);
    }

    fn stats(&self, k: f64, now: Instant, tau: f64) -> Option<(f64, f64, f64, f64, Option<f64>)> {
        self.inner
            .stats(k, now, tau)
            .map(|(score, weight, failure, alpha, int_ewma)| {
                (score.exp(), weight, failure, alpha, int_ewma)
            })
    }
}

/// Complete client-observed statistics for validator interactions.
#[derive(Debug, Clone)]
pub(super) struct ClientObservedStats {
    /// Per-validator statistics mapping authority names to their
    /// client-observed metrics
    validator_stats: HashMap<AuthorityName, ValidatorClientStats>,
    total_observations: u64,
    /// Configuration parameters for scoring and exclusion policies
    config: ValidatorClientMonitorConfig,
}

/// Client-observed stats for a single validator.
#[derive(Debug, Clone)]
struct ValidatorClientStats {
    /// Latency estimators per operation type.
    latency_per_operation: [LogLatencyEwma; 4],
}

impl ValidatorClientStats {
    fn new() -> Self {
        Self {
            latency_per_operation: [LogLatencyEwma::new(); 4],
        }
    }

    fn record_interaction_result(
        &mut self,
        feedback: &OperationFeedback,
        config: &ValidatorClientMonitorConfig,
    ) {
        // Treat timeout/failure as high latency
        let failure_latency = config.health_check_timeout.as_secs_f64() * 3.0;
        let observation = feedback
            .result
            .map(|latency| latency.as_secs_f64())
            .map_err(|_| failure_latency);
        assert!((feedback.operation as usize) < self.latency_per_operation.len());
        self.latency_per_operation[feedback.operation as usize].update(
            observation,
            feedback.timestamp,
            LATENCY_EWMA_TAU,
        );
    }

    fn operation_stats(
        &self,
        operation: OperationType,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> (f64, f64, f64, f64, Option<f64>) {
        assert!((operation as usize) < self.latency_per_operation.len());
        self.latency_per_operation[operation as usize]
            .stats(
                LATENCY_EWMA_SCORE_COEFF,
                now,
                LATENCY_EWMA_TAU,
            )
            .unwrap_or((EMPTY_LATENCY_SCORE, 0.0, 0.0, 1.0, None))
    }

    #[cfg(test)]
    pub(super) fn calculate_selection_score(
        &self,
        total_observations: u64,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> f64 {
        self.performance_score(total_observations, now, config)
            .map_or(
                UNKNOWN_VALIDATOR_SCORE,
                |(exploitation, exploration)| (exploitation - exploration).max(0.0),
            )
    }

    fn performance_score(
        &self,
        total_observations: u64,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> Option<(f64, f64)> {
        let (l_sub, n_sub, f_sub, a_sub, _) =
            self.operation_stats(OperationType::Submit, now, config);
        let (l_eff, n_eff_e, f_eff, a_eff, _) =
            self.operation_stats(OperationType::Effects, now, config);
        let (l_hc, n_hc, f_hc, a_hc, _) =
            self.operation_stats(OperationType::HealthCheck, now, config);
        let (l_con, n_con, f_con, a_con, _) =
            self.operation_stats(OperationType::Consensus, now, config);

        let f_work = f_sub.max(f_eff).max(f_con);
        let f_max = f_work.max(f_hc);
        if n_hc > EXCLUSION_MIN_N_EFF && f_max > EXCLUSION_FAILURE_THRESHOLD {
            // Exclusion check: return None when this validator should be excluded
            // from selection entirely, computing its other scores makes no sense.
            //
            // A validator is excluded when its maximum per-operation failure rate
            // exceeds `EXCLUSION_FAILURE_THRESHOLD` AND enough HealthCheck samples
            // have been collected to make that judgement reliable.
            None
        } else {
            // Latency = Σ_op w_op * exp(μ_op + k·σ_op) / expected_latency_op:
            //   Weighted sum of per-operation EWMA latency scores normalised to a
            //   dimensionless ratio (actual / expected).  1.0 = at expected latency.
            let latency = l_con / EXPECTED_LATENCY_CONSENSUS_SECS * 0.5
                + l_hc / EXPECTED_LATENCY_HEALTHCHECK_SECS * 0.2
                + l_eff / EXPECTED_LATENCY_EFFECTS_SECS * 0.2
                + l_sub / EXPECTED_LATENCY_SUBMIT_SECS * 0.1;

            // Risk = coeff · sqrt( Σ_op (w_op / sqrt(n_eff_op + ε))² ):
            //   Each operation contributes uncertainty proportional to its
            //   latency weight divided by the square root of its effective
            //   sample count.  This avoids the n_eff_min bottleneck: a single
            //   sparsely-sampled operation (Submit / Consensus, which arrive at
            //   rate tx_rate/N) no longer caps the score indefinitely; its
            //   contribution decays naturally as n_eff grows.  Weights match the
            //   latency coefficients so the risk budget mirrors the latency budget.
            let u_con = 0.5 / (n_con + 1e-2).sqrt();
            let u_hc = 0.2 / (n_hc + 1e-2).sqrt();
            let u_eff = 0.2 / (n_eff_e + 1e-2).sqrt();
            let u_sub = 0.1 / (n_sub + 1e-2).sqrt();
            let risk = RISK_COEFF
                * (u_con * u_con + u_hc * u_hc + u_eff * u_eff + u_sub * u_sub).sqrt();

            // Keep n_eff_min for exploration: a validator under-sampled in any
            // single operation is a good candidate for exploration regardless of
            // how well the other operations are known.
            let n_eff_min = n_sub.min(n_eff_e).min(n_hc).min(n_con);

            // Staleness = λ * max_op(1 − exp(−Δt_op / τ)):
            //   Driven by the stalest operation.
            let alpha_max = a_sub.max(a_eff).max(a_hc).max(a_con);
            let staleness = STALE_COEFF * alpha_max;

            // Failure = λ * pf_max / (1 − pf_max + ϵ):
            //   Driven by the highest per-operation failure rate.
            let failure = FAILURE_COEFF * f_max / (1.0 - f_max + 1e-2);

            // SelectiveFailure = λ * max(0, f_work − f_hc − ε) * min(1, n_hc / n_min):
            //   Extra penalty when HealthCheck passes but work operations fail —
            //   the signature of a validator selectively refusing transactions.
            let inconsistency = (f_work - f_hc - SELECTIVE_FAILURE_NOISE_THRESHOLD).max(0.0);
            let confidence = (n_hc / SELECTIVE_FAILURE_MIN_N_EFF).min(1.0);
            let selective_failure = SELECTIVE_FAILURE_COEFF * inconsistency * confidence;

            // Exploitation is the main performance score.
            let exploitation = latency + risk + staleness + failure + selective_failure;

            // Exploration = λ * sqrt(ln(N) / (min_op(n_eff_op) + 1)):
            //   Under-sampling any single operation makes this validator a good
            //   candidate for exploration.
            let total_requests = (total_observations + 1) as f64;
            let exploration =
                EXPLORATION_COEFF * (total_requests.ln() / (n_eff_min + 1.0)).sqrt();

            // The final score is a combination of exclusion, exploitation, and exploration.
            Some((exploitation, exploration))
        }
    }
}

impl ClientObservedStats {
    pub(super) fn new(config: ValidatorClientMonitorConfig) -> Self {
        Self {
            validator_stats: HashMap::new(),
            total_observations: 0,
            config,
        }
    }

    /// Record client-observed interaction result with a validator.
    pub(super) fn record_interaction_result(
        &mut self,
        feedback: &OperationFeedback,
    ) -> Option<(f64, f64)> {
        self.total_observations += 1;
        let validator_stats = self
            .validator_stats
            .entry(feedback.authority_name)
            .or_insert_with(ValidatorClientStats::new);
        validator_stats.record_interaction_result(feedback, &self.config);
        validator_stats.performance_score(self.total_observations, feedback.timestamp, &self.config)
    }

    #[cfg(test)]
    pub(super) fn calculate_selection_score(&self, validator: &AuthorityName, now: Instant) -> f64 {
        self.validator_stats
            .get(validator)
            .map_or(UNKNOWN_VALIDATOR_SCORE, |stats| {
                stats.calculate_selection_score(self.total_observations, now, &self.config)
            })
    }

    pub(super) fn select_shuffled_preferred_validators<'a>(
        &self,
        committee: impl Iterator<Item = &'a AuthorityName>,
        now: Instant,
        mut rng: impl rand::Rng,
    ) -> Vec<&'a AuthorityName> {
        // Phase 0 — Exclusion: partition validators into excluded / candidates.
        let unknown_exploration =
            EXPLORATION_COEFF * ((self.total_observations + 1) as f64).ln().sqrt();
        let mut excluded: Vec<&'a AuthorityName> = vec![];
        let mut candidates: Vec<(&'a AuthorityName, f64, f64)> = vec![];
        for v in committee {
            if let Some(stats) = self.validator_stats.get(v) {
                if let Some((exploitation, exploration)) =
                    stats.performance_score(self.total_observations, now, &self.config)
                {
                    // known validator with reasonable scores
                    candidates.push((v, exploitation, exploration));
                } else {
                    // excluded known validator, too many failures
                    excluded.push(v);
                }
            } else {
                // unknown validator: sentinel exploitation score, maximum
                // exploration bonus so it is picked up quickly.
                candidates.push((v, UNKNOWN_VALIDATOR_SCORE, unknown_exploration));
            }
        }

        if candidates.is_empty() {
            // this looks like a total blackout, fallback -- still try random excluded
            // validators
            let amount = MAX_EXPLORATION_GROUP_SIZE.clamp(
                MIN_PREFERRED_GROUP_SIZE,
                MAX_PREFERRED_GROUP_SIZE,
            );
            return excluded
                .choose_multiple(&mut rng, amount)
                .cloned()
                .collect();
        }

        // Phase 1 — Exploitation: sort by performance ascending, select top
        // group within `PREFERRED_GROUP_DELTA` of the best score.
        candidates.sort_by(|(_, p1, _), (_, p2, _)| {
            // ascending exploitation order (lower is better)
            p1.partial_cmp(p2).unwrap_or(std::cmp::Ordering::Equal)
        });
        // Relative threshold: within `PREFERRED_GROUP_DELTA` fraction of the
        // best score.  All candidates whose perf score is at most this value
        // join Phase 1.
        let perf_threshold = candidates[0].1 * (1.0 + PREFERRED_GROUP_DELTA);
        let phase1_count = candidates
            .iter()
            .enumerate()
            .find(|(_, (_, perf, _))| *perf > perf_threshold)
            .map(|(i, _)| i)
            .unwrap_or(candidates.len())
            .clamp(
                MIN_PREFERRED_GROUP_SIZE,
                MAX_PREFERRED_GROUP_SIZE,
            )
            // Never exceed the actual number of candidates — guards against
            // MIN_PREFERRED_GROUP_SIZE > committee size.
            .min(candidates.len());

        // Phase 2 — Exploration: from the remainder, pick `MAX_EXPLORATION_GROUP_SIZE`
        // validators with the highest exploration bonus.
        let remainder = &mut candidates[phase1_count..];
        remainder.sort_by(|(_, _, e1), (_, _, e2)| {
            // descending exploration order
            e2.partial_cmp(e1).unwrap_or(std::cmp::Ordering::Equal)
        });
        let phase2_count = candidates[phase1_count..]
            .iter()
            .enumerate()
            .find(|(_, (_, _, exploration))| *exploration < MIN_EXPLORATION_THRESHOLD)
            .map(|(i, _)| i)
            .unwrap_or(candidates.len() - phase1_count)
            .min(MAX_EXPLORATION_GROUP_SIZE);

        // Merge Phase 1 + Phase 2 and shuffle to avoid systematic bias.
        let mut selected: Vec<&'a AuthorityName> = candidates
            .into_iter()
            .take(phase1_count + phase2_count)
            .map(|(v, _, _)| v)
            .collect();
        selected.shuffle(&mut rng);
        selected
    }

    /// Retain only the specified validators, removing any others.
    pub(super) fn retain_validators<'a>(
        &mut self,
        validators: impl Iterator<Item = &'a AuthorityName>,
    ) {
        let cur_len = self.validator_stats.len();
        let validator_set: HashSet<_> = validators.collect();
        self.validator_stats
            .retain(|validator, _| validator_set.contains(validator));
        let removed_count = cur_len - self.validator_stats.len();
        if removed_count > 0 {
            debug!("Removed {} stale validator data", removed_count);
        }
    }

    /// Remove the specified validators, retaining any others.
    #[cfg(test)]
    pub(super) fn remove_validators<'a>(
        &mut self,
        validators: impl Iterator<Item = &'a AuthorityName>,
    ) {
        let mut removed_count = 0;
        for validator in validators {
            if self.validator_stats.remove(validator).is_some() {
                removed_count += 1;
            }
        }
        if removed_count > 0 {
            debug!("Removed {} stale validator data", removed_count);
        }
    }

    #[cfg(test)]
    pub(super) fn has_validator(&self, validator: &AuthorityName) -> bool {
        self.validator_stats.contains_key(validator)
    }

    #[cfg(test)]
    pub(super) fn num_validators(&self) -> usize {
        self.validator_stats.len()
    }
}

#[cfg(test)]
mod tests;
