// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::future::Either;
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
        SubmitTransactionsRequest, TxStatusUpdate, ValidatorHealthRequest, ValidatorHealthResponse,
    },
    traffic_control::Weight,
    transaction::Transaction,
};
use tokio_stream::wrappers::ReceiverStream;

/// Maximum number of transactions allowed in a single `submit_tx` request.
const MAX_TRANSACTIONS_PER_SUBMIT: usize = 256;

/// Maximum number of queries allowed in a single `get_tx_status` request.
const MAX_QUERIES_PER_GET_TX_STATUS: usize = 256;

/// Timeout for waiting on transaction execution in `get_tx_status`.
const GET_TX_STATUS_TIMEOUT_SECS: u64 = 30;

/// A single streamed item in a V2 RPC response.
type TxUpdateItem = Result<(TransactionDigest, TxStatusUpdate), tonic::Status>;

use iota_metrics::spawn_monitored_task;

use crate::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    authority_server::{StreamResponse, ValidatorService, ValidatorServiceMetrics},
    consensus_adapter::ConsensusAdapter,
};

impl ValidatorService {
    async fn submit_tx_impl(
        &self,
        request: SubmitTransactionsRequest,
    ) -> Result<(ReceiverStream<TxUpdateItem>, Weight), tonic::Status> {
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
            request.transactions.len() <= MAX_TRANSACTIONS_PER_SUBMIT,
            tonic::Status::invalid_argument(format!(
                "too many transactions: {} exceeds limit of {MAX_TRANSACTIONS_PER_SUBMIT}",
                request.transactions.len()
            ))
        );

        let (tx_sender, rx) = tokio::sync::mpsc::channel(request.transactions.len().max(1));
        let consensus_adapter = self.consensus_adapter.clone();
        let metrics = self.metrics.clone();

        // TODO(#11109): cap in-flight work per request (e.g. buffer_unordered(N)).
        for transaction in request.transactions {
            let state = state.clone();
            let epoch_store = epoch_store.clone();
            let consensus_adapter = consensus_adapter.clone();
            let metrics = metrics.clone();
            let tx_sender = tx_sender.clone();
            spawn_monitored_task!(async move {
                let tx_digest = *transaction.digest();
                let result = Self::submit_single_tx(
                    &state,
                    &consensus_adapter,
                    &metrics,
                    &epoch_store,
                    transaction,
                )
                .await;
                let item = match result {
                    Ok(submit_result) => Ok((tx_digest, submit_result)),
                    Err(status) => Err(status),
                };
                // Ignore error: receiver dropped means client disconnected.
                let _ = tx_sender.send(item).await;
            });
        }

