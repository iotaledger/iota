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

/// After this many latency observations the confidence penalty drops to zero.
const WARMUP_OBS: u64 = 10;

/// EWMA-based latency estimator.
///
/// Each new observation is weighted by α and the prior estimate by (1 − α).
/// At α = 0.5 an overload spike raises the score in one observation and
/// recovery takes ~3–4 observations.
#[derive(Debug, Clone)]
pub struct EwmaLatency {
    ewma_secs: f64,
    alpha: f64,
    /// Total number of observations recorded so far.
    pub count: u64,
}

impl EwmaLatency {
    pub fn new(first: Duration, alpha: f64) -> Self {
        Self {
            ewma_secs: first.as_secs_f64(),
            alpha,
            count: 1,
        }
    }

    pub fn add_value(&mut self, d: Duration) {
        self.ewma_secs = self.alpha * d.as_secs_f64() + (1.0 - self.alpha) * self.ewma_secs;
        self.count += 1;
    }

    pub fn get(&self) -> Duration {
        Duration::from_secs_f64(self.ewma_secs.max(0.0))
    }
}

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
    /// Moving window of success rate (0.0 to 1.0) for ALL operations.
    /// Used for backward-compatible score computation in tests that read
    /// this field directly.  Subject to circuit-breaker flushes.
    pub reliability: MovingWindow<f64>,

    /// Moving window of success rate for *real-transaction* operations only
    /// (Submit, Effects, Consensus — non-ping).  Health-check and probe
    /// successes do not dilute this signal.
    pub real_tx_reliability: MovingWindow<f64>,

    /// EWMA latency estimator per operation type.
    pub average_latencies: BTreeMap<OperationType, EwmaLatency>,

    /// Total real-transaction observations (non-ping Submit/Effects/Consensus).
    /// Used to compute the confidence/UCB penalty.
    pub real_tx_count: u64,

    /// Number of consecutive real-transaction failures since the last success.
    /// When this reaches `circuit_breaker_threshold` the reliability windows
    /// are flushed with zeros to immediately demote the validator.
    pub consecutive_failures: u32,

    // ---- private config snapshot ----
    reliability_window_size: usize,
    circuit_breaker_threshold: u32,
    latency_ewma_alpha: f64,
}

impl ValidatorClientStats {
    /// Construct with explicit parameters (used directly in some tests).
    /// `_latency_window_size` is accepted for API compatibility but ignored;
    /// the EWMA scorer does not use a fixed window size.
    pub fn new(
        init_reliability: f64,
        reliability_moving_window_size: usize,
        _latency_window_size: usize,
    ) -> Self {
        Self::with_config(init_reliability, reliability_moving_window_size, 0.5, 3)
    }

    /// Construct from explicit config values.
    pub fn with_config(
        init_reliability: f64,
        reliability_window_size: usize,
        latency_ewma_alpha: f64,
        circuit_breaker_threshold: u32,
    ) -> Self {
        Self {
            reliability: MovingWindow::new(init_reliability, reliability_window_size),
            real_tx_reliability: MovingWindow::new(init_reliability, reliability_window_size),
            average_latencies: BTreeMap::new(),
            real_tx_count: 0,
            consecutive_failures: 0,
            reliability_window_size,
            circuit_breaker_threshold,
            latency_ewma_alpha,
        }
    }

