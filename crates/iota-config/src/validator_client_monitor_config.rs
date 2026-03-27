// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Configuration for the Validator Client Monitor
//!
//! The Validator Client Monitor tracks client-observed performance metrics for
//! validators in the IOTA network. It runs from the perspective of a fullnode
//! and monitors:
//! - Transaction submission latency
//! - Effects retrieval latency
//! - Health check response times
//! - Success/failure rates

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configuration for validator client monitoring from the client perspective
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ValidatorClientMonitorConfig {
    /// How often to perform health checks on validators.
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval: Duration,

    /// Timeout for health check requests.
    #[serde(default = "default_health_check_timeout")]
    pub health_check_timeout: Duration,

    /// History decay factor τ for EWMA latency estimation (0 < τ ≤ ∞).
    ///
    /// Lower τ means faster decaying and more weight on recent observations.
    #[serde(default = "default_latency_ewma_tau")]
    pub latency_ewma_tau: f64,

    #[serde(default = "default_latency_ewma_score_coeff")]
    pub latency_ewma_score_coeff: f64,

    #[serde(default = "default_empty_latency_score")]
    pub empty_latency_score: f64,

    #[serde(default = "default_risk_coeff")]
    pub risk_coeff: f64,

    #[serde(default = "default_stale_coeff")]
    pub stale_coeff: f64,

    #[serde(default = "default_failure_coeff")]
    pub failure_coeff: f64,

    #[serde(default = "default_exploration_coeff")]
    pub exploration_coeff: f64,

    #[serde(default = "default_no_validator_score")]
    pub no_validator_score: f64,

    /// Minimum number of validators that must appear in the shuffled
    /// preferred group returned by `select_shuffled_preferred_validators`,
    /// preventing a single validator from monopolising all traffic.
    #[serde(default = "default_min_preferred_group_size")]
    pub min_preferred_group_size: usize,

    #[serde(default = "default_max_preferred_group_size")]
    pub max_preferred_group_size: usize,

    #[serde(default = "default_preferred_group_delta")]
    pub preferred_group_delta: f64,

    /// Expected baseline latency for Submit operations (seconds).
    /// Used to normalise raw latency scores to a dimensionless ratio
    /// (actual / expected), making all operation types directly comparable.
    #[serde(default = "default_expected_latency_submit_secs")]
    pub expected_latency_submit_secs: f64,

    /// Expected baseline latency for Effects operations (seconds).
    #[serde(default = "default_expected_latency_effects_secs")]
    pub expected_latency_effects_secs: f64,

    /// Expected baseline latency for HealthCheck operations (seconds).
    #[serde(default = "default_expected_latency_healthcheck_secs")]
    pub expected_latency_healthcheck_secs: f64,

    /// Expected baseline latency for Consensus operations (seconds).
    #[serde(default = "default_expected_latency_consensus_secs")]
    pub expected_latency_consensus_secs: f64,

    /// Coefficient for the selective-failure penalty.
    ///
    /// A validator that passes HealthCheck reliably but fails work operations
    /// (Submit / Consensus / Effects) is likely misbehaving selectively.
    /// This coefficient scales the resulting additional penalty.
    #[serde(default = "default_selective_failure_coeff")]
    pub selective_failure_coeff: f64,

    /// Minimum failure-rate gap between work operations and HealthCheck before
    /// the selective-failure penalty applies.
    ///
    /// Acts as a noise floor: a gap smaller than this threshold is attributed
    /// to statistical variance and does not trigger the penalty.
    /// Example: 0.1 means work ops must fail at least 10 percentage points more
    /// often than HealthCheck before the inconsistency is flagged.
    #[serde(default = "default_selective_failure_noise_threshold")]
    pub selective_failure_noise_threshold: f64,

    /// Minimum HealthCheck effective sample size (n_eff) required before the
    /// selective-failure penalty is applied at full weight.
    ///
    /// The penalty is scaled by `min(1, n_eff_healthcheck / this_value)`,
    /// so it is fully suppressed below a small number of HealthCheck
    /// observations and ramps up linearly to full strength once this threshold
    /// is reached.
    #[serde(default = "default_selective_failure_min_n_eff")]
    pub selective_failure_min_n_eff: f64,

    /// Failure rate above which a validator is excluded from selection
    /// entirely (Phase 0).
    ///
    /// Exclusion is only applied once `exclusion_min_n_eff` HealthCheck
    /// samples have been collected, so a brief outage at startup does not
    /// permanently ban a validator.
    #[serde(default = "default_exclusion_failure_threshold")]
    pub exclusion_failure_threshold: f64,

    /// Minimum HealthCheck effective sample size required before the
    /// exclusion threshold is enforced.
    ///
    /// Guards against excluding a validator on the basis of a handful of
    /// failures that may not be representative.
    #[serde(default = "default_exclusion_min_n_eff")]
    pub exclusion_min_n_eff: f64,

    /// Number of exploration slots: how many validators are selected in
    /// Phase 2 (from the non-excluded, non-Phase-1 remainder) purely on the
    /// basis of their exploration bonus.
    #[serde(default = "default_max_exploration_group_size")]
    pub max_exploration_group_size: usize,

    /// Minimum exploration threshold: the minimum exploration bonus a validator
    /// must have to be considered for selection in Phase 2.
    #[serde(default = "default_min_exploration_threshold")]
    pub min_exploration_threshold: f64,
}

