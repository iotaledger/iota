// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Transaction Orchestrator is a Node component that utilizes Quorum Driver (or
// optionally TransactionDriver) to submit transactions to validators for
// finality, and proactively executes finalized transactions locally.

use std::{
    collections::BTreeMap, net::SocketAddr, ops::Deref, path::Path, sync::Arc, time::Duration,
};

use futures::{
    FutureExt,
    future::{Either, Future, select},
};
use iota_common::sync::notify_read::NotifyRead;
use iota_config::NodeConfig;
use iota_metrics::{
    TX_TYPE_SHARED_OBJ_TX, TX_TYPE_SINGLE_WRITER_TX, add_server_timing,
    spawn_logged_monitored_task, spawn_monitored_task,
};
use iota_storage::write_path_pending_tx_log::WritePathPendingTransactionLog;
use iota_types::{
    base_types::TransactionDigest,
    error::{IotaError, IotaResult},
    iota_system_state::IotaSystemState,
    messages_checkpoint::CheckpointSequenceNumber,
    messages_grpc::SubmitTxRequest,
    quorum_driver_types::{
        EffectsFinalityInfo, ExecuteTransactionRequestType, ExecuteTransactionRequestV1,
        ExecuteTransactionResponseV1, FinalizedEffects, IsTransactionExecutedLocally,
        QuorumDriverEffectsQueueResult, QuorumDriverError, QuorumDriverResponse,
        QuorumDriverResult,
    },
    transaction::{TransactionData, VerifiedTransaction},
    transaction_driver_types::{
        EffectsFinalityInfo as TdEffectsFinalityInfo, FinalizedEffects as TdFinalizedEffects,
    },
    transaction_executor::{SimulateTransactionResult, VmChecks},
};
use prometheus::{
    Histogram, Registry,
    core::{AtomicI64, AtomicU64, GenericCounter, GenericGauge},
    register_histogram_vec_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, register_int_gauge_vec_with_registry,
    register_int_gauge_with_registry,
};
use tokio::{
    sync::broadcast::{Receiver, error::RecvError},
    task::JoinHandle,
    time::timeout,
};
use tracing::{Instrument, debug, error, info, instrument, trace_span, warn};

use crate::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    authority_aggregator::AuthorityAggregator,
    authority_client::{AuthorityAPI, NetworkAuthorityClient},
    quorum_driver::{
        QuorumDriverHandler, QuorumDriverHandlerBuilder, QuorumDriverMetrics,
        reconfig_observer::{OnsiteReconfigObserver, ReconfigObserver},
    },
    transaction_driver::{
        AggregatedRequestErrors, QuorumTransactionResponse, SubmitTransactionOptions,
        TransactionDriver, TransactionDriverError, TransactionDriverMetrics,
        reconfig_observer::OnsiteReconfigObserver as TdOnsiteReconfigObserver,
    },
    validator_client_monitor::ValidatorClientMetrics,
};

// How long to wait for local execution (including parents) before a timeout
// is returned to client.
const LOCAL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(10);

const WAIT_FOR_FINALITY_TIMEOUT: Duration = Duration::from_secs(30);

/// Transaction Orchestrator is a Node component that supports both QuorumDriver
/// and TransactionDriver for submitting transactions to validators for
/// finality. It adds inflight deduplication, waiting for local execution,
/// recovery, and epoch change handling.
pub struct TransactionOrchestrator<A: Clone> {
    // QuorumDriverHandler for the normal flow. Always present if white flag flow is disabled, and
    // None if white flag flow is enabled.
    quorum_driver_handler: Option<Arc<QuorumDriverHandler<A>>>,
    /// Optional TransactionDriver for the white flag direct-to-consensus flow.
    transaction_driver: Option<Arc<TransactionDriver<A>>>,
    validator_state: Arc<AuthorityState>,
    _local_executor_handle: Option<JoinHandle<()>>,
    pending_tx_log: Arc<WritePathPendingTransactionLog>,
    notifier: Arc<NotifyRead<TransactionDigest, QuorumDriverResult>>,
    metrics: Arc<TransactionOrchestratorMetrics>,
}

