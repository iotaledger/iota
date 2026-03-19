// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{
    base_types::AuthorityName, committee::Committee, messages_grpc::ValidatorHealthRequest,
};
use parking_lot::RwLock;
use rand::seq::SliceRandom;
use tokio::{
    task::JoinSet,
    time::{interval, timeout},
};
use tracing::{debug, info, warn};

use crate::{
    authority_aggregator::AuthorityAggregator,
    authority_client::AuthorityAPI,
    validator_client_monitor::{
        OperationFeedback, OperationType, metrics::ValidatorClientMetrics,
        stats::ClientObservedStats,
    },
};

/// Monitors validator interactions from the client's perspective.
pub struct ValidatorClientMonitor<A: Clone> {
    config: ValidatorClientMonitorConfig,
    metrics: Arc<ValidatorClientMetrics>,
    client_stats: RwLock<ClientObservedStats>,
    authority_aggregator: Arc<ArcSwap<AuthorityAggregator<A>>>,
    cached_latencies: RwLock<HashMap<AuthorityName, Duration>>,
}

impl<A> ValidatorClientMonitor<A>
where
    A: AuthorityAPI + Send + Sync + 'static + Clone,
{
    pub fn new(
        config: ValidatorClientMonitorConfig,
        metrics: Arc<ValidatorClientMetrics>,
        authority_aggregator: Arc<ArcSwap<AuthorityAggregator<A>>>,
    ) -> Arc<Self> {
        info!(
            "Validator client monitor starting with config: {:?}",
            config
        );

        let monitor = Arc::new(Self {
            config: config.clone(),
            metrics,
            client_stats: RwLock::new(ClientObservedStats::new(config)),
            authority_aggregator,
            cached_latencies: RwLock::new(HashMap::new()),
        });

        let monitor_clone = monitor.clone();
        tokio::spawn(async move {
            monitor_clone.run_health_checks().await;
        });

        monitor
    }

    #[cfg(test)]
    pub fn new_for_test(authority_aggregator: Arc<AuthorityAggregator<A>>) -> Arc<Self> {
        use prometheus::Registry;

        Self::new(
            ValidatorClientMonitorConfig::default(),
            Arc::new(ValidatorClientMetrics::new(&Registry::default())),
            Arc::new(ArcSwap::new(authority_aggregator)),
        )
    }

    /// Background task that runs periodic health checks on all validators.
    async fn run_health_checks(self: Arc<Self>) {
        let mut interval = interval(self.config.health_check_interval);

        loop {
            interval.tick().await;

            let authority_agg = self.authority_aggregator.load();

            let current_validators: Vec<_> = authority_agg.committee.names().cloned().collect();
            self.client_stats
                .write()
                .retain_validators(&current_validators);

            let mut tasks = JoinSet::new();

            for (name, safe_client) in authority_agg.authority_clients.iter() {
                let name = *name;
                let display_name = authority_agg.get_display_name(&name);
                let client = safe_client.clone();
                let timeout_duration = self.config.health_check_timeout;
                let monitor = self.clone();

                tasks.spawn(async move {
                    let start = Instant::now();
                    match timeout(
                        timeout_duration,
                        client.validator_health(ValidatorHealthRequest {}),
                    )
                    .await
                    {
                        Ok(Ok(_response)) => {
                            let latency = start.elapsed();
                            monitor.record_interaction_result(OperationFeedback {
                                authority_name: name,
                                display_name: display_name.clone(),
                                operation: OperationType::HealthCheck,
                                ping: false,
                                result: Ok(latency),
                            });
                        }
                        Ok(Err(_)) => {
                            monitor.record_interaction_result(OperationFeedback {
                                authority_name: name,
                                display_name: display_name.clone(),
                                operation: OperationType::HealthCheck,
                                ping: false,
                                result: Err(()),
                            });
                        }
                        Err(_) => {
                            monitor.record_interaction_result(OperationFeedback {
                                authority_name: name,
                                display_name,
                                operation: OperationType::HealthCheck,
                                ping: false,
                                result: Err(()),
                            });
                        }
                    }
                });
            }

            while let Some(result) = tasks.join_next().await {
                if let Err(e) = result {
                    warn!("Health check task failed: {}", e);
                }
            }

            self.update_cached_latencies(&authority_agg);
        }
    }
}

