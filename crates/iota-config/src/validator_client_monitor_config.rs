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

    /// Weight for reliability when computing validator scores.
    ///
    /// Controls importance of reliability when adjusting the validator's
    /// latency for transaction submission selection. The higher the weight,
    /// the more penalty is given to unreliable validators. Default to 2.0.
    /// Value should be positive.
    #[serde(default = "default_reliability_weight")]
    pub reliability_weight: f64,

    /// Smoothing factor α for EWMA latency estimation (0 < α ≤ 1).
    ///
    /// Higher α means more weight on recent observations. At α = 0.5
    /// a new observation has 50 % weight, so overload spikes are detected
    /// in 1–2 observations and recovery takes 3–4 observations.
    #[serde(default = "default_latency_ewma_alpha")]
    pub latency_ewma_alpha: f64,

    /// Weight for the confidence / UCB penalty applied to low-observation validators.
    ///
    /// Validators with fewer than `WARMUP_OBS` (= 10) real-transaction
    /// observations receive an additional score penalty proportional to
    /// this weight, discouraging the scheduler from over-trusting untested
    /// validators. Set to 0.0 to disable.
    #[serde(default = "default_confidence_weight")]
    pub confidence_weight: f64,

    /// Number of consecutive real-transaction failures that trip the
    /// circuit-breaker, flushing the reliability window with zeros so
    /// the validator is immediately demoted.
    #[serde(default = "default_circuit_breaker_threshold")]
    pub circuit_breaker_threshold: u32,

    /// Minimum number of validators that must appear in the shuffled
    /// preferred group returned by `select_shuffled_preferred_validators`,
    /// preventing a single validator from monopolising all traffic.
    #[serde(default = "default_min_preferred_group_size")]
    pub min_preferred_group_size: usize,

    /// Size of the moving window for latency measurements
    ///
    /// Deprecated: kept for config-file backwards compatibility.
    /// The EWMA scorer ignores window size; use `latency_ewma_alpha` instead.
    #[serde(default = "default_latency_moving_window_size")]
    pub latency_moving_window_size: usize,

    /// Size of the moving window for reliability measurements
    #[serde(default = "default_reliability_moving_window_size")]
    pub reliability_moving_window_size: usize,
}

impl Default for ValidatorClientMonitorConfig {
    fn default() -> Self {
        Self {
            health_check_interval: default_health_check_interval(),
            health_check_timeout: default_health_check_timeout(),
            reliability_weight: default_reliability_weight(),
            latency_ewma_alpha: default_latency_ewma_alpha(),
            confidence_weight: default_confidence_weight(),
            circuit_breaker_threshold: default_circuit_breaker_threshold(),
            min_preferred_group_size: default_min_preferred_group_size(),
            latency_moving_window_size: default_latency_moving_window_size(),
            reliability_moving_window_size: default_reliability_moving_window_size(),
        }
    }
}

fn default_health_check_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_health_check_timeout() -> Duration {
    Duration::from_secs(2)
}

fn default_reliability_weight() -> f64 {
    2.0
}

fn default_latency_ewma_alpha() -> f64 {
    0.5
}

fn default_confidence_weight() -> f64 {
    0.3
}

fn default_circuit_breaker_threshold() -> u32 {
    3
}

fn default_min_preferred_group_size() -> usize {
    2
}

fn default_latency_moving_window_size() -> usize {
    40
}

fn default_reliability_moving_window_size() -> usize {
    20
}
