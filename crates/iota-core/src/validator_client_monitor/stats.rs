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
    /// Construct with explicit parameters (used directly in some tests).
    /// `_latency_window_size` is accepted for API compatibility but ignored;
    /// the EWMA scorer does not use a fixed window size.
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
        // treat timeout/failure as high latency
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

    /// The main validator scoring function.
    ///
    /// Key idea: minimize expected *tail latency* under uncertainty with
    /// adversarial failures.
    ///
    /// Tail latency is the latency experienced by the slowest requests.
    /// Tail latency can be estimated as p95 quantile.
    /// Currently, the estimation is done using: μ+kσ,
    /// where μ is the mean and σ is the standard deviation of log latency.
    ///
    /// The score consists of the following metrics:
    ///
    /// Latency = exp(μ+kσ):
    ///   - EWMA score of log-latency.
    ///   - The responsiveness of the validator.
    /// Risk = λ / sqrt(n_eff + ϵ):
    ///   - Penalize low number of samples, where n_eff is effective sample
    ///     size.
    ///   - The potential for failure based on historical data.
    /// Staleness = λ (1−exp(-Δt/τ)):
    ///   - Penalize outdated stats.
    ///   - How outdated the data is.
    /// Failure = λ pf/(1-pf+ϵ):
    ///   - Penalize high failure rates, where pf is the failure rate.
    ///   - The rate of failed interactions.
    /// Congestion = λ n_inflight:
    ///   - Penalize high congestion, not estimated, always 0.
    ///   - The current load on the validator.
    /// Exploration = λ sqrt(ln(N) / (n_eff + 1)):
    ///   - Reward exploration.
    ///   - The willingness to try new paths.
    ///
    /// These score metrics should reflect validator state and behavior:
    ///
    /// - New validator, no data: high Risk, high Exploration.
    /// - Fast, well-sampled, fresh: low Latency, low Risk, low Staleness, low
    ///   Failure, low Exploration.
    /// - Slow, but reliable: high Latency, low Risk, low Failure.
    /// - High failure rate: high Failure.
    /// - Good went stale: increasing Staleness, increasing Exploration.
    ///
    /// The final score can be computed as follows:
    ///
    /// Score = Latency + Risk + Staleness + Failure + Congestion - Exploration
    fn operation_score(
        &self,
        operation: OperationType,
        total_observations: u64,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> ValidatorScore {
        let (latency, n_eff, failure_rate, alpha) = self.operation_stats(operation, now, config);
        let risk = config.risk_coeff / (n_eff + 1e-2).sqrt();
        let staleness = config.stale_coeff * alpha;
        let failure = config.failure_coeff * failure_rate / (1.0 - failure_rate + 1e-2);
        let congestion = 0.0; // we have no way to measure congestion here
        let total_requests = (total_observations + 1) as f64;
        let exploration = config.exploration_coeff * (total_requests.ln() / (n_eff + 1.0)).sqrt();

        // let exploitation = latency + risk + staleness + failure + congestion;
        // exploitation - exploration
        ValidatorScore {
            latency,
            risk,
            staleness,
            failure,
            congestion,
            exploration,
        }
    }

    fn calculate_selection_score(
        &self,
        total_observations: u64,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> f64 {
        let consensus_score = self
            .operation_score(OperationType::Consensus, total_observations, now, config)
            .score();
        let health_check_score = self
            .operation_score(OperationType::HealthCheck, total_observations, now, config)
            .score();
        let effects_score = self
            .operation_score(OperationType::Effects, total_observations, now, config)
            .score();
        let submit_score = self
            .operation_score(OperationType::Submit, total_observations, now, config)
            .score();
        consensus_score * 0.5 + health_check_score * 0.2 + effects_score * 0.2 + submit_score * 0.1
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
    pub(super) fn record_interaction_result(&mut self, feedback: &OperationFeedback) -> f64 {
        self.total_observations += 1;
        let validator_stats = self
            .validator_stats
            .entry(feedback.authority_name)
            .or_insert_with(ValidatorClientStats::new);
        validator_stats.record_interaction_result(feedback, &self.config);
        validator_stats.calculate_selection_score(
            self.total_observations,
            feedback.timestamp,
            &self.config,
        )
    }

    fn calculate_selection_score(&self, validator: &AuthorityName, now: Instant) -> f64 {
        self.validator_stats
            .get(validator)
            .map_or(self.config.no_validator_score, |stats| {
                stats.calculate_selection_score(self.total_observations, now, &self.config)
            })
    }

    fn select_top_validators<'a>(
        validator_with_scores: Vec<(&'a AuthorityName, f64)>,
        config: &ValidatorClientMonitorConfig,
    ) -> Vec<&'a AuthorityName> {
        let lowest_score = validator_with_scores[0].1;
        // compute acceptable score threshold within delta neighborhood of lowest_score
        // use this formula just in case lowest_score is negative
        let threshold = lowest_score + lowest_score.abs() * config.preferred_group_delta;

        let k = validator_with_scores
            .iter()
            .enumerate()
            .find(|(_, (_, latency))| *latency > threshold)
            .map(|(i, _)| i)
            .unwrap_or(validator_with_scores.len())
            .max(config.min_preferred_group_size);

        validator_with_scores
            .into_iter()
            .take(k)
            .map(|(v, _)| v)
            .collect()
    }

    pub(super) fn select_shuffled_preferred_validators<'a>(
        &self,
        committee: impl Iterator<Item = &'a AuthorityName>,
        now: Instant,
        mut rng: impl rand::Rng,
    ) -> Vec<&'a AuthorityName> {
        // 1. calculate scores
        let mut validator_with_scores: Vec<_> = committee
            .map(|v| (v, self.calculate_selection_score(v, now)))
            .collect();

        if validator_with_scores.is_empty() {
            return vec![];
        }
        // 2. reorder scores in ascending order, the lowest score is the best
        validator_with_scores.sort_by(|(_, latency1), (_, latency2)| {
            latency1
                .partial_cmp(latency2)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 3. select the top k validators
        let mut selected_validators =
            Self::select_top_validators(validator_with_scores, &self.config);

        // 4. shuffle to avoid prejudice and randomize selection
        selected_validators.shuffle(&mut rng);

        selected_validators
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

#[derive(Clone, Copy, Debug)]
struct ValidatorScore {
    latency: f64,
    risk: f64,
    staleness: f64,
    failure: f64,
    congestion: f64,
    exploration: f64,
}

impl ValidatorScore {
    fn score(&self) -> f64 {
        let exploitation =
            self.latency + self.risk + self.staleness + self.failure + self.congestion;
        exploitation - self.exploration
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
