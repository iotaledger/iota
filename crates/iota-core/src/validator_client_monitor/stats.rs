// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::base_types::AuthorityName;
use tracing::debug;

use crate::validator_client_monitor::{OperationFeedback, OperationType};

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
    /// Decayed sample size.
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
/// Each new observation is weighted by α and the prior estimate by (1 − α):
/// μ_t ​= α_t x_t​ + (1−α_t) μ_{t−1}​
/// σ_t^2​ = (1−α_t) (σ_{t−1}^2 ​+ α_t(x_t​−μ_{t−1}​)^2)
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
        let dt = now.duration_since(self.last_update).as_secs_f64(); //.max(1e-9)
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
        let log_observation = observation.map(|x| x.max(1e-9).ln());
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
            .unwrap_or((config.empty_latency_score, 0.0, 0.0, 0.0))
    }

    fn operation_score(
        &self,
        operation: OperationType,
        total_observations: u64,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> f64 {
        let (latency, n_eff, failure_rate, alpha) = self.operation_stats(operation, now, config);
        let risk = config.risk_coeff / (n_eff + 1e-2).sqrt();
        let staleness = config.stale_coeff * alpha;
        let failure = config.failure_coeff * failure_rate / (1.0 - failure_rate + 1e-2);
        let congestion = 0.0; // we have no way to measure congestion here
        let total_requests = total_observations as f64;
        let exploration = config.exploration_coeff * (total_requests.ln() / (n_eff + 1.0)).sqrt();

        latency + risk + staleness + failure + congestion - exploration
    }

    fn calculate_selection_score(
        &self,
        total_observations: u64,
        now: Instant,
        config: &ValidatorClientMonitorConfig,
    ) -> f64 {
        let consensus_score =
            self.operation_score(OperationType::Consensus, total_observations, now, config);
        let health_check_score =
            self.operation_score(OperationType::HealthCheck, total_observations, now, config);
        let effects_score =
            self.operation_score(OperationType::Effects, total_observations, now, config);
        consensus_score * 0.6 + health_check_score * 0.2 + effects_score * 0.2
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

    /// Calculate a simpler selection score (EWMA + reliability penalty only,
    /// no confidence penalty) for use in
    /// `select_shuffled_preferred_validators`.
    ///
    /// This keeps the selection ordering stable and proportional to actual
    /// observed latency once a validator has any data at all.
    pub(super) fn calculate_selection_score(&self, validator: &AuthorityName, now: Instant) -> f64 {
        self.validator_stats
            .get(validator)
            .map_or(self.config.no_validator_score, |stats| {
                stats.calculate_selection_score(self.total_observations, now, &self.config)
            })
    }

    /// Retain only the specified validators, removing any others.
    pub(super) fn retain_validators<'a>(&mut self, current_validators: impl Iterator<Item = &'a AuthorityName>) {
        let cur_len = self.validator_stats.len();
        let validator_set: HashSet<_> = current_validators.collect();
        self.validator_stats
            .retain(|validator, _| validator_set.contains(validator));
        let removed_count = cur_len - self.validator_stats.len();
        if removed_count > 0 {
            debug!("Removed {} stale validator data", removed_count);
        }
    }
}