impl TransactionOrchestrator<NetworkAuthorityClient> {
    pub fn new_with_auth_aggregator(
        validators: Arc<AuthorityAggregator<NetworkAuthorityClient>>,
        validator_state: Arc<AuthorityState>,
        reconfig_channel: Receiver<IotaSystemState>,
        parent_path: &Path,
        prometheus_registry: &Registry,
        node_config: Option<&NodeConfig>,
    ) -> Self {
        // Check protocol config to determine if white flag flow is enabled
        let epoch_store = validator_state.load_epoch_store_one_call_per_task();
        let use_transaction_driver = epoch_store.protocol_config().enable_white_flag_flow();

        // Create TransactionDriver reconfig observer only if white flag is enabled
        let td_reconfig_observer = if use_transaction_driver {
            Some(TdOnsiteReconfigObserver::new(
                reconfig_channel.resubscribe(),
                validator_state.get_object_cache_reader().clone(),
                validator_state.clone_committee_store(),
                validators.safe_client_metrics_base.clone(),
            ))
        } else {
            None
        };

        // Create QuorumDriver reconfig observer only if white flag is NOT enabled
        let qd_reconfig_observer = if !use_transaction_driver {
            Some(OnsiteReconfigObserver::new(
                reconfig_channel.resubscribe(),
                validator_state.get_object_cache_reader().clone(),
                validator_state.clone_committee_store(),
                validators.safe_client_metrics_base.clone(),
                validators.metrics.deref().clone(),
            ))
        } else {
            None
        };

        TransactionOrchestrator::new(
            validators,
            validator_state,
            parent_path,
            prometheus_registry,
            qd_reconfig_observer,
            td_reconfig_observer,
            node_config,
        )
    }
}

impl<A> TransactionOrchestrator<A>
where
    A: AuthorityAPI + Send + Sync + 'static + Clone,
    OnsiteReconfigObserver: ReconfigObserver<A>,
    TdOnsiteReconfigObserver: crate::transaction_driver::reconfig_observer::ReconfigObserver<A>,
{
    pub fn new(
        validators: Arc<AuthorityAggregator<A>>,
        validator_state: Arc<AuthorityState>,
        parent_path: &Path,
        prometheus_registry: &Registry,
        reconfig_observer: Option<OnsiteReconfigObserver>,
        td_reconfig_observer: Option<TdOnsiteReconfigObserver>,
        node_config: Option<&NodeConfig>,
    ) -> Self {
        // Check protocol config to determine if white flag flow is enabled
        let epoch_store = validator_state.load_epoch_store_one_call_per_task();
        let use_transaction_driver = epoch_store.protocol_config().enable_white_flag_flow();

        let qd_metrics = Arc::new(QuorumDriverMetrics::new(prometheus_registry));
        let notifier = Arc::new(NotifyRead::new());

        // Create QuorumDriver only if white flag is NOT enabled
        let (quorum_driver_handler, effects_receiver) = if !use_transaction_driver {
            let reconfig_observer = Arc::new(
                reconfig_observer
                    .expect("QuorumDriver reconfig observer required when white flag is disabled"),
            );
            let handler = Arc::new(
                QuorumDriverHandlerBuilder::new(validators.clone(), qd_metrics)
                    .with_notifier(notifier.clone())
                    .with_reconfig_observer(reconfig_observer)
                    .start(),
            );
            let receiver = handler.subscribe_to_effects();
            (Some(handler), Some(receiver))
        } else {
            (None, None)
        };

        // Create TransactionDriver only if white flag is enabled
        let transaction_driver = if use_transaction_driver {
            let td_metrics = Arc::new(TransactionDriverMetrics::new(prometheus_registry));
            let client_metrics = Arc::new(ValidatorClientMetrics::new(prometheus_registry));
            let observer = td_reconfig_observer
                .expect("TransactionDriver reconfig observer required when white flag is enabled");
            Some(TransactionDriver::new(
                validators,
                Arc::new(observer),
                td_metrics,
                node_config,
                client_metrics,
            ))
        } else {
            None
        };

        let metrics = Arc::new(TransactionOrchestratorMetrics::new(prometheus_registry));
        let pending_tx_log = Arc::new(WritePathPendingTransactionLog::new(
            parent_path.join("fullnode_pending_transactions"),
        ));

        // Start pending transaction log cleanup only if QuorumDriver is used
        let _local_executor_handle =
            if let (Some(handler), Some(receiver)) = (&quorum_driver_handler, effects_receiver) {
                let pending_tx_log_clone = pending_tx_log.clone();
                let res = Some(spawn_monitored_task!(async move {
                    Self::loop_pending_transaction_log(receiver, pending_tx_log_clone).await;
                }));

                // Schedule pending transaction recovery (QuorumDriver mode only;
                // TransactionDriver does not track pending certificates)
                Self::schedule_txes_in_log(pending_tx_log.clone(), handler.clone());

                res
            } else {
                // TransactionDriver mode: no pending tx log cleanup needed
                // (transactions go directly to consensus, no certificate tracking)
                None
            };

        Self {
            quorum_driver_handler,
            transaction_driver,
            validator_state,
            _local_executor_handle,
            pending_tx_log,
            notifier,
            metrics,
        }
    }
}

