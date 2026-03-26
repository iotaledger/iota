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

    #[serde(default = "default_preferred_group_delta")]
    pub preferred_group_delta: f64,
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
            preferred_group_delta: default_preferred_group_delta(),
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

fn default_preferred_group_delta() -> f64 {
    0.02
}
