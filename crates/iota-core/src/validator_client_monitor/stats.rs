// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
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

/// Ok(latency in sec) or Err(high failure latency in sec)
type Observation = Result<f64, f64>;

/// EWMA-based estimator.
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
    /// Failure estimate.
    failure: f64,
    /// Decayed/effective sample size.
    weight: f64,
    /// Total number of observations recorded so far.
    count: u64,
}

impl Ewma {
    fn first_value(observation: Observation) -> Self {
        Self {
            mean: observation.unwrap_or_else(|failure| failure),
            variance: 0.0,
            failure: observation.map_or(1.0, |_| 0.0),
            weight: 1.0,
            count: 1,
        }
    }

    fn stddev(&self) -> f64 {
        self.variance.sqrt()
    }

    fn update(&mut self, observation: Observation, alpha: f64) {
        let a1 = 1.0 - alpha;
        // failures EWMA
        self.failure = a1 * self.failure + observation.map_or(alpha, |_| 0.0);
        // treat failures as high latency
        let x = observation.unwrap_or_else(|failure| failure);
        // μ_t ​= α x_t​ + (1−α) μ_{t−1}​ = μ_{t−1}​ + α (x_t​ - μ_{t−1})
        let delta = x - self.mean;
        self.mean += alpha * delta;
        // σ_t^2​ = (1−α) (σ_{t−1}^2 ​+ α (x_t​−μ_{t−1}​)^2) =
        self.variance = a1 * (self.variance + alpha * delta * delta);
        // weight is just EWMA of the count
        // w_t = (1 - α) w_{t-1} + 1
        self.weight = a1 * self.weight + 1.0;
        self.count += 1;
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
/// This is a EWMA estimator with a variable weight α_t:
/// α_t = 1 - exp(-Δt / τ)
#[derive(Clone, Copy, Debug)]
struct TimeDecayEwma {
    ewma: Ewma,
    /// Timestamp of the last update: t.
    last_update: Instant,
}

impl TimeDecayEwma {
    fn first_value(observation: Observation, now: Instant) -> Self {
        Self {
            ewma: Ewma::first_value(observation),
            last_update: now,
        }
    }

    fn alpha(&self, now: Instant, tau: f64) -> f64 {
        debug_assert!(now >= self.last_update, "Timestamps must be non-decreasing");
        // α_t = 1 - exp(-Δt / τ)
        // avoid zero Δt, otherwise (α_t = 0) observation won't be updated
        let dt = now.duration_since(self.last_update).as_secs_f64().max(1e-9);
        1.0 - (-dt / tau).exp()
    }
    fn update_with_time_decay(&mut self, observation: Observation, now: Instant, tau: f64) {
        self.ewma.update(observation, self.alpha(now, tau));
        self.last_update = now;
    }

    fn stats(&self, k: f64, now: Instant, tau: f64) -> (f64, f64, f64, f64) {
        let (score, weight, failure) = self.ewma.stats(k);
        (score, weight, failure, self.alpha(now, tau))
    }
}

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

    fn stats(&self, k: f64, now: Instant, tau: f64) -> Option<(f64, f64, f64, f64)> {
        self.inner
            .map(|time_decay_ewma| time_decay_ewma.stats(k, now, tau))
    }
}

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

