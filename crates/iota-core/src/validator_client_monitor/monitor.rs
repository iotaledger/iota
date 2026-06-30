// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{Arc, Weak},
    time::Duration,
};

use arc_swap::ArcSwap;
use iota_config::validator_client_monitor_config::ValidatorClientMonitorConfig;
use iota_types::{base_types::AuthorityName, messages_grpc::ValidatorHealthRequest};
use parking_lot::RwLock;
use tokio::{
    task::{JoinHandle, JoinSet},
    time::{Instant, interval, timeout},
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
pub struct ValidatorClientMonitor {
    config: ValidatorClientMonitorConfig,
    metrics: Arc<ValidatorClientMetrics>,
    client_stats: RwLock<ClientObservedStats>,
}

impl ValidatorClientMonitor {
    pub fn new(config: ValidatorClientMonitorConfig, metrics: Arc<ValidatorClientMetrics>) -> Self {
        info!(
            "Validator client monitor starting with config: {:?}",
            config
        );

        Self {
            config: config.clone(),
            metrics,
            client_stats: RwLock::new(ClientObservedStats::new(config)),
        }
    }

    pub fn spawn_health_checks<A: AuthorityAPI + Send + Sync + 'static>(
        self: &Arc<Self>,
        authority_aggregator: &Arc<ArcSwap<AuthorityAggregator<A>>>,
    ) -> JoinHandle<()> {
        let period = self.config.health_check_interval;
        let monitor = Arc::downgrade(self);
        let authority_aggregator = Arc::downgrade(authority_aggregator);
        // weak pointers allow health check task break early once shared arc objects are
        // dropped
        tokio::spawn(async move {
            Self::run_health_checks(monitor, authority_aggregator, period).await;
        })
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        // Use a fresh isolated registry per test instance to prevent parallel
        // tests from conflicting when registering metrics with the same names
        // into the global default registry.
        Self::new(
            ValidatorClientMonitorConfig::default(),
            Arc::new(ValidatorClientMetrics::new(
                &prometheus_filtered::Registry::new(),
            )),
        )
    }

    fn spawn_health_checks_tasks<A: AuthorityAPI + Send + Sync + 'static>(
        self: Arc<Self>,
        authority_agg: &Arc<AuthorityAggregator<A>>,
    ) -> JoinSet<()> {
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
                    client.health_check(ValidatorHealthRequest {}),
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

    /// Background task that runs periodic health checks on all validators.
    async fn run_health_checks<A: AuthorityAPI + Send + Sync + 'static>(
        monitor: Weak<Self>,
        authority_aggregator: Weak<ArcSwap<AuthorityAggregator<A>>>,
        period: Duration,
    ) {
        let mut interval = interval(period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;
            let Some(monitor) = monitor.upgrade() else {
                break;
            };
            let Some(authority_agg) = authority_aggregator.upgrade() else {
                break;
            };
            let authority_agg = authority_agg.load();
            let mut tasks = monitor.spawn_health_checks_tasks(&*authority_agg);
            drop(authority_agg);
            while let Some(result) = tasks.join_next().await {
                if let Err(e) = result {
                    warn!("Health check task failed: {}", e);
                }
            }
        }
    }

    /// Record client-observed interaction result with a validator.
    pub fn record_interaction_result(&self, feedback: OperationFeedback) {
        let score = self
            .client_stats
            .write()
            .record_interaction_result(&feedback);
        self.metrics.record_interaction_result(&feedback, score);
    }

    /// Select validators based on client-observed performance.
    ///
    /// Validators with the best performance and exploration scores are shuffled
    /// and placed in the front of the list to avoid overload and guarantee
    /// uniform exploration. The rest of the validators follow in the order
    /// from best to worst combined score.
    pub fn select_shuffled_preferred_validators<'a>(
        &self,
        committee: impl Iterator<Item = &'a AuthorityName>,
    ) -> Vec<&'a AuthorityName> {
        let rng = rand::thread_rng();
        let now = Instant::now();
        let validators = self
            .client_stats
            .read()
            .select_shuffled_preferred_validators(committee, now, rng);
        validators
    }
}
