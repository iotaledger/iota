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

    /// Size of the moving window for latency measurements
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

fn default_latency_moving_window_size() -> usize {
    40
}

fn default_reliability_moving_window_size() -> usize {
    20
}
