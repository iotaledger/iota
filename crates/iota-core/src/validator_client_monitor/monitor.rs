// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{Arc, Weak},
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{
    base_types::AuthorityName, committee::Committee, messages_grpc::ValidatorHealthRequest,
};
use parking_lot::RwLock;
use tokio::{
    task::JoinSet,
    time::{interval, timeout},
};
use tracing::{info, warn};

use crate::{
    authority_aggregator::AuthorityAggregator,
    authority_client::AuthorityAPI,
    validator_client_monitor::{
        OperationFeedback, OperationType, metrics::ValidatorClientMetrics,
        stats::ClientObservedStats,
    },
};

/// Monitors validator interactions from the client's perspective.
pub struct ValidatorClientMonitor<A> {
    config: ValidatorClientMonitorConfig,
    metrics: Arc<ValidatorClientMetrics>,
    client_stats: RwLock<ClientObservedStats>,
    authority_aggregator: Arc<ArcSwap<AuthorityAggregator<A>>>,
}

impl<A> ValidatorClientMonitor<A>
where
    A: AuthorityAPI + Send + Sync + 'static,
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
        });

        let period = monitor.config.health_check_interval;
        let monitor_weak = Arc::downgrade(&monitor);
        tokio::spawn(async move {
            Self::run_health_checks(monitor_weak, period).await;
        });

        monitor
    }

    #[cfg(test)]
    pub fn new_for_test(authority_aggregator: Arc<AuthorityAggregator<A>>) -> Arc<Self> {
        // Use a fresh isolated registry per test instance to prevent parallel
        // tests from conflicting when registering metrics with the same names
        // into the global default registry.
        Self::new(
            ValidatorClientMonitorConfig::default(),
            Arc::new(ValidatorClientMetrics::new(&prometheus::Registry::new())),
            Arc::new(ArcSwap::new(authority_aggregator)),
        )
    }

    /// Background task that runs periodic health checks on all validators.
    fn spawn_health_checks_tasks(self: Arc<Self>) -> JoinSet<()> {
        let authority_agg = self.authority_aggregator.load();

        let current_validators = authority_agg.committee.names();
        self.client_stats
            .write()
            .retain_validators(current_validators);

        let mut tasks = JoinSet::new();

        for (name, safe_client) in authority_agg.authority_clients.iter() {
            let name = *name;
            let display_name = authority_agg.get_display_name(&name);
            let client = safe_client.clone();
            let timeout_duration = self.config.health_check_timeout;
            let monitor = self.clone();

            tasks.spawn(async move {
                let feedback_builder =
                    OperationFeedback::builder(name, display_name, OperationType::HealthCheck);
                let start = Instant::now();
                let result = match timeout(
                    timeout_duration,
                    client.validator_health(ValidatorHealthRequest {}),
                )
                .await
                {
                    Ok(Ok(_response)) => Ok(start.elapsed()),
                    Ok(Err(_)) | Err(_) => Err(()),
                };
                monitor.record_interaction_result(feedback_builder.result_now(result));
            });
        }

        tasks
    }

    async fn run_health_checks(monitor: Weak<Self>, period: Duration) {
        let mut interval = interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            let Some(monitor) = monitor.upgrade() else {
                break;
            };
            let mut tasks = monitor.spawn_health_checks_tasks();
            while let Some(result) = tasks.join_next().await {
                if let Err(e) = result {
                    warn!("Health check task failed: {}", e);
                }
            }
        }
    }
}

impl<A> ValidatorClientMonitor<A> {
    /// Record client-observed interaction result with a validator.
    pub fn record_interaction_result(&self, feedback: OperationFeedback) {
        let score = self
            .client_stats
            .write()
            .record_interaction_result(&feedback);
        self.metrics.record_interaction_result(&feedback, score);
    }

    /// Select validators based on client-observed performance for the given
    /// transaction type.
    ///
    /// Scores are computed live from the current `client_stats` so that
    /// recently recorded failures or latency spikes are reflected immediately
    /// without waiting for the next health-check cache refresh.
    ///
    /// The preferred prefix (validators within `delta` of the best score) is
    /// shuffled to spread traffic; the rest are returned in score order.
    /// The prefix is guaranteed to contain at least `min_preferred_group_size`
    /// validators to prevent a single validator from monopolising all traffic.
    pub fn select_shuffled_preferred_validators(
        &self,
        committee: &Committee,
    ) -> Vec<AuthorityName> {
        let rng = rand::thread_rng();
        let now = Instant::now();
        let validators = self
            .client_stats
            .read()
            .select_shuffled_preferred_validators(committee.names(), now, rng);
        self.metrics
            .shuffled_validators
            .observe(validators.len() as f64);
        validators
    }

    #[cfg(test)]
    pub fn get_client_stats_len(&self) -> usize {
        self.client_stats.read().num_validators()
    }

    #[cfg(test)]
    pub fn has_validator_stats(&self, validator: &AuthorityName) -> bool {
        self.client_stats.read().has_validator(validator)
    }
}