    pub fn update_average_latency(&mut self, operation: OperationType, new_latency: Duration) {
        let alpha = self.latency_ewma_alpha;
        match self.average_latencies.entry(operation) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().add_value(new_latency);
            }
            Entry::Vacant(entry) => {
                entry.insert(EwmaLatency::new(new_latency, alpha));
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
        let config = &self.config;
        let validator_stats = self
            .validator_stats
            .entry(feedback.authority_name)
            .or_insert_with(|| {
                ValidatorClientStats::with_config(
                    1.0,
                    config.reliability_moving_window_size,
                    config.latency_ewma_alpha,
                    config.circuit_breaker_threshold,
                )
            });

        // Real-tx ops: non-ping Submit / Effects / Consensus.
        // Health-check and ping probes do NOT update consecutive_failures or
        // real_tx_reliability so that a validator cannot mask real-tx failures
        // by responding to probes.
        let is_real_tx = !feedback.ping
            && matches!(
                feedback.operation,
                OperationType::Submit | OperationType::Effects | OperationType::Consensus
            );

        match feedback.result {
            Ok(latency) => {
                validator_stats.reliability.add_value(1.0);
                if is_real_tx {
                    validator_stats.consecutive_failures = 0;
                    validator_stats.real_tx_reliability.add_value(1.0);
                    validator_stats.real_tx_count += 1;
                }
                validator_stats.update_average_latency(feedback.operation, latency);
            }
            Err(()) => {
                validator_stats.reliability.add_value(0.0);
                if is_real_tx {
                    validator_stats.consecutive_failures += 1;
                    validator_stats.real_tx_reliability.add_value(0.0);

                    // Circuit-breaker: when consecutive failures first hit the
                    // threshold, flood both reliability windows with zeros so
                    // the validator is immediately demoted without waiting for
                    // the window to roll over organically.
                    if validator_stats.consecutive_failures
                        == validator_stats.circuit_breaker_threshold
                    {
                        let ws = validator_stats.reliability_window_size;
                        for _ in 0..ws {
                            validator_stats.reliability.add_value(0.0);
                            validator_stats.real_tx_reliability.add_value(0.0);
                        }
                    }
                }
            }
        }
    }

    /// Get adjusted latency scores for all validators in the committee.
    ///
    /// The score includes: EWMA base latency, real-tx reliability penalty,
    /// and a confidence/UCB penalty for validators with few observations.
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

    /// Calculate the full adjusted score for a validator (used for reporting
    /// and the Gap-2 confidence test).  Includes EWMA latency, real-tx
    /// reliability penalty, and confidence/UCB penalty.
    fn calculate_client_latency(&self, validator: &AuthorityName) -> Duration {
        let Some(stats) = self.validator_stats.get(validator) else {
            return MAX_LATENCY;
        };

        // Prefer Consensus latency; fall back to HealthCheck so validators
        // can be ranked before their first real transaction.
        let (base_latency, obs_count) =
            if let Some(ewma) = stats.average_latencies.get(&OperationType::Consensus) {
                (ewma.get(), ewma.count)
            } else if let Some(ewma) = stats.average_latencies.get(&OperationType::HealthCheck) {
                (ewma.get(), ewma.count)
            } else {
                return MAX_LATENCY;
            };

        let real_tx_reliability = stats.real_tx_reliability.get();
        let reliability_penalty =
            MAX_LATENCY.mul_f64((1.0 - real_tx_reliability) * self.config.reliability_weight);

        // Confidence / UCB penalty: extra cost for validators with few
        // observations.  Decays linearly to zero after WARMUP_OBS observations.
        let confidence_penalty = if obs_count >= WARMUP_OBS {
            Duration::ZERO
        } else {
            let frac = 1.0 - obs_count as f64 / WARMUP_OBS as f64;
            MAX_LATENCY.mul_f64(self.config.confidence_weight * frac)
        };

        (base_latency + reliability_penalty + confidence_penalty).min(MAX_LATENCY)
    }

    /// Calculate a simpler selection score (EWMA + reliability penalty only,
    /// no confidence penalty) for use in
    /// `select_shuffled_preferred_validators`.
    ///
    /// This keeps the selection ordering stable and proportional to actual
    /// observed latency once a validator has any data at all.
    pub fn calculate_selection_score(&self, validator: &AuthorityName) -> Duration {
        let Some(stats) = self.validator_stats.get(validator) else {
            return MAX_LATENCY;
        };

        let base_latency =
            if let Some(ewma) = stats.average_latencies.get(&OperationType::Consensus) {
                ewma.get()
            } else if let Some(ewma) = stats.average_latencies.get(&OperationType::HealthCheck) {
                ewma.get()
            } else {
                return MAX_LATENCY;
            };

        let real_tx_reliability = stats.real_tx_reliability.get();
        let reliability_penalty =
            MAX_LATENCY.mul_f64((1.0 - real_tx_reliability) * self.config.reliability_weight);

        (base_latency + reliability_penalty).min(MAX_LATENCY)
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
