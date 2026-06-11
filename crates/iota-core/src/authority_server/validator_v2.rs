// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::{StreamExt, future::Either, stream};
use iota_network::api::{
    GetTxStatusRequest, HealthCheckRequest, HealthCheckResponse, NotifyCapabilitiesRequest,
    NotifyCapabilitiesResponse, SubmitTxRequest, TxStatus, ValidatorV2,
};
use iota_types::{
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaError,
    fp_ensure,
    messages_consensus::ConsensusTransaction,
    messages_grpc::{
        ExecutedData, GetTxStatusRequest as DomainGetTxStatusRequest,
        HandleCapabilityNotificationRequestV1, HandleCapabilityNotificationResponseV1,
        TxStatusUpdate, ValidatorHealthRequest, ValidatorHealthResponse,
    },
    traffic_control::Weight,
    transaction::Transaction,
};
use tokio_stream::wrappers::ReceiverStream;

/// Maximum number of transactions allowed in a single `submit_tx` request.
/// Sized so that per-item traffic tallies from a single max-batch request
/// stay well under `PolicyConfig::channel_capacity` (default 100), leaving
/// room for concurrent requests before the tally channel overflows.
const MAX_TRANSACTIONS_PER_SUBMIT: usize = 32;

/// Maximum number of queries allowed in a single `get_tx_status` request.
/// Sized to match `MAX_TRANSACTIONS_PER_SUBMIT` for the same tally-channel
/// reason.
const MAX_QUERIES_PER_GET_TX_STATUS: usize = 32;

/// Timeout for waiting on transaction execution in `get_tx_status`.
const GET_TX_STATUS_TIMEOUT_SECS: u64 = 30;

/// Maximum number of `submit_single_tx` futures allowed to run concurrently
/// per `submit_tx` request. Caps pre-consensus work (signature verification,
/// validation, DB reads) and contention on `consensus_adapter`'s submit
/// semaphore.
const MAX_CONCURRENT_SUBMIT_TASKS: usize = 16;

/// A single streamed item in a V2 RPC response. The `Weight` is the per-item
/// spam-policy contribution decided by the producing code path.
type TxUpdateItem = Result<((TransactionDigest, TxStatusUpdate), Weight), tonic::Status>;

use iota_metrics::spawn_monitored_task;

use crate::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    authority_server::{StreamResponse, ValidatorService, ValidatorServiceMetrics, normalize},
    consensus_adapter::ConsensusAdapter,
};

