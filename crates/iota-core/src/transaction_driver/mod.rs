// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod effects_certifier;
mod error;
mod metrics;
mod request_retrier;
mod transaction_submitter;

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use arc_swap::ArcSwap;
use effects_certifier::*;
/// Exports
pub use error::{AggregatedRequestErrors, TransactionDriverError};
use iota_common::backoff::ExponentialBackoff;
use iota_metrics::{monitored_future, spawn_logged_monitored_task};
use iota_types::{
    committee::EpochId,
    messages_grpc::{SubmitTransactionsRequest, TxStatusUpdate},
};
pub use metrics::*;
use parking_lot::Mutex;
use rand::Rng;
use tokio::{
    task::JoinSet,
    time::{interval, sleep},
};
use tracing::instrument;
use transaction_submitter::*;

use crate::{
    authority_aggregator::AuthorityAggregator,
    authority_client::AuthorityAPI,
    validator_client_monitor::{
        OperationFeedback, OperationType, ValidatorClientMetrics, ValidatorClientMonitor,
    },
};

pub mod reconfig_observer;
pub use reconfig_observer::ReconfigObserver;

/// Trait for components that can update their AuthorityAggregator during
/// reconfiguration. Used by ReconfigObserver to notify components of epoch
/// changes.
pub trait AuthorityAggregatorUpdatable<A: Clone>: Send + Sync + 'static {
    fn epoch(&self) -> EpochId;
    fn authority_aggregator(&self) -> Arc<AuthorityAggregator<A>>;
    fn update_authority_aggregator(&self, new_authorities: Arc<AuthorityAggregator<A>>);
}

use iota_config::node::NodeConfig;

/// Options for submitting a transaction.
#[derive(Clone, Default, Debug)]
pub struct SubmitTransactionOptions {
    /// When forwarding transactions on behalf of a client, this is the client's
    /// address specified for ddos protection.
    pub forwarded_client_addr: Option<SocketAddr>,

    /// When submitting a transaction, only the validators in the allowed
    /// validator list can be used to submit the transaction to.
    /// When the allowed validator list is empty, any validator can be used.
    pub allowed_validators: Vec<String>,

