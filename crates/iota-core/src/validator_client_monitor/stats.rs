// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::base_types::AuthorityName;
use rand::seq::SliceRandom;
use tokio::time::Instant;
use tracing::debug;

use crate::validator_client_monitor::{OperationFeedback, OperationType};

/// Ok(latency in sec) or Err(high failure latency in sec)
type Observation = Result<f64, f64>;

/// Exponential weighted moving average (EWMA) estimator.
///
/// Each new observation is weighted by α and the prior estimate by (1 − α):
/// μ_t ​= α x_t​ + (1−α) μ_{t−1}​
/// σ_t^2​ = (1−α) σ_{t−1}^2 ​+ α (x_t​−μ_{t−1}​)^2
#[derive(Clone, Copy, Debug)]
struct Ewma {
    /// Current mean estimate: μ_t.
    mean: f64,
    /// Current variance estimate: σ_t^2.
    variance: f64,
    /// Failure mean estimate.
    failure: f64,
    /// Decayed/effective sample size.
    sample_size: f64,
}

impl Ewma {
    fn first_value(observation: Observation) -> Self {
        Self {
            mean: observation.unwrap_or_else(|failure| failure),
            variance: 0.0,
            failure: observation.map_or(1.0, |_| 0.0),
            sample_size: 1.0,
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
        let ad = alpha * delta;
        self.mean += ad;
        // σ_t^2​ = (1−α) σ_{t−1}^2 ​+ α (x_t​−μ_{t−1}​)^2
        self.variance = a1 * self.variance + ad * delta;
        // failures EWMA
        self.failure = a1 * self.failure + observation.map_or(alpha, |_| 0.0);
        // effective sample size is just EWMA of the observation indicators
        // w_t = (1 - α) w_{t-1} + α
        self.sample_size = a1 * self.sample_size + alpha;
    }

    fn score(&self, k: f64) -> f64 {
        // Score = mean + k * stddev. Higher k means more penalty for
        // variability and less confidence in the estimate.
        self.mean + k * self.stddev()
    }

    fn get_stats(&self, k: f64) -> (f64, f64, f64) {
        (self.score(k), self.sample_size, self.failure)
    }
}

/// Time-decayed EWMA-based estimator.
///
/// This is a EWMA estimator with a variable weight α_t (interval between
/// updates): α_t = 1 - exp(-Δt / τ)
#[derive(Clone, Copy, Debug)]
struct TimeDecayEwma {
    /// Base EWMA estimator.
    ewma: Ewma,
    /// Timestamp of the last update: t.
    last_update: Instant,
    /// EWMA of normalized observations interval (Δt/τ).
    /// None until the second observation.
    interval_ewma: Option<f64>,
}

impl TimeDecayEwma {
    fn first_value(observation: Observation, now: Instant) -> Self {
        Self {
            ewma: Ewma::first_value(observation),
            last_update: now,
            interval_ewma: None,
        }
    }

    fn interval_alpha(&self, tau: f64, now: Instant) -> (f64, f64) {
        debug_assert!(now >= self.last_update, "Timestamps must be non-decreasing");
        // α_t = 1 - exp(-Δt / τ)
        // avoid zero Δt, otherwise (α_t = 0) observation won't be updated
        let dt = now.duration_since(self.last_update).as_secs_f64().max(1e-9);
        let interval = dt / tau;
        (interval, 1.0 - (-interval).exp())
    }

    fn update_with_time_decay(&mut self, tau: f64, now: Instant, observation: Observation) {
        let (interval, alpha) = self.interval_alpha(tau, now);
        // Update the value EWMA with time-derived alpha.
        self.ewma.update(observation, alpha);
        self.last_update = now;
        // Update the interval EWMA with a fixed weight of 0.1.
        const ALPHA: f64 = 0.1;
        self.interval_ewma = Some(
            self.interval_ewma
                .map_or(interval, |prev| (1.0 - ALPHA) * prev + ALPHA * interval),
        );
    }