    fn stats(&self, k: f64, now: Instant, tau: f64) -> Option<(f64, f64, f64, f64)> {
        self.inner
            .stats(k, now, tau)
            .map(|(score, weight, failure, alpha)| (score.exp(), weight, failure, alpha))
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
            config.latency_ewma_tau,
        );
    }

    fn operation_stats(
        &self,
        operation: OperationType,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> (f64, f64, f64, f64) {
        assert!((operation as usize) < self.latency_per_operation.len());
        self.latency_per_operation[operation as usize]
            .stats(
                config.latency_ewma_score_coeff,
                now,
                config.latency_ewma_tau,
            )
            .unwrap_or((config.empty_latency_score, 0.0, 0.0, 1.0))
    }

    fn performance_score(&self, total_observations: u64, now: Instant, config: &ValidatorClientMonitorConfig) -> Option<(f64, f64)> {
        let (l_sub, n_sub, f_sub, a_sub) =
            self.operation_stats(OperationType::Submit, now, config);
        let (l_eff, n_eff_e, f_eff, a_eff) =
            self.operation_stats(OperationType::Effects, now, config);
        let (l_hc, n_hc, f_hc, a_hc) =
            self.operation_stats(OperationType::HealthCheck, now, config);
        let (l_con, n_con, f_con, a_con) =
            self.operation_stats(OperationType::Consensus, now, config);

        let f_work = f_sub.max(f_eff).max(f_con);
        let f_max = f_work.max(f_hc);
        if n_hc > config.exclusion_min_n_eff && f_max > config.exclusion_failure_threshold {
            // Exclusion check: return None when this validator should be excluded
            // from selection entirely, computing its other scores makes no sense.
            //
            // A validator is excluded when its maximum per-operation failure rate
            // exceeds `exclusion_failure_threshold` AND enough HealthCheck samples
            // have been collected to make that judgement reliable.
            None
        } else {
            // Latency = Σ_op w_op * exp(μ_op + k·σ_op) / expected_latency_op:
            //   Weighted sum of per-operation EWMA latency scores normalised to a
            //   dimensionless ratio (actual / expected).  1.0 = at expected latency.
            let latency = l_con / config.expected_latency_consensus_secs * 0.5
                + l_hc / config.expected_latency_healthcheck_secs * 0.2
                + l_eff / config.expected_latency_effects_secs * 0.2
                + l_sub / config.expected_latency_submit_secs * 0.1;

            // Risk = λ / sqrt(min_op(n_eff_op) + ϵ):
            //   Driven by the least-sampled operation — confidence is limited by
            //   the operation we know least about.
            let n_eff_min = n_sub.min(n_eff_e).min(n_hc).min(n_con);
            let risk = config.risk_coeff / (n_eff_min + 1e-2).sqrt();

            // Staleness = λ * max_op(1 − exp(−Δt_op / τ)):
            //   Driven by the stalest operation.
            let alpha_max = a_sub.max(a_eff).max(a_hc).max(a_con);
            let staleness = config.stale_coeff * alpha_max;

            // Failure = λ * pf_max / (1 − pf_max + ϵ):
            //   Driven by the highest per-operation failure rate.
            let failure = config.failure_coeff * f_max / (1.0 - f_max + 1e-2);

            // SelectiveFailure = λ * max(0, f_work − f_hc − ε) * min(1, n_hc / n_min):
            //   Extra penalty when HealthCheck passes but work operations fail —
            //   the signature of a validator selectively refusing transactions.
            let inconsistency =
                (f_work - f_hc - config.selective_failure_noise_threshold).max(0.0);
            let confidence = (n_hc / config.selective_failure_min_n_eff).min(1.0);
            let selective_failure = config.selective_failure_coeff * inconsistency * confidence;

            // Exploitation is the main performance score.
            let exploitation = latency + risk + staleness + failure + selective_failure;

            // Exploration = λ * sqrt(ln(N) / (min_op(n_eff_op) + 1)):
            //   Under-sampling any single operation makes this validator a good
            //   candidate for exploration.
            let total_requests = (total_observations + 1) as f64;
            let exploration = config.exploration_coeff * (total_requests.ln() / (n_eff_min + 1.0)).sqrt();

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
    pub(super) fn record_interaction_result(&mut self, feedback: &OperationFeedback) {
        self.total_observations += 1;
        let validator_stats = self
            .validator_stats
            .entry(feedback.authority_name)
            .or_insert_with(ValidatorClientStats::new);
        validator_stats.record_interaction_result(feedback, &self.config);
    }

    #[cfg(test)]
    fn calculate_selection_score(&self, validator: &AuthorityName, now: Instant) -> f64 {
        self.validator_stats
            .get(validator)
            .map_or(self.config.no_validator_score, |stats| {
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
        let exploration = self.config.exploration_coeff
            * ((self.total_observations + 1) as f64).ln().sqrt();
        let mut excluded: Vec<&'a AuthorityName> = vec![];
        let mut candidates: Vec<(&'a AuthorityName, f64, f64)> = vec![];
        for v in committee {
            if let Some(stats) = self.validator_stats.get(v) {
                if let Some((exploitation, exploration)) = stats.performance_score(self.total_observations, now, &self.config) {
                    // known validator with reasonable scores
                    candidates.push((v, exploitation, exploration));
                } else {
                    // excluded known validator, too many failures
                    excluded.push(v);
                }
            } else {
                // unknown validator: sentinel exploitation score, maximum
                // exploration bonus so it is picked up quickly.
                candidates.push((v, self.config.no_validator_score, exploration));
            }
        }

        if candidates.is_empty() {
            // this looks like a total blackout, fallback -- still try random excluded validators
            return excluded.choose_multiple(&mut rng, self.config.max_exploration_group_size).cloned().collect();
        }

        // Phase 1 — Exploitation: sort by performance ascending, select top
        // group within `preferred_group_delta` of the best score.
        candidates.sort_by(|(_, p1, _), (_, p2, _)| {
            // ascending exploitation order (lower is better)
            p1.partial_cmp(p2).unwrap_or(std::cmp::Ordering::Equal)
        });
        // Relative threshold: within `preferred_group_delta` fraction of the
        // best score.  All candidates whose perf score is at most this value
        // join Phase 1.
        let perf_threshold = candidates[0].1 * (1.0 + self.config.preferred_group_delta);
        let phase1_count = candidates
            .iter()
            .enumerate()
            .find(|(_, (_, perf, _))| *perf > perf_threshold)
            .map(|(i, _)| i)
            .unwrap_or(candidates.len())
            .max(self.config.min_preferred_group_size)
            .min(self.config.max_preferred_group_size);

        // Phase 2 — Exploration: from the remainder, pick `max_exploration_group_size`
        // validators with the highest exploration bonus.
        let remainder = &mut candidates[phase1_count..];
        remainder.sort_by(|(_, _, e1), (_, _, e2)| {
            // descending exploration order
            e2.partial_cmp(e1).unwrap_or(std::cmp::Ordering::Equal)
        });
        let phase2_count = candidates[phase1_count..]
            .iter()
            .enumerate()
            .find(|(_, (_, _, exploration))| *exploration < self.config.min_exploration_threshold)
            .map(|(i, _)| i)
            .unwrap_or(candidates.len() - phase1_count)
            .min(self.config.max_exploration_group_size);

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
mod tests {
    use super::*;

    fn create_test_validator_names(n: usize) -> Vec<AuthorityName> {
        (0..n)
            .map(|_| {
                let (_, key_pair): (_, AuthorityKeyPair) = get_key_pair();
                key_pair.public().into()
            })
            .collect()
    }

    #[test]
    fn test_client_stats_record_success() {
        let config = ValidatorClientMonitorConfig::default();
        let mut stats = ClientObservedStats::new(config);

        let validators = create_test_validator_names(1);
        let validator = validators[0];

        let now = Instant::now();
        let feedback = OperationFeedback::builder(
            validator,
            validator.concise().to_string(),
            OperationType::Submit,
        )
        .ok_at(Duration::from_millis(100), now);

        let score = stats.record_interaction_result(feedback);
        let score2 = stats.calculate_selection_score(&validator, now);

        assert_eq!((score - score2).abs() < f64::EPSILON, true);
    }
}