    /// When submitting a transaction, the validators in the blocked validator
    /// list cannot be used to submit the transaction to. When the blocked
    /// validator list is empty, no restrictions are applied.
    pub blocked_validators: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct QuorumTransactionResponse {
    pub effects: iota_types::transaction_driver_types::FinalizedEffects,

    pub events: Option<iota_types::effects::TransactionEvents>,
    // Input objects will only be populated in the happy path
    pub input_objects: Option<Vec<iota_types::object::Object>>,
    // Output objects will only be populated in the happy path
    pub output_objects: Option<Vec<iota_types::object::Object>>,
    pub auxiliary_data: Option<Vec<u8>>,
}

pub struct TransactionDriver<A: Clone> {
    authority_aggregator: Arc<ArcSwap<AuthorityAggregator<A>>>,
    state: Mutex<State>,
    metrics: Arc<TransactionDriverMetrics>,
    submitter: TransactionSubmitter,
    certifier: EffectsCertifier,
    client_monitor: Arc<ValidatorClientMonitor<A>>,
}

impl<A> TransactionDriver<A>
where
    A: AuthorityAPI + Send + Sync + 'static + Clone,
{
    pub fn new(
        authority_aggregator: Arc<AuthorityAggregator<A>>,
        reconfig_observer: Arc<dyn ReconfigObserver<A> + Sync + Send>,
        metrics: Arc<TransactionDriverMetrics>,
        node_config: Option<&NodeConfig>,
        client_metrics: Arc<ValidatorClientMetrics>,
    ) -> Arc<Self> {
        let shared_swap = Arc::new(ArcSwap::new(authority_aggregator));

        // Extract validator client monitor config from NodeConfig or use default
        let monitor_config = node_config
            .and_then(|nc| nc.validator_client_monitor_config.clone())
            .unwrap_or_default();
        let client_monitor =
            ValidatorClientMonitor::new(monitor_config, client_metrics, shared_swap.clone());

        let driver = Arc::new(Self {
            authority_aggregator: shared_swap,
            state: Mutex::new(State::new()),
            metrics: metrics.clone(),
            submitter: TransactionSubmitter::new(metrics.clone()),
            certifier: EffectsCertifier::new(metrics),
            client_monitor,
        });

        let driver_clone = driver.clone();

        spawn_logged_monitored_task!(Self::run_latency_checks(driver_clone));

        driver.enable_reconfig(reconfig_observer);
        driver
    }

    /// Returns the authority aggregator wrapper which upgrades on epoch
    /// changes.
    pub fn authority_aggregator(&self) -> &Arc<ArcSwap<AuthorityAggregator<A>>> {
        &self.authority_aggregator
    }

    /// Drives transaction to finalization.
    ///
    /// Internally, retries the attempt to finalize a transaction until:
    /// - The transaction is finalized.
    /// - The transaction observes a non-retriable error.
    /// - Timeout is reached.
    #[instrument(level = "error", skip_all, fields(tx_digest = ?request.tx_digest(), ping = %request.is_ping()))]
    pub async fn drive_transaction(
        &self,
        request: SubmitTransactionsRequest,
        options: SubmitTransactionOptions,
        timeout_duration: Option<Duration>,
    ) -> Result<QuorumTransactionResponse, TransactionDriverError> {
        const MAX_DRIVE_TRANSACTION_RETRY_DELAY: Duration = Duration::from_secs(10);

        // The amplification factor controls how many validators to submit to
        // simultaneously. For ping requests and the initial IOTA port, always
        // use 1. TODO: Implement proper amplification factor based on gas_price
        // / reference_gas_price.
        let amplification_factor: u64 = 1;

        let ping_label = request.transactions.is_empty().to_string();
        let ping_label = ping_label.as_str();

        let timer = Instant::now();

        self.metrics
            .total_transactions_submitted
            .with_label_values(&[ping_label])
            .inc();

        let mut backoff = ExponentialBackoff::new(
            Duration::from_millis(100),
            MAX_DRIVE_TRANSACTION_RETRY_DELAY,
        );
        let mut attempts = 0;
        let mut latest_retriable_error = None;

        let retry_loop = async {
            loop {
                match self
                    .drive_transaction_once(amplification_factor, request.clone(), &options)
                    .await
                {
                    Ok(resp) => {
                        let settlement_finality_latency = timer.elapsed().as_secs_f64();
                        self.metrics
                            .settlement_finality_latency
                            .with_label_values(&[ping_label])
                            .observe(settlement_finality_latency);
                        // Record the number of retries for successful transaction
                        self.metrics
                            .transaction_retries
                            .with_label_values(&["success", ping_label])
                            .observe(attempts as f64);
                        return Ok(resp);
                    }
                    Err(e) => {
                        let error_category: &str = e.categorize().into();
                        self.metrics
                            .drive_transaction_errors
                            .with_label_values(&[error_category, ping_label])
                            .inc();
                        if !e.is_submission_retriable() {
                            // Record the number of retries for failed transaction
                            self.metrics
                                .transaction_retries
                                .with_label_values(&["failure", ping_label])
                                .observe(attempts as f64);
                            if !request.transactions.is_empty() {
                                tracing::info!(
                                    "User transaction failed to finalize (attempt {}), with non-retriable error: {}",
                                    attempts,
                                    e
                                );
                            }
                            return Err(e);
                        }
                        if !request.transactions.is_empty() {
                            tracing::info!(
                                "User transaction failed to finalize (attempt {}): {}. Retrying ...",
                                attempts,
                                e
                            );
                        }
                        // Buffer the latest retriable error to be returned in case of timeout
                        latest_retriable_error = Some(e);
                    }
                }

                use iota_types::error::ErrorCategory;
                let overload = if let Some(e) = &latest_retriable_error {
                    e.categorize() == ErrorCategory::ValidatorOverloaded
                } else {
                    false
                };
                let delay = if overload {
                    // Increase delay during overload.
                    const OVERLOAD_ADDITIONAL_DELAY: Duration = Duration::from_secs(10);
                    backoff.next().unwrap() + OVERLOAD_ADDITIONAL_DELAY
                } else {
                    backoff.next().unwrap()
                };
                sleep(delay).await;

                attempts += 1;
            }
        };

        match timeout_duration {
            Some(duration) => {
                tokio::time::timeout(duration, retry_loop)
                    .await
                    .unwrap_or_else(|_| {
                        // Timeout occurred, return with latest retriable error if available
                        let e = TransactionDriverError::TimeoutWithLastRetriableError {
                            last_error: latest_retriable_error.map(Box::new),
                            attempts,
                            timeout: duration,
                        };
                        if !request.transactions.is_empty() {
                            tracing::info!(
                                "User transaction timed out after {} attempts. Last error: {}",
                                attempts,
                                e
                            );
                        }
                        Err(e)
                    })
            }
            None => retry_loop.await,
        }
    }

    #[instrument(level = "error", skip_all, err(level = "debug"))]
    async fn drive_transaction_once(
        &self,
        amplification_factor: u64,
        request: SubmitTransactionsRequest,
        options: &SubmitTransactionOptions,
    ) -> Result<QuorumTransactionResponse, TransactionDriverError> {
        let auth_agg = self.authority_aggregator.load();
        let amplification_factor =
            amplification_factor.min(auth_agg.committee.num_members() as u64);
        let start_time = Instant::now();
        let tx_digest = request.tx_digest();
        let is_ping = request.transactions.is_empty();

        let (name, submit_txn_result) = self
            .submitter
            .submit_transaction(
                &auth_agg,
                &self.client_monitor,
                amplification_factor,
                request,
                options,
            )
            .await?;
        match &submit_txn_result {
            TxStatusUpdate::Rejected { error } => {
                return Err(TransactionDriverError::ClientInternal {
                    error: format!(
                        "TxStatusUpdate::Rejected should have been returned as an error in submit_transaction(): {error:?}",
                    ),
                });
            }
            TxStatusUpdate::Expired { epoch } => {
                return Err(TransactionDriverError::ClientInternal {
                    error: format!(
                        "TxStatusUpdate::Expired should have been returned as an error in submit_transaction() (epoch {epoch})",
                    ),
                });
            }
            _ => {}
        }

        // Wait for quorum effects using EffectsCertifier
        let result = self
            .certifier
            .get_certified_finalized_effects(
                &auth_agg,
                &self.client_monitor,
                tx_digest.first().cloned(),
                name,
                submit_txn_result,
                options,
            )
            .await;

        if result.is_ok() {
            self.client_monitor
                .record_interaction_result(OperationFeedback {
                    authority_name: name,
                    display_name: auth_agg.get_display_name(&name),
                    operation: OperationType::Consensus,
                    ping: is_ping,
                    result: Ok(start_time.elapsed()),
                });
        }
        result
    }

    // Runs a background task to send ping transactions to all validators to perform
    // latency checks.
    async fn run_latency_checks(self: Arc<Self>) {
        const INTERVAL_BETWEEN_RUNS: Duration = Duration::from_secs(15);
        const MAX_JITTER: Duration = Duration::from_secs(10);
        const PING_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

        let mut interval = interval(INTERVAL_BETWEEN_RUNS);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            let mut tasks = JoinSet::new();

            Self::ping(self.clone(), &mut tasks, MAX_JITTER, PING_REQUEST_TIMEOUT);

            while let Some(result) = tasks.join_next().await {
                if let Err(e) = result {
                    tracing::debug!("Error while driving ping transaction: {}", e);
                }
            }
        }
    }

