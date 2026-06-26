// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Transaction Orchestrator is a Node component that utilizes Quorum Driver (or
// optionally TransactionDriver) to submit transactions to validators for
// finality, and proactively executes finalized transactions locally.

use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    net::SocketAddr,
    ops::Deref,
    path::Path,
    sync::Arc,
    time::Duration,
};

use futures::{
    FutureExt,
    future::{Either, Future, select},
};
use iota_common::{debug_fatal, sync::notify_read::NotifyRead};
use iota_config::NodeConfig;
use iota_metrics::{
    TX_TYPE_SHARED_OBJ_TX, TX_TYPE_SINGLE_WRITER_TX, add_server_timing,
    spawn_logged_monitored_task, spawn_monitored_task,
};
use iota_sdk_types::{Transaction, TransactionDigest};
use iota_storage::write_path_pending_tx_log::WritePathPendingTransactionLog;
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult},
    iota_system_state::IotaSystemState,
    messages_checkpoint::CheckpointSequenceNumber,
    quorum_driver_types::{
        EffectsFinalityInfo, ExecuteTransactionRequestType, ExecuteTransactionRequestV1,
        ExecuteTransactionResponseV1, FinalizedEffects, IsTransactionExecutedLocally,
        QuorumDriverEffectsQueueResult, QuorumDriverError, QuorumDriverResponse,
        QuorumDriverResult,
    },
    transaction::{SenderSignedTransactionAPI, VerifiedTransaction},
    transaction_driver_types::{
        EffectsFinalityInfo as TdEffectsFinalityInfo, FinalizedEffects as TdFinalizedEffects,
    },
    transaction_executor::{SimulateTransactionResult, VmChecks},
};
use parking_lot::Mutex;
use prometheus_filtered::{
    Histogram, MetricLevel, Registry,
    core::{AtomicI64, AtomicU64, GenericCounter, GenericGauge},
    register_histogram_vec_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry, register_int_gauge_vec_with_registry,
    register_int_gauge_with_registry,
};
use tokio::{
    sync::{
        broadcast::{Receiver, error::RecvError},
        watch,
    },
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

/// The submission flow used to drive transactions to finality. Exactly one
/// flow is active, selected by the P-COOL protocol flag at construction
/// time.
enum Driver<A: Clone> {
    /// Certificate-based flow (P-COOL disabled).
    Quorum(Arc<QuorumDriverHandler<A>>),
    /// Direct-to-consensus P-COOL flow.
    Transaction(Arc<TransactionDriver<A>>),
}

/// Transaction Orchestrator is a Node component that supports both QuorumDriver
/// and TransactionDriver for submitting transactions to validators for
/// finality. It adds inflight deduplication, waiting for local execution,
/// recovery, and epoch change handling.
pub struct TransactionOrchestrator<A: Clone> {
    driver: Driver<A>,
    validator_state: Arc<AuthorityState>,
    _local_executor_handle: Option<JoinHandle<()>>,
    pending_tx_log: Arc<WritePathPendingTransactionLog>,
    /// Digests currently being driven to finality by the TransactionDriver;
    /// used to deduplicate concurrent submissions of the same transaction,
    /// with a channel per digest through which the driving submission
    /// publishes its outcome to concurrent duplicates. Kept in memory only:
    /// the driver path is best-effort, so there is nothing to recover after
    /// a restart. The QuorumDriver path tracks its submissions in
    /// `pending_tx_log` instead.
    in_flight_transactions: InFlightTransactions,
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
        // Check protocol config to determine if P-COOL flow is enabled
        let epoch_store = validator_state.load_epoch_store_one_call_per_task();
        let use_transaction_driver = epoch_store.protocol_config().enable_pcool_flow();

        // Create TransactionDriver reconfig observer only if P-COOL is enabled
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

        // Create QuorumDriver reconfig observer only if P-COOL is NOT enabled
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
        // Check protocol config to determine if P-COOL flow is enabled
        let epoch_store = validator_state.load_epoch_store_one_call_per_task();
        let use_transaction_driver = epoch_store.protocol_config().enable_pcool_flow();

        let notifier = Arc::new(NotifyRead::new());
        let metrics = Arc::new(TransactionOrchestratorMetrics::new(prometheus_registry));
        let pending_tx_log = Arc::new(WritePathPendingTransactionLog::new(
            parent_path.join("fullnode_pending_transactions"),
        ));

        let (driver, _local_executor_handle) = if !use_transaction_driver {
            let qd_metrics = Arc::new(QuorumDriverMetrics::new(prometheus_registry));
            let reconfig_observer = Arc::new(
                reconfig_observer
                    .expect("QuorumDriver reconfig observer required when P-COOL is disabled"),
            );
            let handler = Arc::new(
                QuorumDriverHandlerBuilder::new(validators, qd_metrics)
                    .with_notifier(notifier.clone())
                    .with_reconfig_observer(reconfig_observer)
                    .start(),
            );
            let effects_receiver = handler.subscribe_to_effects();
            let pending_tx_log_clone = pending_tx_log.clone();
            let local_executor_handle = spawn_monitored_task!(async move {
                Self::loop_pending_transaction_log(effects_receiver, pending_tx_log_clone).await;
            });
            // Pending-transaction recovery is QuorumDriver-only; the
            // TransactionDriver goes directly to consensus and tracks no
            // pending certificates.
            Self::schedule_txes_in_log(pending_tx_log.clone(), handler.clone());
            (Driver::Quorum(handler), Some(local_executor_handle))
        } else {
            let td_metrics = Arc::new(TransactionDriverMetrics::new(prometheus_registry));
            let client_metrics = Arc::new(ValidatorClientMetrics::new(prometheus_registry));
            let observer = td_reconfig_observer
                .expect("TransactionDriver reconfig observer required when P-COOL is enabled");
            (
                Driver::Transaction(TransactionDriver::new(
                    validators,
                    Arc::new(observer),
                    td_metrics,
                    node_config,
                    client_metrics,
                )),
                None,
            )
        };

        Self {
            driver,
            validator_state,
            _local_executor_handle,
            pending_tx_log,
            in_flight_transactions: Default::default(),
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

        let transaction = epoch_store
            .verify_transaction(request.transaction.clone())
            .map_err(QuorumDriverError::InvalidUserSignature)?;

        // Captured before `request` moves so the skip-cert reconcile reads
        // caller intent, not whatever the submitter happened to return — a
        // Byzantine submitter could otherwise censor a field by returning
        // `None`.
        let include_events = request.include_events;
        let include_input_objects = request.include_input_objects;
        let include_output_objects = request.include_output_objects;

        let tx_digest = *transaction.digest();

        // A resubmission of an already-executed transaction is answered from
        // the local cache instead of being driven through the validators
        // again.
        if let Some(response) = Self::build_response_from_local_effects(
            &self.validator_state,
            &tx_digest,
            include_events,
            include_input_objects,
            include_output_objects,
        )? {
            self.metrics.early_cached_response.inc();
            debug!(
                ?tx_digest,
                "Returning cached results for already-executed transaction"
            );
            return Ok((response, true));
        }

        // Reject malformed transactions before either driver inspects shared
        // inputs or `MoveAuthenticator`. Runs after the cache lookup so that,
        // as on the upstream flow, a resubmission of an executed transaction
        // gets its cached results even if it no longer passes the current
        // epoch's checks (e.g. its expiration epoch has passed).
        transaction
            .validity_check(&epoch_store.tx_validity_check_context())
            .map_err(QuorumDriverError::InvalidTransaction)?;

        let wait_for_local_execution = matches!(
            request_type,
            ExecuteTransactionRequestType::WaitForLocalExecution
        );
        let (mut response, seq) = match (&self.driver, wait_for_local_execution) {
            (Driver::Transaction(td), true) => {
                let td = td.clone();
                let in_flight_transactions = self.in_flight_transactions.clone();
                let validator_state = self.validator_state.clone();
                let metrics = self.metrics.clone();
                // Detached so a client disconnect (this future dropped) does
                // not cancel a submission that may already be in consensus;
                // the task drives the transaction to finality on its own.
                join_submission_task(spawn_monitored_task!(Self::submit_with_checkpoint_race(
                    td,
                    in_flight_transactions,
                    validator_state,
                    metrics,
                    request,
                    client_addr,
                    tx_digest,
                )))
                .await?
            }
            (Driver::Transaction(td), false) => {
                let td = td.clone();
                let in_flight_transactions = self.in_flight_transactions.clone();
                let validator_state = self.validator_state.clone();
                // Detached for the same reason as above.
                let result = join_submission_task(spawn_monitored_task!(
                    Self::submit_with_transaction_driver(
                        td,
                        in_flight_transactions,
                        validator_state,
                        request,
                        client_addr,
                        false,
                    )
                ))
                .await?;
                (Some(result), None)
            }
            (Driver::Quorum(qd), _) => {
                let qd_resp = self
                    .execute_transaction_impl(
                        qd,
                        &epoch_store,
                        request,
                        transaction.clone(),
                        client_addr,
                    )
                    .await?;
                (Some(quorum_driver_response_to_v1(qd_resp)), None)
            }
        };

        // `needs_cache_rebuild` is derived from finality, not caller intent:
        // the QD fallback path returns `Certified` and a duplicate
        // submission inheriting the outcome of an in-flight certifying
        // submission returns `QuorumExecuted` — neither needs a rebuild —
        // even when the caller asked for `WaitForLocalExecution`, while only
        // the TD skip-cert engine produces `UncertifiedSingleValidator`. The
        // checkpoint sequence comes from `submit_with_checkpoint_race`, which
        // relies on `executed_transactions_to_checkpoint` being written
        // strictly after every tx's effects — so a `Some(seq)` here implies
        // the cache has authoritative effects.
        let needs_cache_rebuild = matches!(
            response.as_ref().map(|r| &r.effects.finality_info),
            None | Some(EffectsFinalityInfo::UncertifiedSingleValidator(_)),
        );

        let executed_locally = if !wait_for_local_execution {
            false
        } else if needs_cache_rebuild {
            let Some(seq) = seq else {
                // Timed out waiting for the tx to land in a local checkpoint.
                // In this branch `response` is either `None` (recovery) or
                // `UncertifiedSingleValidator` (TD skip-cert) — both must
                // surface as `TimeoutBeforeFinality` rather than leaking
                // uncorroborated single-validator effects to the client.
                return Err(QuorumDriverError::TimeoutBeforeFinality);
            };
            match response.as_mut() {
                Some(existing) => Self::reconcile_effects_from_cache(
                    &self.validator_state,
                    tx_digest,
                    seq,
                    include_events,
                    include_input_objects,
                    include_output_objects,
                    existing,
                    &self.metrics,
                )?,
                None => {
                    response = Some(Self::build_response_from_cache(
                        &self.validator_state,
                        tx_digest,
                        seq,
                        include_events,
                        include_input_objects,
                        include_output_objects,
                    )?);
                }
            }
            true
        } else {
            // The response is already 2f+1 certified — from the QuorumDriver,
            // or inherited by a duplicate submission from an in-flight
            // certifying submission — so just confirm local execution
            // finished.
            let ok = Self::wait_for_finalized_tx_executed_locally_with_timeout(
                &self.validator_state,
                &transaction,
                &self.metrics,
            )
            .await
            .is_ok();
            add_server_timing("local_execution");
            ok
        };

        let response = response.expect("response must be populated before return");

        // Safety guard: `UncertifiedSingleValidator` finality carries effects
        // from the single submitting validator only — they MUST NOT reach the
        // client without first being corroborated against the local cache. The
        // reachable paths today all either upgrade finality via
        // `reconcile_effects_from_cache` / `build_response_from_cache`, or
        // branch to `TimeoutBeforeFinality`; this guard is the last-chance
        // fallback for a future refactor that forgets to reconcile. Do not
        // remove as dead code.
        if matches!(
            response.effects.finality_info,
            EffectsFinalityInfo::UncertifiedSingleValidator(_)
        ) {
            debug_fatal!(
                "Uncertified effects (UncertifiedSingleValidator) about to be returned \
                 to the client for tx {:?}",
                response.effects.effects.transaction_digest()
            );
            return Err(QuorumDriverError::QuorumDriverInternal(IotaError::Unknown(
                "internal error: transaction effects not finalized".to_string(),
            )));
        }

        Ok((response, executed_locally))
    }

    /// Replace the response's effects, events, and input/output objects with
    /// the authoritative copies derived from the local cache — the local
    /// checkpoint executor has processed the tx, so the cache has the real
    /// data and the TD-returned (single-validator) copies can be discarded.
    ///
    /// `tx_digest` must be the digest of the caller's original transaction,
    /// not the digest carried in `response.effects.effects` — a byzantine
    /// submitter could set the latter to an unrelated (already-executed) tx
    /// so we'd read unrelated effects from the cache.
    ///
    /// The caller must have obtained `checkpoint_seq` from
    /// `wait_for_checkpoint_inclusion` (not just `get_transaction_checkpoint`),
    /// because that function guarantees both the effects write and the
    /// checkpoint-mapping write have landed — it's the only way to avoid the
    /// race between `notify_read_executed_effects_digests` (fires per-tx) and
    /// `insert_finalized_transactions` (fires per-checkpoint, after the
    /// `CheckpointExecutor` has awaited every tx in that checkpoint).
    ///
    /// Upgrades the finality info to `Checkpointed(epoch, checkpoint_seq)`. A
    /// warning is logged if the TD-returned effects digest diverges from the
    /// cache digest, or the submitter claimed events the cache doesn't have
    /// (byzantine submitter or bug).
    fn reconcile_effects_from_cache(
        validator_state: &Arc<AuthorityState>,
        tx_digest: TransactionDigest,
        checkpoint_seq: CheckpointSequenceNumber,
        include_events: bool,
        include_input_objects: bool,
        include_output_objects: bool,
        response: &mut ExecuteTransactionResponseV1,
        metrics: &TransactionOrchestratorMetrics,
    ) -> Result<(), QuorumDriverError> {
        let rebuilt = Self::build_response_from_cache(
            validator_state,
            tx_digest,
            checkpoint_seq,
            include_events,
            include_input_objects,
            include_output_objects,
        )?;

        let td_digest = response.effects.effects.digest();
        let cache_digest = rebuilt.effects.effects.digest();
        if td_digest != cache_digest {
            warn!(
                ?tx_digest,
                ?td_digest,
                ?cache_digest,
                "reconcile_effects_from_cache: TransactionDriver and local cache disagree \
                 on effects digest — replacing with cache (possible byzantine submitter)"
            );
        }
        if include_events && response.events.is_some() && rebuilt.events.is_none() {
            warn!(
                ?tx_digest,
                "reconcile_effects_from_cache: submitter claimed events but cache has \
                 none — discarding (possible byzantine submitter)"
            );
            metrics.skip_effect_cert_events_cache_miss.inc();
        }
        *response = rebuilt;
        Ok(())
    }

    /// Build a skip-effect-certification response entirely from the local
    /// cache. The caller must have already obtained `checkpoint_seq` via
    /// `wait_for_checkpoint_inclusion`, which is supposed to guarantee both
    /// the effects write and the checkpoint-mapping write have landed. A
    /// missing cache entry here would mean a transient races we observed in
    /// practice; mapped to `TimeoutBeforeFinality` so the client retries
    /// rather than seeing a misleading `QuorumDriverInternal`.
    fn build_response_from_cache(
        validator_state: &Arc<AuthorityState>,
        tx_digest: TransactionDigest,
        checkpoint_seq: CheckpointSequenceNumber,
        include_events: bool,
        include_input_objects: bool,
        include_output_objects: bool,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let cached = read_cached_transaction_data(
            validator_state,
            &tx_digest,
            include_events,
            include_input_objects,
            include_output_objects,
        )
        .map_err(|e| {
            QuorumDriverError::QuorumDriverInternal(IotaError::Unknown(format!(
                "failed to read cached tx data for {tx_digest:?}: {e:?}"
            )))
        })?
        .ok_or_else(|| {
            // Checkpoint inclusion is supposed to guarantee the cache has
            // effects, but we've seen transient misses; surface as a retriable
            // timeout rather than an internal error.
            warn!(
                ?tx_digest,
                "effects missing from cache after checkpoint inclusion — surfacing as \
                 TimeoutBeforeFinality"
            );
            QuorumDriverError::TimeoutBeforeFinality
        })?;
        let iota_types::transaction_executor::CachedTransactionData {
            effects,
            events,
            input_objects,
            output_objects,
        } = cached;

        let epoch = effects.epoch();
        Ok(ExecuteTransactionResponseV1 {
            effects: FinalizedEffects {
                effects,
                finality_info: EffectsFinalityInfo::Checkpointed(epoch, checkpoint_seq),
            },
            events,
            input_objects,
            output_objects,
            auxiliary_data: None,
        })
    }

    /// Build a response from the local cache for a transaction that has
    /// already been executed on this node. Returns `Ok(None)` when the
    /// transaction has not been executed locally. Unlike
    /// `build_response_from_cache`, no checkpoint sequence is required: local
    /// effects only exist for finalized transactions, so the response is
    /// tagged `QuorumExecuted`.
    fn build_response_from_local_effects(
        validator_state: &Arc<AuthorityState>,
        tx_digest: &TransactionDigest,
        include_events: bool,
        include_input_objects: bool,
        include_output_objects: bool,
    ) -> Result<Option<ExecuteTransactionResponseV1>, QuorumDriverError> {
        let Some(cached) = read_cached_transaction_data(
            validator_state,
            tx_digest,
            include_events,
            include_input_objects,
            include_output_objects,
        )
        .map_err(QuorumDriverError::QuorumDriverInternal)?
        else {
            return Ok(None);
        };
        let iota_types::transaction_executor::CachedTransactionData {
            effects,
            events,
            input_objects,
            output_objects,
        } = cached;

        let epoch = effects.epoch();
        Ok(Some(ExecuteTransactionResponseV1 {
            effects: FinalizedEffects {
                effects,
                finality_info: EffectsFinalityInfo::QuorumExecuted(epoch),
            },
            events,
            input_objects,
            output_objects,
            auxiliary_data: None,
        }))
    }

    // Utilize the handle_certificate_v1 validator api to request input/output
    // objects
    #[instrument(name = "tx_orchestrator_execute_transaction_v1", level = "trace", skip_all,
        fields(tx_digest = ?request.transaction.digest()))]
    pub async fn execute_transaction_v1(
        &self,
        request: ExecuteTransactionRequestV1,
        skip_certification: bool,
        client_addr: Option<SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let epoch_store = self.validator_state.load_epoch_store_one_call_per_task();

        let transaction = epoch_store
            .verify_transaction(request.transaction.clone())
            .map_err(QuorumDriverError::InvalidUserSignature)?;
        let tx_digest = *transaction.digest();

        // A resubmission of an already-executed transaction is answered from
        // the local cache instead of being driven through the validators
        // again.
        if let Some(response) = Self::build_response_from_local_effects(
            &self.validator_state,
            &tx_digest,
            request.include_events,
            request.include_input_objects,
            request.include_output_objects,
        )? {
            self.metrics.early_cached_response.inc();
            debug!(
                ?tx_digest,
                "Returning cached results for already-executed transaction"
            );
            return Ok(response);
        }

        // Reject malformed transactions before either driver inspects shared
        // inputs or `MoveAuthenticator`. Runs after the cache lookup so that,
        // as on the upstream flow, a resubmission of an executed transaction
        // gets its cached results even if it no longer passes the current
        // epoch's checks (e.g. its expiration epoch has passed).
        transaction
            .validity_check(&epoch_store.tx_validity_check_context())
            .map_err(QuorumDriverError::InvalidTransaction)?;

        match &self.driver {
            Driver::Transaction(td) => {
                let td = td.clone();
                let in_flight_transactions = self.in_flight_transactions.clone();
                let validator_state = self.validator_state.clone();
                // v1 does not do an internal wait; callers (e.g. the gRPC
                // execution service) are responsible for their own
                // `wait_for_checkpoint_inclusion` when they need it, and will
                // reconcile the response from the cache there.
                //
                // Detached so a client disconnect does not cancel a submission
                // that may already be in consensus.
                join_submission_task(spawn_monitored_task!(Self::submit_with_transaction_driver(
                    td,
                    in_flight_transactions,
                    validator_state,
                    request,
                    client_addr,
                    skip_certification,
                )))
                .await
            }
            Driver::Quorum(qd) => {
                let qd_resp = self
                    .execute_transaction_impl(qd, &epoch_store, request, transaction, client_addr)
                    .await?;
                Ok(quorum_driver_response_to_v1(qd_resp))
            }
        }
    }

    /// Submit on the skip-effect-certification path while concurrently
    /// waiting for local checkpoint inclusion. The race is asymmetric:
    ///
    /// - If the **checkpoint** future resolves first (slow driver, e.g. stuck
    ///   corroborating a Byzantine validator's rejection), the driver future is
    ///   dropped and the caller rebuilds the response from the local cache.
    /// - If the **driver** returns first, its result is taken and the
    ///   checkpoint future is awaited to completion (up to the shared
    ///   `WAIT_FOR_FINALITY_TIMEOUT`) before returning, so the caller has a
    ///   checkpoint sequence to reconcile against.
    ///
    /// Returns `(response, seq)` where `response` is `Some` when the driver
    /// returned a result (which may carry `UncertifiedSingleValidator`
    /// finality requiring rebuild) and `seq` is the checkpoint sequence if
    /// either future yielded it.
    ///
    /// Run inside a detached task so a client disconnect cannot cancel the
    /// race before the checkpoint-sequence bookkeeping completes.
    #[instrument(name = "tx_orchestrator_submit_with_checkpoint_race", level = "trace", skip_all,
        fields(tx_digest = ?tx_digest))]
    async fn submit_with_checkpoint_race(
        td: Arc<TransactionDriver<A>>,
        in_flight_transactions: InFlightTransactions,
        validator_state: Arc<AuthorityState>,
        metrics: Arc<TransactionOrchestratorMetrics>,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<SocketAddr>,
        tx_digest: TransactionDigest,
    ) -> Result<
        (
            Option<ExecuteTransactionResponseV1>,
            Option<CheckpointSequenceNumber>,
        ),
        QuorumDriverError,
    > {
        let digests = [tx_digest];
        let checkpoint_inclusion =
            validator_state.wait_for_checkpoint_inclusion(&digests, WAIT_FOR_FINALITY_TIMEOUT);
        tokio::pin!(checkpoint_inclusion);
        let driver = Self::submit_with_transaction_driver(
            td,
            in_flight_transactions,
            validator_state.clone(),
            request,
            client_addr,
            true,
        );

        let seq_for_tx = |inclusion_map: BTreeMap<_, (CheckpointSequenceNumber, _)>| {
            inclusion_map.get(&tx_digest).map(|&(seq, _)| seq)
        };

        let result = tokio::select! {
            biased;
            // `SubmittedButFetchFailed` is retriable (`ErrorCategory::Unavailable`)
            // so the driver's outer loop reissues submission internally and
            // only returns here as `Ok`, `TimeoutWithLastRetriableError`, or
            // a non-retriable error like `RejectedByValidators`.
            driver_result = driver => {
                let response = Some(driver_result?);
                let seq = (&mut checkpoint_inclusion).await.ok().and_then(seq_for_tx);
                (response, seq)
            }
            checkpoint_result = &mut checkpoint_inclusion => {
                metrics.skip_effect_cert_checkpoint_overrode_driver.inc();
                // Dropping the cancelled driver closes the in-flight outcome
                // channel; duplicate submissions fall back to waiting for
                // checkpoint inclusion, which this race winning guarantees
                // resolves immediately.
                let seq = checkpoint_result.ok().and_then(seq_for_tx);
                (None, seq)
            }
        };
        add_server_timing("local_execution");
        Ok(result)
    }

    /// Submit a transaction via the TransactionDriver (P-COOL flow).
    ///
    /// With `skip_certification = true` the driver may return
    /// `UncertifiedSingleValidator` effects without a 2f+1 broadcast. The
    /// caller (gRPC `execute_transactions` or `execute_transaction_block`)
    /// is then responsible for `wait_for_checkpoint_inclusion` and the
    /// cache-rebuild gate that replaces those single-validator effects with
    /// authoritative data — uncertified data must never reach the client.
    /// See `corroborate_single_validator_error` for the per-submission
    /// fetch-failure recovery flow inside the driver.
    ///
    /// Run inside a detached task so a client disconnect cannot cancel a
    /// `drive_transaction` call that may already be in consensus.
    #[instrument(name = "tx_orchestrator_submit_with_td", level = "trace", skip_all,
        fields(tx_digest = ?request.transaction.digest()))]
    async fn submit_with_transaction_driver(
        td: Arc<TransactionDriver<A>>,
        in_flight_transactions: InFlightTransactions,
        validator_state: Arc<AuthorityState>,
        request: ExecuteTransactionRequestV1,
        client_addr: Option<SocketAddr>,
        skip_certification: bool,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let tx_digest = *request.transaction.digest();

        // Deduplicate concurrent submissions of the same digest: only the
        // first caller drives the committee-wide submission and publishes its
        // outcome; the rest await that outcome. The guard removes the digest
        // from the in-flight map on every exit path (success, error, timeout,
        // or cancellation) when it is dropped.
        let guard = match TransactionSubmissionGuard::acquire(in_flight_transactions, tx_digest) {
            TransactionSubmission::Driving(guard) => guard,
            TransactionSubmission::AlreadyInFlight(receiver) => {
                debug!(
                    ?tx_digest,
                    "transaction already in flight; awaiting its outcome instead of driving a \
                     duplicate submission"
                );
                return Self::await_in_flight_transaction(
                    receiver,
                    &td,
                    &validator_state,
                    tx_digest,
                    &request,
                    client_addr,
                    skip_certification,
                )
                .await;
            }
        };

        // This call runs inside a task detached from the caller, so the
        // outcome is logged here rather than left to the caller — a
        // disconnected client's continuation never runs and would
        // otherwise never observe it.
        let td_response = match td
            .drive_transaction(
                Some(request.transaction.clone()),
                SubmitTransactionOptions {
                    forwarded_client_addr: client_addr,
                    ..Default::default()
                },
                Some(WAIT_FOR_FINALITY_TIMEOUT),
                skip_certification,
            )
            .await
        {
            Ok(response) => response,
            Err(e) => {
                warn!(?tx_digest, "TransactionDriver submission failed: {e}");
                let error = map_td_error_to_qd(e);
                guard.publish(Err(error.clone()));
                return Err(error);
            }
        };

        debug!(?tx_digest, "TransactionDriver submission succeeded");

        let td_response = Arc::new(td_response);
        guard.publish(Ok(td_response.clone()));
        // Dropping the guard closes the channel, releasing its copy of the
        // response unless a duplicate submission still holds a receiver — in
        // the common no-duplicate case the response is then moved into the
        // reply instead of cloned.
        drop(guard);
        let td_response = Arc::try_unwrap(td_response).unwrap_or_else(|shared| (*shared).clone());

        Ok(Self::response_from_driver_response(td_response, &request))
    }

    /// Build a caller-specific response from a driver response, honoring the
    /// caller's include flags.
    fn response_from_driver_response(
        td_response: QuorumTransactionResponse,
        request: &ExecuteTransactionRequestV1,
    ) -> ExecuteTransactionResponseV1 {
        let QuorumTransactionResponse {
            effects,
            events,
            input_objects,
            output_objects,
            auxiliary_data,
        } = td_response;
        ExecuteTransactionResponseV1 {
            effects: convert_td_to_qd_effects(effects),
            events: request.include_events.then_some(events).flatten(),
            input_objects: request
                .include_input_objects
                .then_some(input_objects)
                .flatten(),
            output_objects: request
                .include_output_objects
                .then_some(output_objects)
                .flatten(),
            auxiliary_data: request
                .include_auxiliary_data
                .then_some(auxiliary_data)
                .flatten(),
        }
    }

    /// Await the outcome of an already in-flight submission of `tx_digest`
    /// instead of starting a second committee-wide submission for the same
    /// transaction. Resolves to that submission's outcome — running the
    /// effects-certification step first if the outcome does not satisfy this
    /// caller — falls back to waiting for checkpoint inclusion if the
    /// driving submission went away without publishing one (checkpoint-race
    /// cancellation, panic, or shutdown), and returns
    /// `TimeoutBeforeFinality` if nothing is published within
    /// `WAIT_FOR_FINALITY_TIMEOUT` or the follow-up effects certification
    /// does not complete within another `WAIT_FOR_FINALITY_TIMEOUT`.
    async fn await_in_flight_transaction(
        mut receiver: watch::Receiver<Option<InFlightSubmissionResult>>,
        td: &Arc<TransactionDriver<A>>,
        validator_state: &Arc<AuthorityState>,
        tx_digest: TransactionDigest,
        request: &ExecuteTransactionRequestV1,
        client_addr: Option<SocketAddr>,
        skip_certification: bool,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        // The `Ref` returned by `wait_for` is a read guard and must not be
        // held across an await, so the outcome is cloned out before
        // branching. `wait_for` only returns a value matching its predicate,
        // so `Some` is guaranteed on success; a closed channel yields `None`.
        let published = tokio::time::timeout(
            WAIT_FOR_FINALITY_TIMEOUT,
            receiver.wait_for(|outcome| outcome.is_some()),
        )
        .await
        .map_err(|_elapsed| QuorumDriverError::TimeoutBeforeFinality)?
        .ok()
        .and_then(|outcome_ref| outcome_ref.clone());

        let Some(outcome) = published else {
            // Channel closed without an outcome: the driving submission went
            // away without publishing — routinely because its checkpoint
            // race observed the transaction in a local checkpoint and
            // cancelled it, exceptionally on panic or shutdown. Checkpoint
            // inclusion is the remaining signal of the outcome.
            return Self::response_from_checkpoint_inclusion(validator_state, tx_digest, request)
                .await;
        };
        let td_response = outcome?;

        let uncertified = matches!(
            td_response.effects.finality_info,
            TdEffectsFinalityInfo::UncertifiedSingleValidator(_)
        );
        if uncertified && !skip_certification {
            // The in-flight submission already drove the transaction into
            // consensus; only the 2f+1 effects certification is missing for
            // this caller. Certify the effects directly instead of starting
            // a second committee-wide submission or waiting for a checkpoint
            // inclusion the caller never asked for. `certify_transaction` is
            // internally bounded only by committee size times its per-request
            // timeout, so cap it to the same client-facing budget the driving
            // submission gets for its whole `drive_transaction` call.
            let certified = tokio::time::timeout(
                WAIT_FOR_FINALITY_TIMEOUT,
                td.certify_transaction(
                    tx_digest,
                    SubmitTransactionOptions {
                        forwarded_client_addr: client_addr,
                        ..Default::default()
                    },
                ),
            )
            .await
            .map_err(|_elapsed| QuorumDriverError::TimeoutBeforeFinality)?
            .map_err(map_td_error_to_qd)?;
            return Ok(Self::response_from_driver_response(certified, request));
        }

        Ok(Self::response_from_driver_response(
            (*td_response).clone(),
            request,
        ))
    }

    /// Wait for `tx_digest` to reach a local checkpoint and build the
    /// response from the authoritative cache. The fallback outcome signal
    /// for a duplicate whose driving submission went away without
    /// publishing; the result carries `Checkpointed` finality, so it
    /// satisfies every caller. Times out with `TimeoutBeforeFinality` if
    /// the transaction does not get checkpointed in time.
    async fn response_from_checkpoint_inclusion(
        validator_state: &Arc<AuthorityState>,
        tx_digest: TransactionDigest,
        request: &ExecuteTransactionRequestV1,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let digests = [tx_digest];
        // The caller has typically already waited a full timeout on the
        // outcome channel, but this wait is still required: it is what
        // yields the checkpoint sequence and guarantees the checkpoint
        // mapping write has landed (see `reconcile_effects_from_cache`).
        // When the transaction is already checkpointed — the routine reason
        // the fallback fires — it resolves immediately; only after a
        // driving-task death does it actually wait, as the last remaining
        // signal of the outcome.
        let seq = validator_state
            .wait_for_checkpoint_inclusion(&digests, WAIT_FOR_FINALITY_TIMEOUT)
            .await
            .ok()
            .and_then(|inclusion| inclusion.get(&tx_digest).map(|&(seq, _)| seq))
            .ok_or(QuorumDriverError::TimeoutBeforeFinality)?;
        Self::build_response_from_cache(
            validator_state,
            tx_digest,
            seq,
            request.include_events,
            request.include_input_objects,
            request.include_output_objects,
        )
    }

    /// Submit a transaction via the QuorumDriver. `transaction` must be the
    /// signature-verified form of `request.transaction`, and the caller must
    /// have run `validity_check` on it beforehand.
    #[instrument(level = "trace", skip_all, fields(tx_digest = ?request.transaction.digest()))]
    async fn execute_transaction_impl(
        &self,
        quorum_driver: &Arc<QuorumDriverHandler<A>>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        request: ExecuteTransactionRequestV1,
        transaction: VerifiedTransaction,
        client_addr: Option<SocketAddr>,
    ) -> Result<QuorumDriverResponse, QuorumDriverError> {
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
            .submit(
                quorum_driver,
                epoch_store.clone(),
                transaction.clone(),
                request,
                client_addr,
            )
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
                Ok(response)
            }
        }
    }

    /// Submits the transaction to Quorum Driver for execution.
    /// Returns an awaitable Future.
    #[instrument(name = "tx_orchestrator_submit", level = "trace", skip_all)]
    async fn submit(
        &self,
        quorum_driver: &Arc<QuorumDriverHandler<A>>,
        epoch_store: Arc<AuthorityPerEpochStore>,
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
            quorum_driver
                .submit_transaction_no_ticket(request.clone(), client_addr)
                .await?;
        }
        // It's possible that the transaction effects is already stored in DB at this
        // point. So we also subscribe to that. If we hear from `effects_await`
        // first, it means the ticket misses the previous notification, and we
        // want to ask quorum driver to form a certificate for us again, to
        // serve this request.
        let cache_reader = self.validator_state.get_transaction_cache_reader().clone();
        let qd = quorum_driver.clone();
        Ok(async move {
            let digests = [tx_digest];
            let effects_await =
                epoch_store.within_alive_epoch(cache_reader.try_notify_read_executed_effects(
                    "TransactionOrchestrator::notify_read_submit_with_qd",
                    &digests,
                ));
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

    #[instrument(
        name = "tx_orchestrator_wait_for_finalized_tx_executed_locally_with_timeout",
        level = "debug",
        skip_all,
        fields(tx_digest = ?transaction.digest()),
        err
    )]
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
                .try_notify_read_executed_effects_digests(
                    "TransactionOrchestrator::notify_read_wait_for_local_execution",
                    &[tx_digest],
                ),
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

    /// Returns the quorum driver, or `None` under the P-COOL flow.
    pub fn quorum_driver(&self) -> Option<&Arc<QuorumDriverHandler<A>>> {
        match &self.driver {
            Driver::Quorum(handler) => Some(handler),
            Driver::Transaction(_) => None,
        }
    }

    /// Returns the quorum driver, or `None` under the P-COOL flow.
    pub fn clone_quorum_driver(&self) -> Option<Arc<QuorumDriverHandler<A>>> {
        self.quorum_driver().cloned()
    }

    /// Returns the transaction driver, or `None` when the P-COOL flow is
    /// disabled.
    pub fn transaction_driver(&self) -> Option<&Arc<TransactionDriver<A>>> {
        match &self.driver {
            Driver::Quorum(_) => None,
            Driver::Transaction(td) => Some(td),
        }
    }

    /// Returns the authority aggregator of the active driver.
    pub fn clone_authority_aggregator(&self) -> Arc<AuthorityAggregator<A>> {
        match &self.driver {
            Driver::Quorum(qd) => qd.authority_aggregator().load_full(),
            Driver::Transaction(td) => td.authority_aggregator().load_full(),
        }
    }

    /// Returns `None` under the P-COOL flow, which has no effects broadcast.
    pub fn subscribe_to_effects_queue(&self) -> Option<Receiver<QuorumDriverEffectsQueueResult>> {
        self.quorum_driver().map(|qd| qd.subscribe_to_effects())
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

    /// Reports whether a driver submission of `tx_digest` is in flight, and
    /// if so how many duplicate submissions are awaiting its outcome.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn in_flight_duplicates_for_testing(&self, tx_digest: &TransactionDigest) -> Option<usize> {
        self.in_flight_transactions
            .lock()
            .get(tx_digest)
            .map(|sender| sender.receiver_count())
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
        TdEffectsFinalityInfo::UncertifiedSingleValidator(epoch) => {
            EffectsFinalityInfo::UncertifiedSingleValidator(epoch)
        }
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
        other @ SubmittedButFetchFailed { .. } => {
            let msg = other.to_string();
            warn!("TransactionDriver submitted transaction but failed to fetch effects: {msg}");
            QuorumDriverError::QuorumDriverInternal(IotaError::Unknown(msg))
        }
        Congested {
            suggested_gas_price,
        } => {
            // Execution-worker congestion: the submitted bytes would be shed
            // again at the same price, so this is non-recoverable as-is. The
            // structured `ValidatorTransactionCongested` error carries the
            // suggested gas price for the client to resubmit a new transaction.
            QuorumDriverError::NonRecoverableTransactionError {
                errors: vec![(
                    IotaError::ValidatorTransactionCongested {
                        suggested_gas_price,
                    },
                    0,
                    vec![],
                )],
            }
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

/// Await a detached submission task, surfacing a task panic as an internal
/// error.
async fn join_submission_task<T>(
    handle: tokio::task::JoinHandle<Result<T, QuorumDriverError>>,
) -> Result<T, QuorumDriverError> {
    handle.await.unwrap_or_else(|e| {
        Err(QuorumDriverError::QuorumDriverInternal(IotaError::Unknown(
            format!("transaction submission task panicked: {e}"),
        )))
    })
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

    early_cached_response: GenericCounter<AtomicU64>,

    // Bumped when the skip-effect-certification path reconciles against the
    // local cache but the cache has no events for a tx the single submitter
    // claimed had events. Uncertified events are rejected and the request
    // fails via the safety guard.
    skip_effect_cert_events_cache_miss: GenericCounter<AtomicU64>,

    // Bumped when local checkpoint inclusion completes before the TD
    // skip-effect-certification call returns. Indicates the driver was slow
    // (e.g., corroborating a single-validator rejection) and the checkpoint
    // race cancelled the in-flight driver work in favor of rebuilding from
    // the local cache.
    skip_effect_cert_checkpoint_overrode_driver: GenericCounter<AtomicU64>,

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
            registry;
            MetricLevel::Warn,
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
            registry;
            MetricLevel::Warn,
        )
        .unwrap();

        let good_response_single_writer =
            good_response.with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]);
        let good_response_shared_object = good_response.with_label_values(&[TX_TYPE_SHARED_OBJ_TX]);

        let req_in_flight = register_int_gauge_vec_with_registry!(
            "tx_orchestrator_req_in_flight",
            "Number of requests in flights Transaction Orchestrator processes, group by tx type",
            &["tx_type"],
            registry;
            MetricLevel::Warn,
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
            registry;
            MetricLevel::Warn,
        )
        .unwrap();
        let wait_for_finality_latency = register_histogram_vec_with_registry!(
            "tx_orchestrator_wait_for_finality_latency",
            "Time spent in waiting for one Transaction Orchestrator request gets finalized",
            &["tx_type"],
            iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
            registry;
            MetricLevel::Warn,
        )
        .unwrap();
        let local_execution_latency = register_histogram_vec_with_registry!(
            "tx_orchestrator_local_execution_latency",
            "Time spent in waiting for one Transaction Orchestrator gets locally executed",
            &["tx_type"],
            iota_metrics::COARSE_LATENCY_SEC_BUCKETS.to_vec(),
            registry;
            MetricLevel::Warn,
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
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            wait_for_finality_finished: register_int_counter_with_registry!(
                "tx_orchestrator_wait_for_finality_finished",
                "Total number of txns Transaction Orchestrator gets responses from Quorum Driver before timeout, either success or failure",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            wait_for_finality_timeout: register_int_counter_with_registry!(
                "tx_orchestrator_wait_for_finality_timeout",
                "Total number of txns timing out in waiting for finality Transaction Orchestrator handles",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            local_execution_in_flight: register_int_gauge_with_registry!(
                "tx_orchestrator_local_execution_in_flight",
                "Number of local execution txns in flights Transaction Orchestrator handles",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            local_execution_success: register_int_counter_with_registry!(
                "tx_orchestrator_local_execution_success",
                "Total number of successful local execution txns Transaction Orchestrator handles",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            local_execution_timeout: register_int_counter_with_registry!(
                "tx_orchestrator_local_execution_timeout",
                "Total number of timed-out local execution txns Transaction Orchestrator handles",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            local_execution_failure: register_int_counter_with_registry!(
                "tx_orchestrator_local_execution_failure",
                "Total number of failed local execution txns Transaction Orchestrator handles",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            early_cached_response: register_int_counter_with_registry!(
                "tx_orchestrator_early_cached_response",
                "Total number of requests returning cached results for already-executed transactions",
                registry,
            )
                .unwrap(),
            skip_effect_cert_events_cache_miss: register_int_counter_with_registry!(
                "tx_orchestrator_skip_effect_cert_events_cache_miss",
                "Number of skip-effect-certification responses rejected because the \
                 single submitter claimed to have events but the local cache did not \
                 corroborate them",
                registry,
            )
                .unwrap(),
            skip_effect_cert_checkpoint_overrode_driver: register_int_counter_with_registry!(
                "tx_orchestrator_skip_effect_cert_checkpoint_overrode_driver",
                "Number of skip-effect-certification requests where local checkpoint \
                 inclusion completed before the TransactionDriver call returned; the \
                 driver future was cancelled and the response was rebuilt from cache",
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
        skip_certification: bool,
        client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        self.execute_transaction_v1(request, skip_certification, client_addr)
            .await
    }

    fn simulate_transaction(
        &self,
        transaction: Transaction,
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

    fn read_transaction_from_cache(
        &self,
        digest: &TransactionDigest,
        include_events: bool,
        include_input_objects: bool,
        include_output_objects: bool,
    ) -> Result<Option<iota_types::transaction_executor::CachedTransactionData>, IotaError> {
        read_cached_transaction_data(
            &self.validator_state,
            digest,
            include_events,
            include_input_objects,
            include_output_objects,
        )
    }
}

/// Read a transaction's authoritative data from the local cache. Returns
/// `Ok(None)` if the tx hasn't been executed locally yet. Shared by the
/// orchestrator's skip-cert response builder and the `TransactionExecutor`
/// trait method consumed by the gRPC handler.
fn read_cached_transaction_data(
    validator_state: &Arc<AuthorityState>,
    digest: &TransactionDigest,
    include_events: bool,
    include_input_objects: bool,
    include_output_objects: bool,
) -> Result<Option<iota_types::transaction_executor::CachedTransactionData>, IotaError> {
    let cache = validator_state.get_transaction_cache_reader();
    let Some(effects) = cache.try_get_executed_effects(digest)? else {
        return Ok(None);
    };

    let events = if include_events && effects.events_digest().is_some() {
        Some(validator_state.get_transaction_events(digest)?)
    } else {
        None
    };

    let input_objects = if include_input_objects {
        Some(
            validator_state
                .get_transaction_input_objects(&effects)
                .map_err(|e| IotaError::Unknown(format!("input objects: {e:?}")))?,
        )
    } else {
        None
    };
    let output_objects = if include_output_objects {
        Some(
            validator_state
                .get_transaction_output_objects(&effects)
                .map_err(|e| IotaError::Unknown(format!("output objects: {e:?}")))?,
        )
    } else {
        None
    };

    Ok(Some(
        iota_types::transaction_executor::CachedTransactionData {
            effects,
            events,
            input_objects,
            output_objects,
        },
    ))
}

/// Successful outcome of an in-flight driver submission: the unfiltered
/// driver response, or the local checkpoint the transaction was observed in
/// when the checkpoint race cancelled the driver (the cache is then the
/// authoritative source of the effects).
/// Outcome of an in-flight driver submission, shared with concurrent
/// submissions of the same digest.
type InFlightSubmissionResult = Result<Arc<QuorumTransactionResponse>, QuorumDriverError>;

/// Digests currently being driven to finality by the TransactionDriver,
/// each with a channel through which the driving submission publishes its
/// outcome to concurrent duplicates.
type InFlightTransactions =
    Arc<Mutex<HashMap<TransactionDigest, watch::Sender<Option<InFlightSubmissionResult>>>>>;

/// Result of trying to register a submission of a digest in the in-flight
/// map: either this caller drives the committee-wide submission, or another
/// submission of the same digest is already in flight and this caller should
/// await its published outcome instead.
enum TransactionSubmission {
    Driving(TransactionSubmissionGuard),
    AlreadyInFlight(watch::Receiver<Option<InFlightSubmissionResult>>),
}

/// Tracks a transaction that is being submitted to finality so that
/// concurrent submissions of the same digest deduplicate.
///
/// Held only by the driving submission, which must `publish` its outcome so
/// concurrent duplicates can return it. Dropping the guard removes the
/// digest from the in-flight map on every exit path (success, error,
/// timeout, and cancellation); receivers subscribed before removal still
/// observe a published outcome, and if the entry is removed without any
/// outcome (checkpoint-race cancellation, panic, or shutdown) the closed
/// channel tells duplicates to fall back to checkpoint inclusion.
struct TransactionSubmissionGuard {
    in_flight_transactions: InFlightTransactions,
    tx_digest: TransactionDigest,
}

impl TransactionSubmissionGuard {
    fn acquire(
        in_flight_transactions: InFlightTransactions,
        tx_digest: TransactionDigest,
    ) -> TransactionSubmission {
        {
            let mut in_flight = in_flight_transactions.lock();
            match in_flight.entry(tx_digest) {
                Entry::Occupied(entry) => {
                    return TransactionSubmission::AlreadyInFlight(entry.get().subscribe());
                }
                Entry::Vacant(entry) => {
                    let (sender, _initial_receiver) = watch::channel(None);
                    entry.insert(sender);
                    debug!(?tx_digest, "added transaction to in-flight map");
                }
            }
        }
        TransactionSubmission::Driving(Self {
            in_flight_transactions,
            tx_digest,
        })
    }

    /// Publish the submission outcome to concurrent duplicate submissions.
    /// The outcome is stored in the channel even when nobody is subscribed
    /// yet, so a duplicate that subscribes after this call but before the
    /// entry is removed still reads it instead of a closed channel.
    fn publish(&self, result: InFlightSubmissionResult) {
        if let Some(sender) = self.in_flight_transactions.lock().get(&self.tx_digest) {
            sender.send_replace(Some(result));
        }
    }
}

impl Drop for TransactionSubmissionGuard {
    fn drop(&mut self) {
        self.in_flight_transactions.lock().remove(&self.tx_digest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acquire_driving(
        in_flight: &InFlightTransactions,
        tx_digest: TransactionDigest,
    ) -> TransactionSubmissionGuard {
        match TransactionSubmissionGuard::acquire(in_flight.clone(), tx_digest) {
            TransactionSubmission::Driving(guard) => guard,
            TransactionSubmission::AlreadyInFlight(_) => {
                panic!("expected to acquire the driving submission")
            }
        }
    }

    fn acquire_duplicate(
        in_flight: &InFlightTransactions,
        tx_digest: TransactionDigest,
    ) -> watch::Receiver<Option<InFlightSubmissionResult>> {
        match TransactionSubmissionGuard::acquire(in_flight.clone(), tx_digest) {
            TransactionSubmission::Driving(_) => {
                panic!("expected the digest to already be in flight")
            }
            TransactionSubmission::AlreadyInFlight(receiver) => receiver,
        }
    }

    #[tokio::test]
    async fn duplicate_submission_receives_published_outcome() {
        let in_flight = InFlightTransactions::default();
        let tx_digest = TransactionDigest::random();

        let guard = acquire_driving(&in_flight, tx_digest);
        let mut receiver = acquire_duplicate(&in_flight, tx_digest);

        guard.publish(Err(QuorumDriverError::TimeoutBeforeFinality));
        drop(guard);

        // The published outcome must survive the guard drop for receivers
        // subscribed before the entry was removed.
        let outcome = receiver
            .wait_for(|outcome| outcome.is_some())
            .await
            .expect("outcome was published before the sender dropped")
            .clone()
            .expect("wait_for only returns once the outcome is Some");
        assert!(matches!(
            outcome,
            Err(QuorumDriverError::TimeoutBeforeFinality)
        ));
        assert!(
            in_flight.lock().is_empty(),
            "guard drop must remove the in-flight entry"
        );
    }

    #[tokio::test]
    async fn duplicate_subscribing_after_publish_receives_outcome() {
        let in_flight = InFlightTransactions::default();
        let tx_digest = TransactionDigest::random();

        let guard = acquire_driving(&in_flight, tx_digest);
        guard.publish(Err(QuorumDriverError::TimeoutBeforeFinality));

        // Subscribing between the publish and the entry removal must still
        // resolve to the outcome; falling back to checkpoint inclusion here
        // would cost the duplicate a full finality timeout.
        let mut receiver = acquire_duplicate(&in_flight, tx_digest);
        drop(guard);

        let outcome = receiver
            .wait_for(|outcome| outcome.is_some())
            .await
            .expect("the outcome is stored in the channel regardless of subscribers")
            .clone()
            .expect("wait_for only returns once the outcome is Some");
        assert!(matches!(
            outcome,
            Err(QuorumDriverError::TimeoutBeforeFinality)
        ));
    }

    #[tokio::test]
    async fn dropped_guard_without_outcome_closes_channel() {
        let in_flight = InFlightTransactions::default();
        let tx_digest = TransactionDigest::random();

        let guard = acquire_driving(&in_flight, tx_digest);
        let mut receiver = acquire_duplicate(&in_flight, tx_digest);
        drop(guard);

        receiver
            .wait_for(|outcome| outcome.is_some())
            .await
            .expect_err("dropping the guard without publishing must close the channel");
        assert!(in_flight.lock().is_empty());

        // The digest can be driven again once the entry is gone.
        let _guard = acquire_driving(&in_flight, tx_digest);
    }
}