    fn get_stats(&self, k: f64, tau: f64, now: Instant) -> (f64, f64, f64, f64, Option<f64>) {
        let (score, sample_size, failure) = self.ewma.get_stats(k);
        (
            score,
            sample_size,
            failure,
            self.interval_alpha(tau, now).1,
            self.interval_ewma,
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

    fn update(&mut self, tau: f64, timestamp: Instant, observation: Observation) {
        if let Some(inner) = &mut self.inner {
            inner.update_with_time_decay(tau, timestamp, observation);
        } else {
            self.inner = Some(TimeDecayEwma::first_value(observation, timestamp));
        }
    }

    fn get_stats(
        &self,
        k: f64,
        tau: f64,
        now: Instant,
    ) -> Option<(f64, f64, f64, f64, Option<f64>)> {
        self.inner
            .map(|time_decay_ewma| time_decay_ewma.get_stats(k, tau, now))
    }
}

/// Latency estimator with logarithmic observations.
///
/// Logarithmic observations help smooth out high variability in latency
/// measurements. For regular observations LatencyEwma can be used.
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

    /// Update the estimator with a new observation.
    ///
    /// Tau argument should match the expected interval between observations.
    /// A failed observation is still used to update the log latency average.
    /// Thus the failure value is treated as a large latency value (eg.
    /// operation timeout).
    fn update(&mut self, tau: f64, timestamp: Instant, observation: Observation) {
        // Avoid ln(0) by capping at a small positive value.
        let log_observation = observation
            .map(|x| x.max(1e-9).ln())
            .map_err(|x| x.max(1e-9).ln());
        self.inner.update(tau, timestamp, log_observation);
    }

    fn get_stats(
        &self,
        k: f64,
        tau: f64,
        now: Instant,
    ) -> Option<(f64, f64, f64, f64, Option<f64>)> {
        self.inner.get_stats(k, tau, now).map(
            |(score, sample_size, failure, alpha, interval_ewma)| {
                (score.exp(), sample_size, failure, alpha, interval_ewma)
            },
        )
    }
}

/// Complete client-observed statistics for validator interactions.
#[derive(Debug, Clone)]
pub(super) struct ClientObservedStats {
    /// Per-validator statistics mapping authority names to their
    /// client-observed stats
    validator_stats: HashMap<AuthorityName, ValidatorClientStats>,
    /// Configuration parameters for scoring and exclusion policies
    config: ValidatorClientMonitorConfig,
}

/// Client-observed stats for a single validator.
#[derive(Debug, Clone)]
struct ValidatorClientStats {
    /// Health-check operation stats.
    stats_health_check: LogLatencyEwma,
    /// Submit operation stats.
    stats_submit: LogLatencyEwma,
    /// Effects operation stats.
    stats_effects: LogLatencyEwma,
    /// Consensus operation stats.
    stats_consensus: LogLatencyEwma,
}

/// The reference interval for Submit, Effects, and Consensus operations.
const TAU: f64 = 60.0;

/// Failure penalty (max latency or timeout in seconds) for Submit operation.
const FAILURE_SUBMIT: f64 = 5.0;

/// Failure penalty (max latency or timeout in seconds) for Effects operation.
const FAILURE_EFFECTS: f64 = 10.0;

/// Failure penalty (max latency or timeout in seconds) for Consensus operation.
const FAILURE_CONSENSUS: f64 = FAILURE_SUBMIT + FAILURE_EFFECTS;

/// Default value for maximum weighted interval (in seconds) in case there is not enough observations.
/// No observations in practice is equivalent to the last observation being very old.
const MAX_INTERVAL: f64 = 3600.0;

/// Penalty coefficient k used in latency score calculation: latency_score = mean + k * stddev.
/// Higher k means more penalty for variability and less confidence in the estimate.
/// Health-check operation is the most stable.
const VARIANCE_PENALTY_HEALTH_CHECK: f64 = 0.5;

/// Penalty coefficient k used in latency score calculation: latency_score = mean + k * stddev.
/// Higher k means more penalty for variability and less confidence in the estimate.
/// Submit operation is less stable.
const VARIANCE_PENALTY_SUBMIT: f64 = 2.0;

/// Penalty coefficient k used in latency score calculation: latency_score = mean + k * stddev.
/// Higher k means more penalty for variability and less confidence in the estimate.
/// Effects operation is less stable.
const VARIANCE_PENALTY_EFFECTS: f64 = 3.0;

/// Penalty coefficient k used in latency score calculation: latency_score = mean + k * stddev.
/// Higher k means more penalty for variability and less confidence in the estimate.
/// Consensus operation is the least stable.
const VARIANCE_PENALTY_CONSENSUS: f64 = 4.0;

/// Default latency score (in seconds) for missing observations.
/// Health-check operation is expected to have the lowest latency.
const EMPTY_SCORE_HEALTH_CHECK: f64 = 0.5;

/// Default latency score (in seconds) for missing observations.
/// Submit operation is expected to have lower latency.
const EMPTY_SCORE_SUBMIT: f64 = 2.0;

/// Default latency score (in seconds) for missing observations.
/// Effects operation is expected to have higher latency.
const EMPTY_SCORE_EFFECTS: f64 = 3.0;

/// Default latency score (in seconds) for missing observations.
/// Consensus operation is expected to have the highest latency.
const EMPTY_SCORE_CONSENSUS: f64 = 4.0;

/// Penalty score (in seconds) for failed operations.
const FAILURE_SCORE: f64 = 10.0;

impl ValidatorClientStats {
    fn new() -> Self {
        Self {
            stats_health_check: LogLatencyEwma::new(),
            stats_submit: LogLatencyEwma::new(),
            stats_effects: LogLatencyEwma::new(),
            stats_consensus: LogLatencyEwma::new(),
        }
    }