    /// Pings all validators for e2e latency with the provided transaction type.
    fn ping(
        self: Arc<Self>,
        tasks: &mut JoinSet<()>,
        max_jitter: Duration,
        ping_timeout: Duration,
    ) {
        let auth_agg = self.authority_aggregator.load().clone();
        let validators = auth_agg.committee.names().cloned().collect::<Vec<_>>();

        self.metrics.latency_check_runs.inc();

        for name in validators {
            let display_name = auth_agg.get_display_name(&name);
            let delay_ms = rand::thread_rng().gen_range(0..max_jitter.as_millis()) as u64;
            let self_clone = self.clone();

            let task = async move {
                // Add some random delay to the task to avoid all tasks running at the same time
                if delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
                let start_time = Instant::now();

                // Now send a ping transaction to the chosen validator for the provided tx type
                match self_clone
                    .drive_transaction(
                        SubmitTransactionsRequest::new_ping(),
                        SubmitTransactionOptions {
                            allowed_validators: vec![display_name.clone()],
                            ..Default::default()
                        },
                        Some(ping_timeout),
                    )
                    .await
                {
                    Ok(_) => {
                        tracing::debug!(
                            "Ping transaction to validator {} completed end to end in {} seconds",
                            display_name,
                            start_time.elapsed().as_secs_f64()
                        );
                    }
                    Err(err) => {
                        tracing::debug!(
                            "Failed to get certified finalized effects, for ping transaction to validator {}: {}",
                            display_name,
                            err
                        );
                    }
                }
            };

            tasks.spawn(task);
        }
    }

    fn enable_reconfig(
        self: &Arc<Self>,
        reconfig_observer: Arc<dyn ReconfigObserver<A> + Sync + Send>,
    ) {
        let driver = self.clone();
        self.state.lock().tasks.spawn(monitored_future!(async move {
            let mut reconfig_observer = reconfig_observer.clone_boxed();
            reconfig_observer.run(driver).await;
        }));
    }
}

impl<A> AuthorityAggregatorUpdatable<A> for TransactionDriver<A>
where
    A: AuthorityAPI + Send + Sync + 'static + Clone,
{
    fn epoch(&self) -> EpochId {
        self.authority_aggregator.load().committee.epoch
    }

    fn authority_aggregator(&self) -> Arc<AuthorityAggregator<A>> {
        self.authority_aggregator.load_full()
    }

    fn update_authority_aggregator(&self, new_authorities: Arc<AuthorityAggregator<A>>) {
        tracing::info!(
            "Transaction Driver updating AuthorityAggregator with committee {}",
            new_authorities.committee
        );

        self.authority_aggregator.store(new_authorities);
    }
}

// Inner state of TransactionDriver.
struct State {
    tasks: JoinSet<()>,
}

impl State {
    fn new() -> Self {
        Self {
            tasks: JoinSet::new(),
        }
    }
}