        // TODO(#11080): scale traffic weight with batch size.
        Ok((ReceiverStream::new(rx), Weight::one()))
    }

    /// Handles submission of a single transaction. Validates, checks for prior
    /// execution, verifies signature, runs deny checks, and submits to
    /// consensus.
    async fn submit_single_tx(
        state: &Arc<AuthorityState>,
        consensus_adapter: &Arc<ConsensusAdapter>,
        metrics: &Arc<ValidatorServiceMetrics>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        transaction: Transaction,
    ) -> Result<TxStatusUpdate, tonic::Status> {
        let tx_digest = *transaction.digest();

        let build_executed =
            |effects: TransactionEffects| -> Result<TxStatusUpdate, tonic::Status> {
                let effects_digest = effects.digest();
                Ok(TxStatusUpdate::Executed {
                    effects_digest,
                    details: Some(Self::build_executed_data(state, &effects)),
                })
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
            return Ok(TxStatusUpdate::Rejected { error: e });
        }

        // Validate transaction.
        if let Err(e) =
            transaction.validity_check(epoch_store.protocol_config(), epoch_store.epoch())
        {
            return Ok(TxStatusUpdate::Rejected { error: e });
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
            return build_executed(effects);
        }

        // Verify user signature.
        let tx_verif_guard = metrics.tx_verification_latency.start_timer();
        let verified_tx = match epoch_store.verify_transaction(transaction) {
            Ok(verified) => verified,
            Err(e) => {
                metrics.signature_errors.inc();
                return Ok(TxStatusUpdate::Rejected { error: e });
            }
        };
        drop(tx_verif_guard);

        // TODO(#11110): return Rejected instead of Err(tonic::Status) for consistency.
        // Early bail-out during epoch boundary.
        if !epoch_store
            .get_reconfig_state_read_lock_guard()
            .should_accept_user_certs()
        {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            return Err(IotaError::ValidatorHaltedAtEpochEnd.into());
        }

        // TODO(#11110): return Rejected instead of Err(tonic::Status) for consistency.
        // Content validation: deny checks + owned object version validation.
        let owned_objects = state
            .handle_transaction_validation_checks(&verified_tx, epoch_store)
            .await
            .map_err(tonic::Status::from)?;
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
                return build_executed(effects);
            }
            return Err(tonic::Status::from(e));
        }

        // TODO(#11110): return Rejected instead of Err(tonic::Status) for consistency.
        // Reconfig check.
        let reconfiguration_lock = epoch_store.get_reconfig_state_read_lock_guard();
        if !reconfiguration_lock.should_accept_user_certs() {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            return Err(IotaError::ValidatorHaltedAtEpochEnd.into());
        }

        // Submit to consensus.
        consensus_adapter
            .submit(
                ConsensusTransaction::new_user_transaction(verified_tx.into_inner()),
                Some(&reconfiguration_lock),
                epoch_store,
            )
            .map_err(tonic::Status::from)?;

        Ok(TxStatusUpdate::Submitted)
    }

    /// Waits for one or more previously submitted transactions to reach
    /// finality and streams a terminal status per digest (Executed, Rejected,
    /// or Expired).
    async fn get_tx_status_impl(
        &self,
        request: DomainGetTxStatusRequest,
    ) -> Result<(ReceiverStream<TxUpdateItem>, Weight), tonic::Status> {
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

        // Empty queries is a valid no-op/ping (consistent with V1's
        // SubmitTransactionsRequest and WaitForEffectsRequest).
        fp_ensure!(
            request.queries.len() <= MAX_QUERIES_PER_GET_TX_STATUS,
            tonic::Status::invalid_argument(format!(
                "too many queries: {} exceeds limit of {MAX_QUERIES_PER_GET_TX_STATUS}",
                request.queries.len()
            ))
        );

        // TODO(#11111): epoch_store is captured once and used for the full 30s wait.
        // If an epoch change occurs mid-wait, notify_read_dropped_digests
        // watches the old epoch's cache and the timeout reports a stale epoch.
        // This matches V1's handle_wait_for_effect_impl behavior. Client retry
        // after timeout gets a fresh epoch store. Consider listening for epoch
        // change to return Expired early if cross-epoch awareness is needed.
        let (tx_sender, rx) = tokio::sync::mpsc::channel(request.queries.len().max(1));

        for query in request.queries {
            let state = state.clone();
            let epoch_store = epoch_store.clone();
            let tx_sender = tx_sender.clone();
            spawn_monitored_task!(async move {
                let update = Self::wait_for_tx_finality(
                    &state,
                    &epoch_store,
                    query.transaction_digest,
                    query.include_details,
                )
                .await;
                let _ = tx_sender.send(Ok((query.transaction_digest, update))).await;
            });
        }

        Ok((ReceiverStream::new(rx), Weight::one()))
    }

    /// Waits for a single transaction to reach finality. Returns immediately
    /// if already executed, otherwise blocks up to the timeout.
    async fn wait_for_tx_finality(
        state: &Arc<AuthorityState>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        tx_digest: TransactionDigest,
        include_details: bool,
    ) -> TxStatusUpdate {
        // Fast path: already executed.
        let cache = state.get_transaction_cache_reader();
        match cache.try_get_executed_effects(&tx_digest) {
            Ok(Some(effects)) => {
                return Self::build_executed_update(state, effects, include_details);
            }
            Err(e) => {
                tracing::warn!(?tx_digest, "failed to read effects cache: {e}");
                // Fall through to the wait path.
            }
            Ok(None) => {}
        }

        // Wait for execution, rejection, or timeout.
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(GET_TX_STATUS_TIMEOUT_SECS),
            async {
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
            },
        )
        .await;

        match result {
            Ok(Either::Left(effects_digests)) => {
                let Some(effects_digest) = effects_digests.into_iter().next() else {
                    tracing::warn!(
                        ?tx_digest,
                        "empty effects from notify_read, returning Expired"
                    );
                    return TxStatusUpdate::Expired {
                        epoch: epoch_store.epoch(),
                    };
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
            Ok(Either::Right(dropped_error)) => TxStatusUpdate::Rejected {
                error: dropped_error,
            },
            Err(_timeout) => TxStatusUpdate::Expired {
                epoch: epoch_store.epoch(),
            },
        }
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