impl ValidatorService {
    async fn submit_tx_impl(
        &self,
        transactions: Vec<Transaction>,
    ) -> Result<ReceiverStream<TxUpdateItem>, tonic::Status> {
        let state = self.state.clone();
        let epoch_store = state.load_epoch_store_one_call_per_task();

        fp_ensure!(
            !state.is_fullnode(&epoch_store),
            IotaError::FullNodeCantHandleValidatorV2.into()
        );

        fp_ensure!(
            epoch_store.protocol_config().enable_white_flag_flow(),
            IotaError::UnsupportedFeature {
                error: "White flag flow is not enabled in this protocol version".to_string()
            }
            .into()
        );

        fp_ensure!(
            transactions.len() <= MAX_TRANSACTIONS_PER_SUBMIT,
            tonic::Status::invalid_argument(format!(
                "too many transactions: {} exceeds limit of {MAX_TRANSACTIONS_PER_SUBMIT}",
                transactions.len()
            ))
        );

        let (tx_sender, rx) = tokio::sync::mpsc::channel(transactions.len().max(1));
        let consensus_adapter = self.consensus_adapter.clone();
        let metrics = self.metrics.clone();

        // Run per-tx tasks concurrently, capped by `MAX_CONCURRENT_SUBMIT_TASKS`.
        // Spawning lets CPU-heavy work run across worker threads; `buffer_unordered`
        // forwards results as tasks complete.
        spawn_monitored_task!(async move {
            let mut in_flight = stream::iter(transactions)
                .map(move |transaction| {
                    let state = state.clone();
                    let epoch_store = epoch_store.clone();
                    let consensus_adapter = consensus_adapter.clone();
                    let metrics = metrics.clone();
                    spawn_monitored_task!(async move {
                        let tx_digest = *transaction.digest();
                        let (update, weight) = Self::submit_single_tx(
                            &state,
                            &consensus_adapter,
                            &metrics,
                            &epoch_store,
                            transaction,
                        )
                        .await;
                        ((tx_digest, update), weight)
                    })
                })
                .buffer_unordered(MAX_CONCURRENT_SUBMIT_TASKS)
                .map(|join_res| {
                    join_res.map_err(|e| {
                        tonic::Status::internal(format!("submit_single_tx task failed: {e}"))
                    })
                });

            while let Some(item) = in_flight.next().await {
                // Stop forwarding on client disconnect; in-flight tasks still
                // run to completion (dropping a JoinHandle doesn't cancel).
                if tx_sender.send(item).await.is_err() {
                    break;
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }

    /// Handles submission of a single transaction. Validates, checks for prior
    /// execution, verifies signature, runs deny checks, and submits to
    /// consensus. Returns the terminal status together with the per-item
    /// traffic weight derived from the outcome: `Weight::one()` for an
    /// already-executed duplicate (spam-like), `normalize(&error)` for a
    /// rejection (signature/epoch errors weigh, others don't), and
    /// `Weight::zero()` for a successful submission.
    async fn submit_single_tx(
        state: &Arc<AuthorityState>,
        consensus_adapter: &Arc<ConsensusAdapter>,
        metrics: &Arc<ValidatorServiceMetrics>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        transaction: Transaction,
    ) -> (TxStatusUpdate, Weight) {
        let tx_digest = *transaction.digest();

        let build_executed = |effects: TransactionEffects| -> TxStatusUpdate {
            let effects_digest = effects.digest();
            TxStatusUpdate::Executed {
                effects_digest,
                details: Some(Self::build_executed_data(state, &effects)),
            }
        };

        // Check system overload.
        if let Err(e) = state.check_system_overload(
            consensus_adapter,
            transaction.data(),
            state.check_system_overload_at_signing(),
        ) {
            metrics
                .num_rejected_tx_during_overload
                .with_label_values(&[e.as_ref()])
                .inc();
            let weight = normalize(&e);
            return (TxStatusUpdate::Rejected { error: e }, weight);
        }

        // Validate transaction.
        if let Err(e) =
            transaction.validity_check(epoch_store.protocol_config(), epoch_store.epoch())
        {
            let weight = normalize(&e);
            return (TxStatusUpdate::Rejected { error: e }, weight);
        }

        // Check if already executed. Transient cache errors are treated as
        // "not found" — consensus handles dedup, so the normal submission
        // flow continues safely.
        if let Some(effects) = state
            .get_transaction_cache_reader()
            .try_get_executed_effects(&tx_digest)
            .ok()
            .flatten()
        {
            return (build_executed(effects), Weight::one());
        }

        // Verify user signature.
        let tx_verif_guard = metrics.tx_verification_latency.start_timer();
        let verified_tx = match epoch_store.verify_transaction(transaction) {
            Ok(verified) => verified,
            Err(e) => {
                metrics.signature_errors.inc();
                let weight = normalize(&e);
                return (TxStatusUpdate::Rejected { error: e }, weight);
            }
        };
        drop(tx_verif_guard);

        // Early bail-out during epoch boundary.
        if !epoch_store
            .get_reconfig_state_read_lock_guard()
            .should_accept_user_certs()
        {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            let error = IotaError::ValidatorHaltedAtEpochEnd;
            let weight = normalize(&error);
            return (TxStatusUpdate::Rejected { error }, weight);
        }

        // Content validation: deny checks + owned object version validation.
        let owned_objects = match state
            .handle_transaction_validation_checks(&verified_tx, epoch_store)
            .await
        {
            Ok(objs) => objs,
            Err(e) => {
                let weight = normalize(&e);
                return (TxStatusUpdate::Rejected { error: e }, weight);
            }
        };
        if let Err(e) = state
            .get_cache_writer()
            .validate_owned_object_versions(&owned_objects)
        {
            // Edge case: check if executed while being validated.
            if let Some(effects) = state
                .get_transaction_cache_reader()
                .try_get_executed_effects(&tx_digest)
                .ok()
                .flatten()
            {
                return (build_executed(effects), Weight::one());
            }
            let weight = normalize(&e);
            return (TxStatusUpdate::Rejected { error: e }, weight);
        }

        // Reconfig check.
        let reconfiguration_lock = epoch_store.get_reconfig_state_read_lock_guard();
        if !reconfiguration_lock.should_accept_user_certs() {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            let error = IotaError::ValidatorHaltedAtEpochEnd;
            let weight = normalize(&error);
            return (TxStatusUpdate::Rejected { error }, weight);
        }

        // Submit to consensus.
        if let Err(e) = consensus_adapter.submit(
            ConsensusTransaction::new_user_transaction(verified_tx.into_inner()),
            Some(&reconfiguration_lock),
            epoch_store,
        ) {
            let weight = normalize(&e);
            return (TxStatusUpdate::Rejected { error: e }, weight);
        }

        (TxStatusUpdate::Submitted, Weight::zero())
    }

    /// Waits for one or more previously submitted transactions to reach
    /// finality and streams a terminal status per digest (Executed, Rejected,
    /// or Expired).
    async fn get_tx_status_impl(
        &self,
        request: DomainGetTxStatusRequest,
    ) -> Result<ReceiverStream<TxUpdateItem>, tonic::Status> {
        let state = self.state.clone();
        let epoch_store = state.load_epoch_store_one_call_per_task();

        fp_ensure!(
            !state.is_fullnode(&epoch_store),
            IotaError::FullNodeCantHandleValidatorV2.into()
        );

        fp_ensure!(
            epoch_store.protocol_config().enable_white_flag_flow(),
            IotaError::UnsupportedFeature {
                error: "White flag flow is not enabled in this protocol version".to_string()
            }
            .into()
        );

        // Empty queries is a valid no-op/ping.
        fp_ensure!(
            request.queries.len() <= MAX_QUERIES_PER_GET_TX_STATUS,
            tonic::Status::invalid_argument(format!(
                "too many queries: {} exceeds limit of {MAX_QUERIES_PER_GET_TX_STATUS}",
                request.queries.len()
            ))
        );

        let (tx_sender, rx) = tokio::sync::mpsc::channel(request.queries.len().max(1));

        for query in request.queries {
            let state = state.clone();
            let epoch_store = epoch_store.clone();
            let tx_sender = tx_sender.clone();
            spawn_monitored_task!(async move {
                let (update, weight) = Self::wait_for_tx_finality(
                    &state,
                    &epoch_store,
                    query.transaction_digest,
                    query.include_details,
                )
                .await;
                let _ = tx_sender
                    .send(Ok(((query.transaction_digest, update), weight)))
                    .await;
            });
        }

        Ok(ReceiverStream::new(rx))
    }

    /// Waits for a single transaction to reach finality. Returns immediately
    /// if already executed, otherwise blocks up to the timeout. The returned
    /// `Weight` is the per-item spam contribution decided by the path taken:
    /// a fast-path cache hit means the client is querying an already-finalized
    /// tx (duplicate/redundant query, `Weight::one()`), while every wait-path
    /// outcome corresponds to legitimate polling for an in-flight tx and is
    /// weighted as `Weight::zero()`.
    async fn wait_for_tx_finality(
        state: &Arc<AuthorityState>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        tx_digest: TransactionDigest,
        include_details: bool,
    ) -> (TxStatusUpdate, Weight) {
        // Fast path: already executed → duplicate query, spam-like.
        let cache = state.get_transaction_cache_reader();
        match cache.try_get_executed_effects(&tx_digest) {
            Ok(Some(effects)) => {
                return (
                    Self::build_executed_update(state, effects, include_details),
                    Weight::one(),
                );
            }
            Err(e) => {
                tracing::warn!(?tx_digest, "failed to read effects cache: {e}");
                // Fall through to the wait path.
            }
            Ok(None) => {}
        }

        // Wait for execution, rejection, epoch end, or timeout. All outcomes
        // below are legitimate polling for an in-flight tx → Weight::zero().
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(GET_TX_STATUS_TIMEOUT_SECS),
            epoch_store.within_alive_epoch(async {
                let digests_to_watch = [tx_digest];
                tokio::select! {
                    biased;
                    effects_digests = cache.notify_read_executed_effects_digests(&digests_to_watch) => {
                        Either::Left(effects_digests)
                    }
                    dropped_error = epoch_store.notify_read_dropped_digests(tx_digest) => {
                        Either::Right(dropped_error)
                    }
                }
            }),
        )
        .await;

        let update = match result {
            // Epoch ended before execution or rejection.
            Ok(Err(())) => TxStatusUpdate::Expired {
                epoch: epoch_store.epoch(),
            },
            Ok(Ok(Either::Left(effects_digests))) => {
                let Some(effects_digest) = effects_digests.into_iter().next() else {
                    tracing::warn!(
                        ?tx_digest,
                        "empty effects from notify_read, returning Expired"
                    );
                    return (
                        TxStatusUpdate::Expired {
                            epoch: epoch_store.epoch(),
                        },
                        Weight::zero(),
                    );
                };
                let details = if include_details {
                    match cache.try_get_executed_effects(&tx_digest) {
                        Ok(Some(effects)) => Some(Self::build_executed_data(state, &effects)),
                        Ok(None) => {
                            tracing::warn!(
                                ?tx_digest,
                                "effects disappeared between notification and fetch, \
                                 returning Executed without details"
                            );
                            None
                        }
                        Err(e) => {
                            tracing::warn!(
                                ?tx_digest,
                                "failed to read effects: {e}, returning Executed without details"
                            );
                            None
                        }
                    }
                } else {
                    None
                };
                TxStatusUpdate::Executed {
                    effects_digest,
                    details,
                }
            }
            Ok(Ok(Either::Right(dropped_error))) => TxStatusUpdate::Rejected {
                error: dropped_error,
            },
            Err(_timeout) => TxStatusUpdate::Expired {
                epoch: epoch_store.epoch(),
            },
        };
        (update, Weight::zero())
    }

    /// Builds a `TxStatusUpdate::Executed` from known effects, optionally
    /// including full details.
    fn build_executed_update(
        state: &Arc<AuthorityState>,
        effects: TransactionEffects,
        include_details: bool,
    ) -> TxStatusUpdate {
        let effects_digest = effects.digest();
        let details = if include_details {
            Some(Self::build_executed_data(state, &effects))
        } else {
            None
        };
        TxStatusUpdate::Executed {
            effects_digest,
            details,
        }
    }

    /// Fetches execution details (events, input/output objects) for a
    /// transaction.
    fn build_executed_data(
        state: &Arc<AuthorityState>,
        effects: &TransactionEffects,
    ) -> Box<ExecutedData> {
        let events = if effects.events_digest().is_some() {
            state
                .get_transaction_events(effects.transaction_digest())
                .ok()
        } else {
            None
        };
        let input_objects = state
            .get_transaction_input_objects(effects)
            .ok()
            .unwrap_or_default();
        let output_objects = state
            .get_transaction_output_objects(effects)
            .ok()
            .unwrap_or_default();
        Box::new(ExecutedData {
            effects: effects.clone(),
            events,
            input_objects,
            output_objects,
        })
    }

    async fn notify_capabilities_impl(
        &self,
        request: HandleCapabilityNotificationRequestV1,
    ) -> Result<(HandleCapabilityNotificationResponseV1, Weight), tonic::Status> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();

        fp_ensure!(
            epoch_store
                .protocol_config()
                .track_non_committee_eligible_validators(),
            IotaError::UnsupportedFeature {
                error: "capability notification endpoint is not supported in this Protocol Version"
                    .to_string()
            }
            .into()
        );

        fp_ensure!(
            !self.state.is_fullnode(&epoch_store),
            IotaError::FullNodeCantHandleValidatorV2.into()
        );

        let existing_capabilities = epoch_store.get_capabilities_v1()?;
        let incoming_capability = request.message.data();

        tracing::info!(
            "Received capability notification: {:?}",
            incoming_capability
        );

        if let Some(existing) = existing_capabilities
            .iter()
            .find(|cap| cap.authority == incoming_capability.authority)
        {
            if incoming_capability.generation <= existing.generation {
                return Ok((
                    HandleCapabilityNotificationResponseV1 { _unused: false },
                    Weight::one(),
                ));
            }
        }

        if let Err(error) = self.consensus_adapter.check_consensus_overload() {
            self.metrics
                .num_rejected_capability_notifications_during_overload
                .with_label_values(&[error.as_ref()])
                .inc();
            return Err(error.into());
        }

        let _metrics_guard = self
            .metrics
            .handle_capability_notification_latency
            .start_timer();

        let signed_authority_capabilities = request.message;
        let verified_authority_capabilities = epoch_store
            .verify_authority_capabilities(signed_authority_capabilities)
            .inspect_err(|_e| {
                self.metrics.signature_errors.inc();
            })?;

        let authority_name = verified_authority_capabilities.authority;
        // Process the verified capabilities
        tracing::debug!("Verified capability notification for authority {authority_name:?}");

        let transaction = ConsensusTransaction::new_signed_capability_notification_v1(
            verified_authority_capabilities.into_inner(),
        );

        self.consensus_adapter
            .submit(transaction, None, &epoch_store)?;

        tracing::debug!(
            "Submitted capability notification to consensus for authority {authority_name:?}"
        );

        Ok((
            HandleCapabilityNotificationResponseV1 { _unused: false },
            Weight::one(),
        ))
    }

    fn health_check_impl(
        &self,
        _request: ValidatorHealthRequest,
    ) -> Result<(ValidatorHealthResponse, Weight), tonic::Status> {
        let epoch_store = self.state.load_epoch_store_one_call_per_task();

        let last_locally_built_checkpoint = epoch_store
            .last_built_checkpoint_summary()
            .map_err(|e| {
                tonic::Status::internal(format!(
                    "Failed to read last built checkpoint summary: {e}"
                ))
            })?
            .map(|(seq, _)| seq)
            .unwrap_or(0);

        Ok((
            ValidatorHealthResponse {
                num_inflight_execution_transactions: self
                    .state
                    .transaction_manager()
                    .inflight_queue_len()
                    as u64,
                num_inflight_consensus_transactions: self
                    .consensus_adapter
                    .num_inflight_transactions(),
                last_locally_built_checkpoint,
            },
            Weight::zero(),
        ))
    }
}

#[async_trait::async_trait]
impl ValidatorV2 for ValidatorService {
    type SubmitTxStream = StreamResponse<TxStatus>;

    async fn submit_tx(
        &self,
        request: tonic::Request<SubmitTxRequest>,
    ) -> Result<tonic::Response<Self::SubmitTxStream>, tonic::Status> {
        let (req, ip) = self.pre_handle(request).await?;
        self.post_handle_stream(ip, self.submit_tx_impl(req).await)
    }

    type GetTxStatusStream = StreamResponse<TxStatus>;

    async fn get_tx_status(
        &self,
        request: tonic::Request<GetTxStatusRequest>,
    ) -> Result<tonic::Response<Self::GetTxStatusStream>, tonic::Status> {
        let (req, ip) = self.pre_handle(request).await?;
        self.post_handle_stream(ip, self.get_tx_status_impl(req).await)
    }

    async fn notify_capabilities(
        &self,
        request: tonic::Request<NotifyCapabilitiesRequest>,
    ) -> Result<tonic::Response<NotifyCapabilitiesResponse>, tonic::Status> {
        let (req, ip) = self.pre_handle(request).await?;
        self.post_handle_unary(ip, self.notify_capabilities_impl(req).await)
    }

    async fn health_check(
        &self,
        request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        let (req, ip) = self.pre_handle(request).await?;
        self.post_handle_unary(ip, self.health_check_impl(req))
    }
}