impl<A> TransactionOrchestrator<A>
where
    A: AuthorityAPI + Send + Sync + 'static + Clone,
{
    #[instrument(name = "tx_orchestrator_execute_transaction_block", level = "trace", skip_all,
    fields(
        tx_digest = ?request.transaction.digest(),
        tx_type = ?request_type,
    ),
    err)]
    pub async fn execute_transaction_block(
        &self,
        request: ExecuteTransactionRequestV1,
        request_type: ExecuteTransactionRequestType,
        client_addr: Option<SocketAddr>,
    ) -> Result<(ExecuteTransactionResponseV1, IsTransactionExecutedLocally), QuorumDriverError>
    {
        let epoch_store = self.validator_state.load_epoch_store_one_call_per_task();

        // Use TransactionDriver if configured, otherwise fall back to QuorumDriver.
        let (transaction, response) = if let Some(td) = &self.transaction_driver {
            self.submit_with_transaction_driver(td.clone(), &epoch_store, request, client_addr)
                .await?
        } else {
            let (tx, qd_resp) = self
                .execute_transaction_impl(&epoch_store, request, client_addr)
                .await?;
            let resp = quorum_driver_response_to_v1(qd_resp);
            (tx, resp)
        };

        let executed_locally = if matches!(
            request_type,
            ExecuteTransactionRequestType::WaitForLocalExecution
        ) {
            let executed_locally = Self::wait_for_finalized_tx_executed_locally_with_timeout(
                &self.validator_state,
                &transaction,
                &self.metrics,
            )
            .await
            .is_ok();
            add_server_timing("local_execution");
            executed_locally
        } else {
            false
        };

        Ok((response, executed_locally))
    }

    // Utilize the handle_certificate_v1 validator api to request input/output
    // objects
    #[instrument(name = "tx_orchestrator_execute_transaction_v1", level = "trace", skip_all,
                 fields(tx_digest = ?request.transaction.digest()))]
    pub async fn execute_transaction_v1(
        &self,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let epoch_store = self.validator_state.load_epoch_store_one_call_per_task();

        if let Some(td) = &self.transaction_driver {
            let (_, response) = self
                .submit_with_transaction_driver(td.clone(), &epoch_store, request, client_addr)
                .await?;
            return Ok(response);
        }

        let qd_resp = self
            .execute_transaction_impl(&epoch_store, request, client_addr)
            .await
            .map(|(_, r)| r)?;

        Ok(quorum_driver_response_to_v1(qd_resp))
    }

    /// Submit a transaction using the TransactionDriver (white flag flow).
    #[instrument(name = "tx_orchestrator_submit_with_td", level = "trace", skip_all,
                 fields(tx_digest = ?request.transaction.digest()))]
    async fn submit_with_transaction_driver(
        &self,
        td: Arc<TransactionDriver<A>>,
        epoch_store: &AuthorityPerEpochStore,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<(VerifiedTransaction, ExecuteTransactionResponseV1), QuorumDriverError> {
        let transaction = epoch_store
            .verify_transaction(request.transaction.clone())
            .map_err(QuorumDriverError::InvalidUserSignature)?;
        let tx_digest = *transaction.digest();

        // TODO: add transaction to some struct to prevent sending the same transaction
        // multiple times in case client sends it multiple times if self
        //     .pending_tx_log
        //     .write_pending_transaction_maybe(&transaction)
        //     .await
        //     .map_err(|e| QuorumDriverError::QuorumDriverInternal(e))?
        // {
        //     debug!(?tx_digest, "no pending request in flight, submitting to
        // TransactionDriver."); } else {
        //     debug!(?tx_digest, "transaction already in flight, skipping duplicate
        // submission."); }

        let td_response = td
            .drive_transaction(
                SubmitTxRequest::new_transaction(request.transaction),
                SubmitTransactionOptions {
                    forwarded_client_addr: client_addr,
                    ..Default::default()
                },
                Some(WAIT_FOR_FINALITY_TIMEOUT),
            )
            .await
            .map_err(map_td_error_to_qd)?;

        debug!(
            "TransactionOrchestrator: TransactionDriver submission succeeded for transaction {}",
            tx_digest
        );

        let QuorumTransactionResponse {
            effects: td_effects,
            events,
            input_objects,
            output_objects,
            auxiliary_data,
        } = td_response;

        let effects = convert_td_to_qd_effects(td_effects);
        let response = ExecuteTransactionResponseV1 {
            effects,
            events: if request.include_events { events } else { None },
            input_objects: if request.include_input_objects {
                input_objects
            } else {
                None
            },
            output_objects: if request.include_output_objects {
                output_objects
            } else {
                None
            },
            auxiliary_data: if request.include_auxiliary_data {
                auxiliary_data
            } else {
                None
            },
        };

        Ok((transaction, response))
    }

    // TODO check if tx is already executed on this node.
    // Note: since EffectsCert is not stored today, we need to gather that from
    // validators (and maybe store it for caching purposes)
    #[instrument(level = "trace", skip_all, fields(tx_digest = ?request.transaction.digest()))]
    pub async fn execute_transaction_impl(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<(VerifiedTransaction, QuorumDriverResponse), QuorumDriverError> {
        // Reject malformed transactions before any code path inspects shared
        // inputs or `MoveAuthenticator`
        request
            .transaction
            .validity_check(epoch_store.protocol_config(), epoch_store.epoch())
            .map_err(QuorumDriverError::InvalidTransaction)?;
        let transaction = epoch_store
            .verify_transaction(request.transaction.clone())
            .map_err(QuorumDriverError::InvalidUserSignature)?;
        let (_in_flight_metrics_guards, good_response_metrics) = self.update_metrics(&transaction);
        let tx_digest = *transaction.digest();
        debug!(?tx_digest, "TO Received transaction execution request.");

        let (_e2e_latency_timer, _txn_finality_timer) = if transaction.contains_shared_object() {
            (
                self.metrics.request_latency_shared_obj.start_timer(),
                self.metrics
                    .wait_for_finality_latency_shared_obj
                    .start_timer(),
            )
        } else {
            (
                self.metrics.request_latency_single_writer.start_timer(),
                self.metrics
                    .wait_for_finality_latency_single_writer
                    .start_timer(),
            )
        };

        // TODO: refactor all the gauge and timer metrics with `monitored_scope`
        let wait_for_finality_gauge = self.metrics.wait_for_finality_in_flight.clone();
        wait_for_finality_gauge.inc();
        let _wait_for_finality_gauge = scopeguard::guard(wait_for_finality_gauge, |in_flight| {
            in_flight.dec();
        });

        let ticket = self
            .submit(transaction.clone(), request, client_addr)
            .await
            .map_err(|e| {
                warn!(?tx_digest, "QuorumDriverInternalError: {e:?}");
                QuorumDriverError::QuorumDriverInternal(e)
            })?;

        let Ok(result) = timeout(WAIT_FOR_FINALITY_TIMEOUT, ticket).await else {
            debug!(?tx_digest, "Timeout waiting for transaction finality.");
            self.metrics.wait_for_finality_timeout.inc();
            return Err(QuorumDriverError::TimeoutBeforeFinality);
        };
        add_server_timing("wait_for_finality");

        drop(_txn_finality_timer);
        drop(_wait_for_finality_gauge);
        self.metrics.wait_for_finality_finished.inc();

        match result {
            Err(err) => {
                warn!(?tx_digest, "QuorumDriverInternalError: {err:?}");
                Err(QuorumDriverError::QuorumDriverInternal(err))
            }
            Ok(Err(err)) => Err(err),
            Ok(Ok(response)) => {
                good_response_metrics.inc();
                Ok((transaction, response))
            }
        }
    }

    /// Submits the transaction to Quorum Driver for execution.
    /// Returns an awaitable Future.
    #[instrument(name = "tx_orchestrator_submit", level = "trace", skip_all)]
    async fn submit(
        &self,
        transaction: VerifiedTransaction,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> IotaResult<impl Future<Output = IotaResult<QuorumDriverResult>> + '_> {
        let tx_digest = *transaction.digest();
        let ticket = self.notifier.register_one(&tx_digest);
        // TODO(william) need to also write client adr to pending tx log below
        // so that we can re-execute with this client addr if we restart
        if self
            .pending_tx_log
            .write_pending_transaction_maybe(&transaction)
            .await?
        {
            debug!(?tx_digest, "no pending request in flight, submitting.");
            self.quorum_driver()
                .submit_transaction_no_ticket(request.clone(), client_addr)
                .await?;
        }
        // It's possible that the transaction effects is already stored in DB at this
        // point. So we also subscribe to that. If we hear from `effects_await`
        // first, it means the ticket misses the previous notification, and we
        // want to ask quorum driver to form a certificate for us again, to
        // serve this request.
        let cache_reader = self.validator_state.get_transaction_cache_reader().clone();
        let qd = self.clone_quorum_driver();
        Ok(async move {
            let digests = [tx_digest];
            let effects_await = cache_reader.try_notify_read_executed_effects(&digests);
            // let-and-return necessary to satisfy borrow checker.
            let res = match select(ticket, effects_await.boxed()).await {
                Either::Left((quorum_driver_response, _)) => Ok(quorum_driver_response),
                Either::Right((_, unfinished_quorum_driver_task)) => {
                    debug!(
                        ?tx_digest,
                        "Effects are available in DB, use quorum driver to get a certificate"
                    );
                    qd.submit_transaction_no_ticket(request, client_addr)
                        .await?;
                    Ok(unfinished_quorum_driver_task.await)
                }
            };
            res
        })
    }

    #[instrument(name = "tx_orchestrator_wait_for_finalized_tx_executed_locally_with_timeout", level = "debug", skip_all, fields(tx_digest = ?transaction.digest()), err)]
    async fn wait_for_finalized_tx_executed_locally_with_timeout(
        validator_state: &Arc<AuthorityState>,
        transaction: &VerifiedTransaction,
        metrics: &TransactionOrchestratorMetrics,
    ) -> IotaResult {
        let tx_digest = *transaction.digest();
        metrics.local_execution_in_flight.inc();
        let _metrics_guard =
            scopeguard::guard(metrics.local_execution_in_flight.clone(), |in_flight| {
                in_flight.dec();
            });

        let _guard = if transaction.contains_shared_object() {
            metrics.local_execution_latency_shared_obj.start_timer()
        } else {
            metrics.local_execution_latency_single_writer.start_timer()
        };
        debug!(
            ?tx_digest,
            "Waiting for finalized tx to be executed locally."
        );
        match timeout(
            LOCAL_EXECUTION_TIMEOUT,
            validator_state
                .get_transaction_cache_reader()
                .try_notify_read_executed_effects_digests(&[tx_digest]),
        )
        .instrument(trace_span!("local_execution"))
        .await
        {
            Err(_elapsed) => {
                debug!(
                    ?tx_digest,
                    "Waiting for finalized tx to be executed locally timed out within {:?}.",
                    LOCAL_EXECUTION_TIMEOUT
                );
                metrics.local_execution_timeout.inc();
                Err(IotaError::Timeout)
            }
            Ok(Err(err)) => {
                debug!(
                    ?tx_digest,
                    "Waiting for finalized tx to be executed locally failed with error: {:?}", err
                );
                metrics.local_execution_failure.inc();
                Err(IotaError::TransactionOrchestratorLocalExecution {
                    error: err.to_string(),
                })
            }
            Ok(Ok(_)) => {
                metrics.local_execution_success.inc();
                Ok(())
            }
        }
    }

    // TODO: Potentially cleanup this function and pending transaction log.
    async fn loop_pending_transaction_log(
        mut effects_receiver: Receiver<QuorumDriverEffectsQueueResult>,
        pending_transaction_log: Arc<WritePathPendingTransactionLog>,
    ) {
        loop {
            match effects_receiver.recv().await {
                Ok(Ok((transaction, ..))) => {
                    let tx_digest = transaction.digest();
                    if let Err(err) = pending_transaction_log.finish_transaction(tx_digest) {
                        error!(
                            ?tx_digest,
                            "Failed to finish transaction in pending transaction log: {err}"
                        );
                    }
                }
                Ok(Err((tx_digest, _err))) => {
                    if let Err(err) = pending_transaction_log.finish_transaction(&tx_digest) {
                        error!(
                            ?tx_digest,
                            "Failed to finish transaction in pending transaction log: {err}"
                        );
                    }
                }
                Err(RecvError::Closed) => {
                    error!("Sender of effects subscriber queue has been dropped!");
                    return;
                }
                Err(RecvError::Lagged(skipped_count)) => {
                    warn!("Skipped {skipped_count} transasctions in effects subscriber queue.");
                }
            }
        }
    }

    pub fn quorum_driver(&self) -> &Arc<QuorumDriverHandler<A>> {
        self.quorum_driver_handler
            .as_ref()
            .expect("QuorumDriverHandler is not initialized.")
    }

    pub fn clone_quorum_driver(&self) -> Arc<QuorumDriverHandler<A>> {
        self.quorum_driver_handler
            .clone()
            .expect("QuorumDriverHandler is not initialized.")
    }

    pub fn transaction_driver(&self) -> Option<&Arc<TransactionDriver<A>>> {
        self.transaction_driver.as_ref()
    }

    pub fn clone_authority_aggregator(&self) -> Arc<AuthorityAggregator<A>> {
        self.quorum_driver().authority_aggregator().load_full()
    }

    pub fn subscribe_to_effects_queue(&self) -> Receiver<QuorumDriverEffectsQueueResult> {
        if let Some(handler) = &self.quorum_driver_handler {
            handler.subscribe_to_effects()
        } else {
            panic!("QuorumDriverHandler is not initialized, cannot subscribe to effects queue.");
        }
    }

    fn update_metrics(
        &'_ self,
        transaction: &VerifiedTransaction,
    ) -> (impl Drop, &'_ GenericCounter<AtomicU64>) {
        let (in_flight, good_response) = if transaction.contains_shared_object() {
            self.metrics.total_req_received_shared_object.inc();
            (
                self.metrics.req_in_flight_shared_object.clone(),
                &self.metrics.good_response_shared_object,
            )
        } else {
            self.metrics.total_req_received_single_writer.inc();
            (
                self.metrics.req_in_flight_single_writer.clone(),
                &self.metrics.good_response_single_writer,
            )
        };
        in_flight.inc();
        (
            scopeguard::guard(in_flight, |in_flight| {
                in_flight.dec();
            }),
            good_response,
        )
    }

    fn schedule_txes_in_log(
        pending_tx_log: Arc<WritePathPendingTransactionLog>,
        quorum_driver: Arc<QuorumDriverHandler<A>>,
    ) {
        spawn_logged_monitored_task!(async move {
            if std::env::var("SKIP_LOADING_FROM_PENDING_TX_LOG").is_ok() {
                info!("Skipping loading pending transactions from pending_tx_log.");
                return;
            }
            let pending_txes = pending_tx_log
                .load_all_pending_transactions()
                .expect("failed to load all pending transactions");
            info!(
                "Recovering {} pending transactions from pending_tx_log.",
                pending_txes.len()
            );
            for (i, tx) in pending_txes.into_iter().enumerate() {
                // TODO: ideally pending_tx_log would not contain VerifiedTransaction, but that
                // requires a migration.
                let tx = tx.into_inner();
                let tx_digest = *tx.digest();
                // It's not impossible we fail to enqueue a task but that's not the end of
                // world. TODO(william) correctly extract client_addr from logs
                if let Err(err) = quorum_driver
                    .submit_transaction_no_ticket(
                        ExecuteTransactionRequestV1 {
                            transaction: tx,
                            include_events: true,
                            include_input_objects: false,
                            include_output_objects: false,
                            include_auxiliary_data: false,
                        },
                        None,
                    )
                    .await
                {
                    warn!(
                        ?tx_digest,
                        "Failed to enqueue transaction from pending_tx_log, err: {err:?}"
                    );
                } else {
                    debug!(?tx_digest, "Enqueued transaction from pending_tx_log");
                    if (i + 1) % 1000 == 0 {
                        info!("Enqueued {} transactions from pending_tx_log.", i + 1);
                    }
                }
            }
            // Transactions will be cleaned up in
            // loop_execute_finalized_tx_locally() after they
            // produce effects.
        });
    }

    pub fn load_all_pending_transactions(&self) -> IotaResult<Vec<VerifiedTransaction>> {
        self.pending_tx_log.load_all_pending_transactions()
    }
}

/// Convert a `QuorumDriverResponse` (contains
/// `VerifiedCertifiedTransactionEffects`) to the V1 response format that uses
/// `FinalizedEffects`.
fn quorum_driver_response_to_v1(response: QuorumDriverResponse) -> ExecuteTransactionResponseV1 {
    let QuorumDriverResponse {
        effects_cert,
        events,
        input_objects,
        output_objects,
        auxiliary_data,
    } = response;
    ExecuteTransactionResponseV1 {
        effects: FinalizedEffects::new_from_effects_cert(effects_cert.into()),
        events,
        input_objects,
        output_objects,
        auxiliary_data,
    }
}

/// Convert a `transaction_driver_types::FinalizedEffects` into a
/// `quorum_driver_types::FinalizedEffects`.
fn convert_td_to_qd_effects(td: TdFinalizedEffects) -> FinalizedEffects {
    let finality_info = match td.finality_info {
        TdEffectsFinalityInfo::Certified(sig) => EffectsFinalityInfo::Certified(sig),
        TdEffectsFinalityInfo::Checkpointed(epoch, seq) => {
            EffectsFinalityInfo::Checkpointed(epoch, seq)
        }
        TdEffectsFinalityInfo::QuorumExecuted(epoch) => EffectsFinalityInfo::QuorumExecuted(epoch),
    };
    FinalizedEffects {
        effects: td.effects,
        finality_info,
    }
}

/// Map a `TransactionDriverError` to a `QuorumDriverError` for client
/// reporting. The variant choice signals retriability: clients retry on
/// `QuorumDriverInternal`, `FailedWithTransientErrorAfterMaximumAttempts`,
/// and `TimeoutBeforeFinality`, but treat `InvalidTransaction` /
/// `InvalidUserSignature` as terminal. Submission-time rejections that
/// cannot succeed on resubmission must therefore not be reported as
/// internal.
fn map_td_error_to_qd(e: TransactionDriverError) -> QuorumDriverError {
    use TransactionDriverError::*;
    match e {
        ValidationFailed { error } => {
            QuorumDriverError::InvalidUserSignature(IotaError::InvalidSignature { error })
        }
        TimeoutWithLastRetriableError { .. } => QuorumDriverError::TimeoutBeforeFinality,
        RejectedByValidators {
            submission_non_retriable_errors,
            ..
        } => {
            // f+1 stake of validators returned non-retriable errors during
            // submission (bad signature, malformed tx, lock conflict, ...).
            // f+1 means at least one honest validator considered this tx
            // invalid, so resubmitting the same bytes cannot succeed.
            let representative = submission_non_retriable_errors
                .errors
                .into_iter()
                .next()
                .map(|(msg, _, _, _)| msg)
                .unwrap_or_else(|| "transaction rejected as invalid during submission".to_string());
            QuorumDriverError::InvalidTransaction(IotaError::Unknown(format!(
                "Transaction was rejected as invalid by more than 1/3 of validator stake \
                 during submission (non-retriable): {representative}"
            )))
        }
        Aborted {
            submission_retriable_errors,
            submission_non_retriable_errors,
            ..
        } => {
            // Driver exhausted the validator list without reaching the f+1
            // non-retriable threshold — most failures were transient
            // (validator down, network, overload). Surface as retriable so
            // the client can resubmit.
            let attempts = count_validator_attempts(&submission_retriable_errors)
                + count_validator_attempts(&submission_non_retriable_errors);
            QuorumDriverError::FailedWithTransientErrorAfterMaximumAttempts {
                total_attempts: attempts,
            }
        }
        other @ ForkedExecution { .. } => {
            // Validators disagree on effects digests — a protocol-level
            // invariant violation, never a client retry case. Log loud so
            // on-call sees it; surface as internal.
            let msg = other.to_string();
            error!("TransactionDriver observed forked execution: {msg}");
            QuorumDriverError::QuorumDriverInternal(IotaError::Unknown(msg))
        }
        other @ ClientInternal { .. } => {
            let msg = other.to_string();
            warn!("TransactionDriver client-internal error: {msg}");
            QuorumDriverError::QuorumDriverInternal(IotaError::Unknown(msg))
        }
    }
}

fn count_validator_attempts(errors: &AggregatedRequestErrors) -> u32 {
    errors
        .errors
        .iter()
        .map(|(_, authorities, _, _)| authorities.len() as u32)
        .sum()
}

/// Prometheus metrics which can be displayed in Grafana, queried and alerted on
#[derive(Clone)]
pub struct TransactionOrchestratorMetrics {
    total_req_received_single_writer: GenericCounter<AtomicU64>,
    total_req_received_shared_object: GenericCounter<AtomicU64>,

    good_response_single_writer: GenericCounter<AtomicU64>,
    good_response_shared_object: GenericCounter<AtomicU64>,

    req_in_flight_single_writer: GenericGauge<AtomicI64>,
    req_in_flight_shared_object: GenericGauge<AtomicI64>,

    wait_for_finality_in_flight: GenericGauge<AtomicI64>,
    wait_for_finality_finished: GenericCounter<AtomicU64>,
    wait_for_finality_timeout: GenericCounter<AtomicU64>,

    local_execution_in_flight: GenericGauge<AtomicI64>,
    local_execution_success: GenericCounter<AtomicU64>,
    local_execution_timeout: GenericCounter<AtomicU64>,
    local_execution_failure: GenericCounter<AtomicU64>,

    request_latency_single_writer: Histogram,
    request_latency_shared_obj: Histogram,
    wait_for_finality_latency_single_writer: Histogram,
    wait_for_finality_latency_shared_obj: Histogram,
    local_execution_latency_single_writer: Histogram,
    local_execution_latency_shared_obj: Histogram,
}

// Note that labeled-metrics are stored upfront individually
// to mitigate the perf hit by MetricsVec.
// See https://github.com/tikv/rust-prometheus/tree/master/static-metric
impl TransactionOrchestratorMetrics {
    pub fn new(registry: &Registry) -> Self {
        let total_req_received = register_int_counter_vec_with_registry!(
            "tx_orchestrator_total_req_received",
            "Total number of executions request Transaction Orchestrator receives, group by tx type",
            &["tx_type"],
            registry
        )
        .unwrap();

        let total_req_received_single_writer =
            total_req_received.with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]);
        let total_req_received_shared_object =
            total_req_received.with_label_values(&[TX_TYPE_SHARED_OBJ_TX]);

        let good_response = register_int_counter_vec_with_registry!(
            "tx_orchestrator_good_response",
            "Total number of good responses Transaction Orchestrator generates, group by tx type",
            &["tx_type"],
            registry
        )
        .unwrap();

        let good_response_single_writer =
            good_response.with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]);
        let good_response_shared_object = good_response.with_label_values(&[TX_TYPE_SHARED_OBJ_TX]);

        let req_in_flight = register_int_gauge_vec_with_registry!(
            "tx_orchestrator_req_in_flight",
            "Number of requests in flights Transaction Orchestrator processes, group by tx type",
            &["tx_type"],
            registry
        )
        .unwrap();

        let req_in_flight_single_writer =
            req_in_flight.with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]);
        let req_in_flight_shared_object = req_in_flight.with_label_values(&[TX_TYPE_SHARED_OBJ_TX]);

        let request_latency = register_histogram_vec_with_registry!(
            "tx_orchestrator_request_latency",
            "Time spent in processing one Transaction Orchestrator request",
            &["tx_type"],
            iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
            registry,
        )
        .unwrap();
        let wait_for_finality_latency = register_histogram_vec_with_registry!(
            "tx_orchestrator_wait_for_finality_latency",
            "Time spent in waiting for one Transaction Orchestrator request gets finalized",
            &["tx_type"],
            iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
            registry,
        )
        .unwrap();
        let local_execution_latency = register_histogram_vec_with_registry!(
            "tx_orchestrator_local_execution_latency",
            "Time spent in waiting for one Transaction Orchestrator gets locally executed",
            &["tx_type"],
            iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
            registry,
        )
        .unwrap();

        Self {
            total_req_received_single_writer,
            total_req_received_shared_object,
            good_response_single_writer,
            good_response_shared_object,
            req_in_flight_single_writer,
            req_in_flight_shared_object,
            wait_for_finality_in_flight: register_int_gauge_with_registry!(
                "tx_orchestrator_wait_for_finality_in_flight",
                "Number of in flight txns Transaction Orchestrator are waiting for finality for",
                registry,
            )
            .unwrap(),
            wait_for_finality_finished: register_int_counter_with_registry!(
                "tx_orchestrator_wait_for_finality_finished",
                "Total number of txns Transaction Orchestrator gets responses from Quorum Driver before timeout, either success or failure",
                registry,
            )
            .unwrap(),
            wait_for_finality_timeout: register_int_counter_with_registry!(
                "tx_orchestrator_wait_for_finality_timeout",
                "Total number of txns timing out in waiting for finality Transaction Orchestrator handles",
                registry,
            )
            .unwrap(),
            local_execution_in_flight: register_int_gauge_with_registry!(
                "tx_orchestrator_local_execution_in_flight",
                "Number of local execution txns in flights Transaction Orchestrator handles",
                registry,
            )
            .unwrap(),
            local_execution_success: register_int_counter_with_registry!(
                "tx_orchestrator_local_execution_success",
                "Total number of successful local execution txns Transaction Orchestrator handles",
                registry,
            )
            .unwrap(),
            local_execution_timeout: register_int_counter_with_registry!(
                "tx_orchestrator_local_execution_timeout",
                "Total number of timed-out local execution txns Transaction Orchestrator handles",
                registry,
            )
            .unwrap(),
            local_execution_failure: register_int_counter_with_registry!(
                "tx_orchestrator_local_execution_failure",
                "Total number of failed local execution txns Transaction Orchestrator handles",
                registry,
            )
            .unwrap(),
            request_latency_single_writer: request_latency
                .with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]),
            request_latency_shared_obj: request_latency.with_label_values(&[TX_TYPE_SHARED_OBJ_TX]),
            wait_for_finality_latency_single_writer: wait_for_finality_latency
                .with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]),
            wait_for_finality_latency_shared_obj: wait_for_finality_latency
                .with_label_values(&[TX_TYPE_SHARED_OBJ_TX]),
            local_execution_latency_single_writer: local_execution_latency
                .with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]),
            local_execution_latency_shared_obj: local_execution_latency
                .with_label_values(&[TX_TYPE_SHARED_OBJ_TX]),
        }
    }

    pub fn new_for_tests() -> Self {
        let registry = Registry::new();
        Self::new(&registry)
    }
}

#[async_trait::async_trait]
impl<A> iota_types::transaction_executor::TransactionExecutor for TransactionOrchestrator<A>
where
    A: AuthorityAPI + Send + Sync + 'static + Clone,
{
    async fn execute_transaction(
        &self,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        self.execute_transaction_v1(request, client_addr).await
    }

    fn simulate_transaction(
        &self,
        transaction: TransactionData,
        checks: VmChecks,
    ) -> Result<SimulateTransactionResult, IotaError> {
        self.validator_state
            .simulate_transaction(transaction, checks)
    }

    /// Wait for the given transactions to be included in a checkpoint.
    ///
    /// Returns a mapping from transaction digest to
    /// `(checkpoint_sequence_number, checkpoint_timestamp_ms)`.
    /// On timeout, returns partial results for any transactions that were
    /// already checkpointed.
    async fn wait_for_checkpoint_inclusion(
        &self,
        digests: &[TransactionDigest],
        timeout: Duration,
    ) -> Result<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>, IotaError> {
        self.validator_state
            .wait_for_checkpoint_inclusion(digests, timeout)
            .await
    }
}