    fn record_interaction_result(
        &mut self,
        config: &ValidatorClientMonitorConfig,
        feedback: &OperationFeedback,
    ) {

        let observation = feedback.result.map(|latency| latency.as_secs_f64());
        match feedback.operation {
            OperationType::HealthCheck => {
                let tau_hc = config.health_check_interval.as_secs_f64();
                let failure_hc = config.health_check_timeout.as_secs_f64() * 4.0;
                self.stats_health_check.update(
                    tau_hc,
                    feedback.timestamp,
                    observation.map_err(|()| failure_hc),
                );
            }
            OperationType::Submit => {
                self.stats_submit.update(
                    TAU,
                    feedback.timestamp,
                    observation.map_err(|()| FAILURE_SUBMIT),
                );
            }
            OperationType::Effects => {
                self.stats_effects.update(
                    TAU,
                    feedback.timestamp,
                    observation.map_err(|()| FAILURE_EFFECTS),
                );
            }
            OperationType::Consensus => {
                self.stats_consensus.update(
                    TAU,
                    feedback.timestamp,
                    observation.map_err(|()| FAILURE_CONSENSUS),
                );
            }
        }
    }

    fn performance_score(&self, config: &ValidatorClientMonitorConfig, now: Instant) -> (f64, f64) {
        let tau_hc = config.health_check_interval.as_secs_f64();

        let (score_hc, _sample_size_hc, failures_hc, _alpha_hc, _interval_hc) = self
            .stats_health_check
            .get_stats(VARIANCE_PENALTY_HEALTH_CHECK, tau_hc, now)
            .unwrap_or((EMPTY_SCORE_HEALTH_CHECK, 0.0, 0.0, 1.0, None));
        let (score_sub, sample_size_sub, failures_sub, alpha_sub, interval_sub) = self
            .stats_submit
            .get_stats(VARIANCE_PENALTY_SUBMIT, TAU, now)
            .unwrap_or((EMPTY_SCORE_SUBMIT, 0.0, 0.0, 1.0, None));
        let (score_eff, sample_size_eff, failures_eff, alpha_eff, interval_eff) = self
            .stats_effects
            .get_stats(VARIANCE_PENALTY_EFFECTS, TAU, now)
            .unwrap_or((EMPTY_SCORE_EFFECTS, 0.0, 0.0, 1.0, None));
        let (score_con, sample_size_con, failures_con, alpha_con, interval_con) = self
            .stats_consensus
            .get_stats(VARIANCE_PENALTY_CONSENSUS, TAU, now)
            .unwrap_or((EMPTY_SCORE_CONSENSUS, 0.0, 0.0, 1.0, None));

        let score = score_hc + score_sub + score_eff + score_con;
        let sample_size = sample_size_sub.max(sample_size_eff).max(sample_size_con);
        let failures = failures_hc + failures_sub + failures_eff + failures_con;
        let recency = alpha_sub.max(alpha_eff).max(alpha_con);
        // max weighted interval over Submit, Effects, and Consensus operations
        let interval = interval_sub
            .unwrap_or(MAX_INTERVAL)
            .max(interval_eff.unwrap_or(MAX_INTERVAL))
            .max(interval_con.unwrap_or(MAX_INTERVAL));

        // good performance means:
        // - small score (latency)
        // - large sample size
        // - small failures
        // - small recency (of the latest observation) (or close to 1-1/e if
        //   observations interval is close to tau)
        // - small interval (frequent observations)
        // exploitation score is constructed such that lower value means better
        // performance
        let exploitation = (score + failures * FAILURE_SCORE)
            * (1.0 + recency)
            * ((interval + 1e-2) / (sample_size + 1e-2)).sqrt();

        // new validator or stale stats means:
        // - any score (latency)
        // - small sample size
        // - small failures
        // - recency close to 1
        // - large interval
        // exploration score is constructed such that lower value means less info about
        // the validator
        let exploration =
            failures * (1.0 - recency) * ((sample_size + 1e-2) / (interval + 1e-2)).sqrt();

        (exploitation, exploration)
    }
}

impl ClientObservedStats {
    pub(super) fn new(config: ValidatorClientMonitorConfig) -> Self {
        Self {
            validator_stats: HashMap::new(),
            config,
        }
    }

