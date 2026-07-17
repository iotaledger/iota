// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
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

    /// The share (percentage) of committee validators with good performance to
    /// select.
    #[serde(default = "default_exploitation_group_share")]
    pub exploitation_group_share: usize,

    /// The share (percentage) of unknown committee validators or validators
    /// with outdated/stale stats to select.
    #[serde(default = "default_exploration_group_share")]
    pub exploration_group_share: usize,
}

impl Default for ValidatorClientMonitorConfig {
    fn default() -> Self {
        Self {
            health_check_interval: default_health_check_interval(),
            health_check_timeout: default_health_check_timeout(),
            exploitation_group_share: default_exploitation_group_share(),
            exploration_group_share: default_exploration_group_share(),
        }
    }
}

fn default_health_check_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_health_check_timeout() -> Duration {
    Duration::from_secs(2)
}

fn default_exploitation_group_share() -> usize {
    10
}

fn default_exploration_group_share() -> usize {
    10
}