impl Default for ValidatorClientMonitorConfig {
    fn default() -> Self {
        Self {
            health_check_interval: default_health_check_interval(),
            health_check_timeout: default_health_check_timeout(),
            latency_ewma_tau: default_latency_ewma_tau(),
            latency_ewma_score_coeff: default_latency_ewma_score_coeff(),
            empty_latency_score: default_empty_latency_score(),
            risk_coeff: default_risk_coeff(),
            stale_coeff: default_stale_coeff(),
            failure_coeff: default_failure_coeff(),
            exploration_coeff: default_exploration_coeff(),
            no_validator_score: default_no_validator_score(),
            min_preferred_group_size: default_min_preferred_group_size(),
            max_preferred_group_size: default_max_preferred_group_size(),
            preferred_group_delta: default_preferred_group_delta(),
            expected_latency_submit_secs: default_expected_latency_submit_secs(),
            expected_latency_effects_secs: default_expected_latency_effects_secs(),
            expected_latency_healthcheck_secs: default_expected_latency_healthcheck_secs(),
            expected_latency_consensus_secs: default_expected_latency_consensus_secs(),
            selective_failure_coeff: default_selective_failure_coeff(),
            selective_failure_noise_threshold: default_selective_failure_noise_threshold(),
            selective_failure_min_n_eff: default_selective_failure_min_n_eff(),
            exclusion_failure_threshold: default_exclusion_failure_threshold(),
            exclusion_min_n_eff: default_exclusion_min_n_eff(),
            max_exploration_group_size: default_max_exploration_group_size(),
            min_exploration_threshold: default_min_exploration_threshold(),
        }
    }
}

fn default_health_check_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_health_check_timeout() -> Duration {
    Duration::from_secs(2)
}

fn default_latency_ewma_tau() -> f64 {
    10.0
}

fn default_latency_ewma_score_coeff() -> f64 {
    2.0
}

fn default_empty_latency_score() -> f64 {
    10.0
}

fn default_risk_coeff() -> f64 {
    50.0
}

fn default_stale_coeff() -> f64 {
    100.0
}

fn default_failure_coeff() -> f64 {
    500.0
}

fn default_exploration_coeff() -> f64 {
    20.0
}

fn default_no_validator_score() -> f64 {
    100.0
}

fn default_min_preferred_group_size() -> usize {
    2
}

fn default_max_preferred_group_size() -> usize {
    10
}

fn default_preferred_group_delta() -> f64 {
    0.02
}

fn default_expected_latency_submit_secs() -> f64 {
    0.15
}

fn default_expected_latency_effects_secs() -> f64 {
    1.5
}

fn default_expected_latency_healthcheck_secs() -> f64 {
    0.1
}

fn default_expected_latency_consensus_secs() -> f64 {
    0.8
}

fn default_selective_failure_coeff() -> f64 {
    500.0
}

fn default_selective_failure_noise_threshold() -> f64 {
    0.1
}

fn default_selective_failure_min_n_eff() -> f64 {
    5.0
}

fn default_exclusion_failure_threshold() -> f64 {
    0.7
}

fn default_exclusion_min_n_eff() -> f64 {
    5.0
}

fn default_max_exploration_group_size() -> usize {
    1
}

fn default_min_exploration_threshold() -> f64 {
    20.0
}
