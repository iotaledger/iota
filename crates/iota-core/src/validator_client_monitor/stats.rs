// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, HashSet, btree_map::Entry},
    time::Duration,
};

use iota_common::moving_window::MovingWindow;
use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{base_types::AuthorityName, committee::Committee};
use tracing::debug;

use crate::validator_client_monitor::{OperationFeedback, OperationType};

/// Maximum adjusted latency from completely unreachable (reliability = 0.0) or
/// very slow validators.
const MAX_LATENCY: Duration = Duration::from_secs(10);

/// Complete client-observed statistics for validator interactions.
#[derive(Debug, Clone)]
pub struct ClientObservedStats {
    /// Per-validator statistics mapping authority names to their
    /// client-observed metrics
    pub validator_stats: HashMap<AuthorityName, ValidatorClientStats>,
    /// Configuration parameters for scoring and exclusion policies
    pub config: ValidatorClientMonitorConfig,
}

/// Client-observed stats for a single validator.
#[derive(Debug, Clone)]
pub struct ValidatorClientStats {
    /// Moving window of success rate (0.0 to 1.0)
    pub reliability: MovingWindow<f64>,
    /// Moving window of latencies for each operation type (Submit, Effects,
    /// HealthCheck)
    pub average_latencies: BTreeMap<OperationType, MovingWindow<Duration>>,
    /// Size of the moving window for latency measurements
    pub latency_moving_window_size: usize,
}

impl ValidatorClientStats {
    pub fn new(
        init_reliability: f64,
        reliability_moving_window_size: usize,
        latency_moving_window_size: usize,
    ) -> Self {
        Self {
            reliability: MovingWindow::new(init_reliability, reliability_moving_window_size),
            average_latencies: BTreeMap::new(),
            latency_moving_window_size,
        }
    }

    pub fn update_average_latency(&mut self, operation: OperationType, new_latency: Duration) {
        match self.average_latencies.entry(operation) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().add_value(new_latency);
            }
            Entry::Vacant(entry) => {
                entry.insert(MovingWindow::new(
                    new_latency,
                    self.latency_moving_window_size,
                ));
            }
        }
    }
}

impl ClientObservedStats {
    pub fn new(config: ValidatorClientMonitorConfig) -> Self {
        Self {
            validator_stats: HashMap::new(),
            config,
        }
    }

    /// Record client-observed interaction result with a validator.
    pub fn record_interaction_result(&mut self, feedback: OperationFeedback) {
        let validator_stats = self
            .validator_stats
            .entry(feedback.authority_name)
            .or_insert_with(|| {
                ValidatorClientStats::new(
                    1.0,
                    self.config.reliability_moving_window_size,
                    self.config.latency_moving_window_size,
                )
            });

        match feedback.result {
            Ok(latency) => {
                validator_stats.reliability.add_value(1.0);
                validator_stats.update_average_latency(feedback.operation, latency);
            }
            Err(()) => {
                validator_stats.reliability.add_value(0.0);
            }
        }
    }

    /// Get validator latencies for all validators in the committee for the
    /// provided tx type.
    pub fn get_all_validator_stats(
        &self,
        committee: &Committee,
    ) -> HashMap<AuthorityName, Duration> {
        committee
            .names()
            .map(|validator| {
                let latency = self.calculate_client_latency(validator);
                (*validator, latency)
            })
            .collect()
    }

    /// Calculate adjusted latency for a single validator for the provided tx
    /// type.
    fn calculate_client_latency(&self, validator: &AuthorityName) -> Duration {
        let Some(stats) = self.validator_stats.get(validator) else {
            return MAX_LATENCY;
        };

        let operation = OperationType::Consensus;
        let Some(latency) = stats.average_latencies.get(&operation) else {
            return MAX_LATENCY;
        };

        let base_latency = latency.get();
        let reliability = stats.reliability.get();
        let reliability_weight = self.config.reliability_weight;
        let penalty = MAX_LATENCY.mul_f64((1.0 - reliability) * reliability_weight);
        (base_latency + penalty).min(MAX_LATENCY)
    }

    /// Retain only the specified validators, removing any others.
    pub fn retain_validators(&mut self, current_validators: &[AuthorityName]) {
        let cur_len = self.validator_stats.len();
        let validator_set: HashSet<_> = current_validators.iter().collect();
        self.validator_stats
            .retain(|validator, _| validator_set.contains(validator));
        let removed_count = cur_len - self.validator_stats.len();
        if removed_count > 0 {
            debug!("Removed {} stale validator data", removed_count);
        }
    }
}
