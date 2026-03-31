// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_network::{
    api::{SubmitTxRequest, TxDigest, TxStatus, ValidatorV2},
    tonic::{Request, Response, Status},
};
use iota_types::{
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaError,
    fp_ensure,
    message_envelope::Message,
    messages_consensus::ConsensusTransaction,
    messages_grpc::{ExecutedData, SubmitTransactionResult, SubmitTransactionsRequest},
    traffic_control::Weight,
    transaction::Transaction,
};
use tokio_stream::wrappers::ReceiverStream;

/// Maximum number of transactions allowed in a single `submit_tx` request.
const MAX_TRANSACTIONS_PER_SUBMIT: usize = 256;

use crate::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    authority_server::{StreamResponse, ValidatorService, ValidatorServiceMetrics},
    consensus_adapter::ConsensusAdapter,
};

impl ValidatorService {
    async fn submit_tx_impl(
        &self,
        request: SubmitTransactionsRequest,
    ) -> Result<
        (
            ReceiverStream<Result<(TransactionDigest, SubmitTransactionResult), Status>>,
            Weight,
        ),
        Status,
    > {
        let state = self.state.clone();
        let epoch_store = state.load_epoch_store_one_call_per_task();

        fp_ensure!(
            !state.is_fullnode(&epoch_store),
            IotaError::FullNodeCantHandleSubmitTransactions.into()
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
            Status::invalid_argument(format!(
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
            tokio::spawn(async move {
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
    ) -> Result<SubmitTransactionResult, Status> {
        let tx_digest = *transaction.digest();

        let build_executed =
            |effects: TransactionEffects| -> Result<SubmitTransactionResult, Status> {
                let effects_digest = effects.digest();
                let events = if effects.events_digest().is_some() {
                    state
                        .get_transaction_events(effects.transaction_digest())
                        .ok()
                } else {
                    None
                };
                let input_objects = state.get_transaction_input_objects(&effects).ok();
                let output_objects = state.get_transaction_output_objects(&effects).ok();
                Ok(SubmitTransactionResult::Executed {
                    effects_digest,
                    details: Box::new(ExecutedData {
                        effects,
                        events,
                        input_objects: input_objects.unwrap_or_default(),
                        output_objects: output_objects.unwrap_or_default(),
                    }),
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
            return Ok(SubmitTransactionResult::Rejected { error: e });
        }

        // Validate transaction.
        if let Err(e) =
            transaction.validity_check(epoch_store.protocol_config(), epoch_store.epoch())
        {
            return Ok(SubmitTransactionResult::Rejected { error: e });
        }

        // Check if already executed.
        // TODO: The `?` here causes an early error return if the cache read fails.
        // The intent is only to short-circuit when the tx is already executed. A
        // transient cache error should probably not abort submission — consider using
        // `.ok().flatten()` so errors are treated as "not found" and the normal flow
        // continues (consensus handles dedup). The same applies to the second
        // `try_get_executed_effects` call below. V1 has the same pattern, so verify
        // the intended semantics for both code paths.
        if let Some(effects) = state
            .get_transaction_cache_reader()
            .try_get_executed_effects(&tx_digest)?
        {
            return build_executed(effects);
        }

        // Verify user signature.
        let tx_verif_guard = metrics.tx_verification_latency.start_timer();
        let verified_tx = match epoch_store.verify_transaction(transaction) {
            Ok(verified) => verified,
            Err(e) => {
                metrics.signature_errors.inc();
                return Ok(SubmitTransactionResult::Rejected { error: e });
            }
        };
        drop(tx_verif_guard);

        // Early bail-out during epoch boundary.
        if !epoch_store
            .get_reconfig_state_read_lock_guard()
            .should_accept_user_certs()
        {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            return Err(IotaError::ValidatorHaltedAtEpochEnd.into());
        }

        // Content validation: deny checks + owned object version validation.
        let owned_objects = state
            .handle_transaction_validation_checks(&verified_tx, epoch_store)
            .await
            .map_err(Status::from)?;
        if let Err(e) = state
            .get_cache_writer()
            .validate_owned_object_versions(&owned_objects)
        {
            // Edge case: check if executed while being validated.
            if let Some(effects) = state
                .get_transaction_cache_reader()
                .try_get_executed_effects(&tx_digest)?
            {
                return build_executed(effects);
            }
            return Err(Status::from(e));
        }

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
            .map_err(Status::from)?;

        Ok(SubmitTransactionResult::Submitted)
    }
}

#[async_trait::async_trait]
impl ValidatorV2 for ValidatorService {
    type SubmitTxStream = StreamResponse<TxStatus>;

    async fn submit_tx(
        &self,
        request: Request<SubmitTxRequest>,
    ) -> Result<Response<Self::SubmitTxStream>, Status> {
        let (req, ip) = self.pre_handle(request).await?;
        self.post_handle_stream(ip, self.submit_tx_impl(req).await)
    }

    type GetTxStatusStream = ReceiverStream<Result<TxStatus, Status>>;

    async fn get_tx_status(
        &self,
        _request: Request<TxDigest>,
    ) -> Result<Response<Self::GetTxStatusStream>, Status> {
        todo!()
    }
}