impl<A: Clone> ValidatorClientMonitor<A> {
    /// Calculate and cache latencies for all validators.
    fn update_cached_latencies(&self, authority_agg: &AuthorityAggregator<A>) {
        let committee = &authority_agg.committee;
        let mut cached_latencies = self.cached_latencies.write();

        let latencies_map = self.client_stats.read().get_all_validator_stats(committee);

        for (validator, latency) in latencies_map.iter() {
            debug!("Validator {}: latency {}", validator, latency.as_secs_f64());
            let display_name = authority_agg.get_display_name(validator);
            self.metrics
                .performance
                .with_label_values(&[&display_name])
                .set(latency.as_secs_f64());
        }

        *cached_latencies = latencies_map;
    }

    /// Record client-observed interaction result with a validator.
    pub fn record_interaction_result(&self, feedback: OperationFeedback) {
        let operation_str = match feedback.operation {
            OperationType::Submit => "submit",
            OperationType::Effects => "effects",
            OperationType::HealthCheck => "health_check",
            OperationType::Consensus => "consensus",
        };
        let ping_label = feedback.ping.to_string();
        let operation_str = operation_str.to_string();
        match feedback.result {
            Ok(latency) => {
                self.metrics
                    .observed_latency
                    .with_label_values(&[&feedback.display_name, &operation_str, &ping_label])
                    .observe(latency.as_secs_f64());
                self.metrics
                    .operation_success
                    .with_label_values(&[&feedback.display_name, &operation_str, &ping_label])
                    .inc();
            }
            Err(()) => {
                self.metrics
                    .operation_failure
                    .with_label_values(&[&feedback.display_name, &operation_str, &ping_label])
                    .inc();
            }
        }

        let mut client_stats = self.client_stats.write();
        client_stats.record_interaction_result(feedback);
    }

    /// Select validators based on client-observed performance for the given
    /// transaction type.
    pub fn select_shuffled_preferred_validators(
        &self,
        committee: &Committee,
        delta: f64,
    ) -> Vec<AuthorityName> {
        let mut rng = rand::thread_rng();

        let cached_latencies = self.cached_latencies.read();

        let mut validator_with_latencies: Vec<_> = committee
            .names()
            .map(|v| {
                (
                    *v,
                    cached_latencies.get(v).cloned().unwrap_or(Duration::ZERO),
                )
            })
            .collect();
        if validator_with_latencies.is_empty() {
            return vec![];
        }
        validator_with_latencies.sort_by_key(|(_, latency)| *latency);

        let lowest_latency = validator_with_latencies[0].1;
        let threshold = lowest_latency.mul_f64(1.0 + delta);
        let k = validator_with_latencies
            .iter()
            .enumerate()
            .find(|(_, (_, latency))| *latency > threshold)
            .map(|(i, _)| i)
            .unwrap_or(validator_with_latencies.len());
        validator_with_latencies[..k].shuffle(&mut rng);
        self.metrics.shuffled_validators.observe(k as f64);

        validator_with_latencies
            .into_iter()
            .map(|(v, _)| v)
            .collect()
    }

    #[cfg(test)]
    pub fn force_update_cached_latencies(&self, authority_agg: &AuthorityAggregator<A>) {
        self.update_cached_latencies(authority_agg);
    }

    #[cfg(test)]
    pub fn get_client_stats_len(&self) -> usize {
        self.client_stats.read().validator_stats.len()
    }

    #[cfg(test)]
    pub fn has_validator_stats(&self, validator: &AuthorityName) -> bool {
        self.client_stats
            .read()
            .validator_stats
            .contains_key(validator)
    }
}