    /// Record client-observed interaction result with a validator.
    pub(super) fn record_interaction_result(&mut self, feedback: &OperationFeedback) -> (f64, f64) {
        let validator_stats = self
            .validator_stats
            .entry(feedback.authority_name)
            .or_insert_with(ValidatorClientStats::new);
        validator_stats.record_interaction_result(&self.config, feedback);
        validator_stats.performance_score(&self.config, feedback.timestamp)
    }

    pub(super) fn select_shuffled_preferred_validators<'a>(
        &self,
        committee: impl Iterator<Item = &'a AuthorityName>,
        now: Instant,
        mut rng: impl rand::Rng,
    ) -> Vec<&'a AuthorityName> {
        const UNKNOWN_VALIDATOR_SCORE: (f64, f64) = (1e6, 0.0);
        // rate committee validators
        let mut scored_committee: Vec<_> = committee
            .map(|v| {
                (
                    v,
                    self.validator_stats
                        .get(v)
                        .map(|stats| stats.performance_score(&self.config, now))
                        .unwrap_or(UNKNOWN_VALIDATOR_SCORE),
                )
            })
            .collect();
        // order by exploitation score, best performing first
        scored_committee.sort_by(|(_, (e1, _)), (_, (e2, _))| {
            e1.partial_cmp(e2).unwrap_or(std::cmp::Ordering::Equal)
        });
        // select exploitation group
        let exploitation_group_size =
            (self.config.exploitation_group_share.min(100) * scored_committee.len()).div_ceil(100);
        // order the rest by exploration score, new validators first followed by most
        // outdated ones
        scored_committee[exploitation_group_size..].sort_by(|(_, (_, e1)), (_, (_, e2))| {
            e1.partial_cmp(e2).unwrap_or(std::cmp::Ordering::Equal)
        });
        // select exploration group
        let exploration_group_size = (self.config.exploration_group_share.min(100)
            * scored_committee.len())
        .div_ceil(100)
        .min(scored_committee.len() - exploitation_group_size);
        // shuffle the two groups together to avoid overloading the best performing
        // validators and guarantee uniform exploration
        scored_committee[..exploitation_group_size + exploration_group_size].shuffle(&mut rng);
        // order the rest by combined score
        scored_committee[exploitation_group_size + exploration_group_size..].sort_by(
            |(_, e1), (_, e2)| {
                (e1.0 + e1.1)
                    .partial_cmp(&(e2.0 + e2.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            },
        );

        scored_committee.into_iter().map(|(v, _)| v).collect()
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

    #[cfg(test)]
    pub(super) fn has_validator(&self, validator: &AuthorityName) -> bool {
        self.validator_stats.contains_key(validator)
    }

    #[cfg(test)]
    pub(super) fn num_validators(&self) -> usize {
        self.validator_stats.len()
    }
}
