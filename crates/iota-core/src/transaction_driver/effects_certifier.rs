// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{StreamExt as _, future::BoxFuture, stream::FuturesUnordered};
use iota_common::{backoff::ExponentialBackoff, debug_fatal};
use iota_types::{
    base_types::{AuthorityName, ConciseableName as _},
    committee::StakeUnit,
    digests::{TransactionDigest, TransactionEffectsDigest},
    effects::TransactionEffectsAPI as _,
    error::{IotaError, IotaResult},
    messages_grpc::{ExecutedData, GetTxStatusRequest, TxStatusQuery, TxStatusUpdate},
    transaction_driver_types::{EffectsFinalityInfo, FinalizedEffects},
};
use tokio::{
    join,
    sync::mpsc::{Receiver, Sender, channel},
    time::{sleep, timeout},
};
use tracing::instrument;

use crate::{
    authority_aggregator::AuthorityAggregator,
    authority_client::AuthorityAPI,
    safe_client::SafeClient,
    status_aggregator::StatusAggregator,
    transaction_driver::{
        QuorumTransactionResponse, SubmitTransactionOptions,
        error::{
            AggregatedEffectsDigests, TransactionDriverError, TransactionRequestError,
            aggregate_request_errors,
        },
        metrics::TransactionDriverMetrics,
        request_retrier::RequestRetrier,
    },
    validator_client_monitor::{OperationFeedback, OperationType, ValidatorClientMonitor},
};

const WAIT_FOR_EFFECTS_TIMEOUT: Duration = Duration::from_secs(10);

const MAX_WAIT_FOR_EFFECTS_RETRY_DELAY: Duration = Duration::from_secs(2);

/// Delay before starting a speculative get_full_effects request to a fallback
/// validator. If the first validator hasn't responded within this time, we
/// start a parallel request to another validator that has already acknowledged
/// the effects.
const GET_FULL_EFFECTS_FALLBACK_DELAY: Duration = Duration::from_millis(200);

fn build_tx_status_request(
    tx_digest: Option<TransactionDigest>,
    include_details: bool,
) -> GetTxStatusRequest {
    match tx_digest {
        Some(transaction_digest) => GetTxStatusRequest {
            queries: vec![TxStatusQuery {
                transaction_digest,
                include_details,
            }],
        },
        None => GetTxStatusRequest { queries: vec![] },
    }
}

/// Result type for get_full_effects requests.
/// The tuple contains (effects_digest, executed_data) where
type FullEffectsResult =
    Result<(TransactionEffectsDigest, Box<ExecutedData>), TransactionRequestError>;

pub(crate) struct EffectsCertifier {
    metrics: Arc<TransactionDriverMetrics>,
}

impl EffectsCertifier {
    pub(crate) fn new(metrics: Arc<TransactionDriverMetrics>) -> Self {
        Self { metrics }
    }

    #[instrument(level = "error", skip_all, err(level = "debug"))]
    pub(crate) async fn get_certified_finalized_effects<A>(
        &self,
        authority_aggregator: &AuthorityAggregator<A>,
        client_monitor: &ValidatorClientMonitor,
        tx_digest: Option<TransactionDigest>,
        // This keeps track of the current target for getting full effects.
        mut current_target: AuthorityName,
        // Expected to be Submitted or Executed; Rejected and Expired are handled
        // as errors inside this function.
        submit_txn_result: TxStatusUpdate,
        options: &SubmitTransactionOptions,
    ) -> Result<QuorumTransactionResponse, TransactionDriverError>
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        // Skip the first attempt to get full effects if it is already provided.
        let full_effects = match submit_txn_result {
            TxStatusUpdate::Submitted => None,
            TxStatusUpdate::Executed {
                effects_digest,
                details,
            } => details.map(|d| (effects_digest, d)),
            TxStatusUpdate::Rejected { error } => {
                return Err(TransactionDriverError::ClientInternal {
                    error: format!(
                        "Unexpected submission error in get_certified_finalized_effects(): {error:?}"
                    ),
                });
            }
            TxStatusUpdate::Expired { epoch } => {
                return Err(TransactionDriverError::ClientInternal {
                    error: format!(
                        "Transaction expired in epoch {epoch} during get_certified_finalized_effects()",
                    ),
                });
            }
        };

        let mut retrier = RequestRetrier::new(authority_aggregator, client_monitor, &[], &[]);

        // Channel for wait_for_acknowledgments to notify which validators have acked.
        // These validators are known to have executed the transaction, making them good
        // fallback candidates for get_full_effects if the initial validator is slow.
        // Bounded by committee size since each validator sends at most one ack.
        let (acked_validators_tx, acked_validators_rx) =
            channel(authority_aggregator.committee.num_members());

        // Setting this to None at first because if the full effects are already
        // provided, we do not need to record the latency. We track the time in
        // this function instead of inside get_full_effects so that we could
        // record differently depending on whether the result is byzantine.
        let mut full_effects_start_time = None;
        let (acknowledgments_result, (mut full_effects_result, returned_target)) = join!(
            self.wait_for_acknowledgments(
                authority_aggregator,
                client_monitor,
                tx_digest,
                options,
                current_target,
                acked_validators_tx,
            ),
            async {
                // No need to send a full effects request if it is already provided.
                if let Some(full_effects) = full_effects {
                    // In this branch, current_target is the authority providing the full effects,
                    // so it is consistent. This is not used though because current_target is
                    // only used with failed full effects query.
                    return (Ok(full_effects), current_target);
                }
                let (name, client) = retrier
                    .next_target()
                    .expect("there should be at least 1 target");
                full_effects_start_time = Some(Instant::now());
                self.get_full_effects_with_fallback(
                    authority_aggregator,
                    client,
                    name,
                    tx_digest,
                    options,
                    acked_validators_rx,
                )
                .await
            },
        );
        current_target = returned_target;

        // If the consensus position got rejected, effects certification will see the
        // failure and gather error messages to explain the rejection.
        let certified_digest = acknowledgments_result?;

        // Retry until there is a valid full effects that matches the certified digest,
        // or all targets have been attempted.
        loop {
            let display_name = authority_aggregator.get_display_name(&current_target);
            let feedback_builder =
                OperationFeedback::builder(current_target, display_name, OperationType::Effects);
            match full_effects_result {
                Ok((effects_digest, executed_data)) => {
                    if effects_digest != certified_digest {
                        tracing::warn!(
                            ?current_target,
                            "Full effects digest mismatch ({} vs certified {})",
                            effects_digest,
                            certified_digest
                        );
                        // This validator is byzantine, record the error and try to get full effects
                        // from another validator.
                        client_monitor.record_interaction_result(feedback_builder.err_now());
                    } else {
                        if let Some(start_time) = full_effects_start_time {
                            let latency = start_time.elapsed();
                            client_monitor
                                .record_interaction_result(feedback_builder.ok_now(latency));
                        }
                        return Ok(
                            self.get_quorum_transaction_response(effects_digest, *executed_data)
                        );
                    }
                }
                Err(e) => {
                    tracing::debug!(?current_target, "Failed to get full effects: {e}");
                    client_monitor.record_interaction_result(feedback_builder.err_now());
                    // This emits an error when retrier gathers enough (f+1) non-retriable effects
                    // errors, but the error should not happen after effects
                    // certification unless there are software bugs
                    // or > f malicious validators.
                    retrier.add_error(current_target, e)?;
                }
            };

            tokio::task::yield_now().await;

            // Retry getting full effects from the next target.

            // This emits an error when retrier has no targets available.
            let (name, client) = retrier.next_target()?;
            current_target = name;
            full_effects_start_time = Some(Instant::now());
            full_effects_result = self.get_full_effects(client, tx_digest, options).await;
        }
    }

    /// Gets effects from a single validator without broadcasting to 2f+1 for
    /// effects digest certification. Intended for callers that rely on local
    /// checkpoint execution for finality, making the 2f+1 certification
    /// broadcast redundant.
    ///
    /// The returned `executed_data.effects.transaction_digest()` is verified
    /// to match `expected_tx_digest`; a mismatch indicates a byzantine
    /// submitter attempting to inject effects for an unrelated transaction
    /// and is rejected.
    #[instrument(level = "debug", skip_all, err(level = "debug"))]
    pub(crate) async fn get_effects_without_certification<A>(
        &self,
        authority_aggregator: &AuthorityAggregator<A>,
        client_monitor: &Arc<ValidatorClientMonitor>,
        tx_digest: Option<TransactionDigest>,
        current_target: AuthorityName,
        submit_txn_result: TxStatusUpdate,
        options: &SubmitTransactionOptions,
    ) -> Result<QuorumTransactionResponse, TransactionDriverError>
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let (effects_digest, executed_data) = match submit_txn_result {
            TxStatusUpdate::Executed {
                effects_digest,
                details: Some(details),
            } => (effects_digest, details),
            TxStatusUpdate::Submitted | TxStatusUpdate::Executed { details: None, .. } => {
                let client = authority_aggregator
                    .authority_clients
                    .get(&current_target)
                    .ok_or_else(|| TransactionDriverError::ClientInternal {
                        error: format!(
                            "Submitting validator {current_target:?} not found in authority clients"
                        ),
                    })?
                    .clone();
                match self.get_full_effects(client, tx_digest, options).await {
                    Ok(details) => details,
                    Err(e) => {
                        return Err(self
                            .corroborate_single_validator_error(
                                authority_aggregator,
                                client_monitor,
                                tx_digest,
                                current_target,
                                e,
                                options,
                            )
                            .await);
                    }
                }
            }
            // `Rejected` and `Expired` are filtered upstream by
            // `drive_transaction_once` before the skip-cert path is entered;
            // hitting them here is a driver-level invariant break, not a
            // user-facing error.
            TxStatusUpdate::Rejected { error } => {
                debug_fatal!(
                    "Rejected status reached get_effects_without_certification: {error:?}"
                );
                return Err(TransactionDriverError::ClientInternal {
                    error: format!("Rejected reached skip-cert path: {error:?}"),
                });
            }
            TxStatusUpdate::Expired { epoch } => {
                debug_fatal!(
                    "Expired status reached get_effects_without_certification at epoch {epoch}"
                );
                return Err(TransactionDriverError::ClientInternal {
                    error: format!("Expired reached skip-cert path at epoch {epoch}"),
                });
            }
        };

        // Guard against a byzantine submitter returning effects for a different
        // transaction. The caller will later key the local-cache reconciliation
        // off the returned effects digest, so letting this slip through would
        // let the attacker swap in effects from an unrelated (already-executed)
        // tx. `tx_digest` is always `Some` on this path (pings never use
        // WaitForLocalExecution), so we assert rather than silently skip.
        let expected_tx_digest = tx_digest.expect(
            "get_effects_without_certification is only invoked for user transactions; \
             tx_digest must be Some",
        );
        let returned_tx_digest = *executed_data.effects.transaction_digest();
        if returned_tx_digest != expected_tx_digest {
            return Err(TransactionDriverError::ClientInternal {
                error: format!(
                    "Submitting validator {current_target:?} returned effects for tx {returned_tx_digest:?} but we expected {expected_tx_digest:?}"
                ),
            });
        }

        self.metrics.executed_transactions.inc();
        tracing::debug!("Transaction executed (uncertified) with effects digest: {effects_digest}");

        let epoch = executed_data.effects.epoch();
        let effects = FinalizedEffects {
            effects: executed_data.effects,
            finality_info: EffectsFinalityInfo::UncertifiedSingleValidator(epoch),
        };

        Ok(QuorumTransactionResponse {
            effects,
            events: executed_data.events,
            input_objects: if !executed_data.input_objects.is_empty() {
                Some(executed_data.input_objects)
            } else {
                None
            },
            output_objects: if !executed_data.output_objects.is_empty() {
                Some(executed_data.output_objects)
            } else {
                None
            },
            auxiliary_data: None,
        })
    }

    #[instrument(level = "debug", skip_all, err(level = "debug"), fields(tx_digest = ?tx_digest, ret_effects_digest = tracing::field::Empty
    ))]
    async fn get_full_effects<A>(
        &self,
        client: Arc<SafeClient<A>>,
        tx_digest: Option<TransactionDigest>,
        options: &SubmitTransactionOptions,
    ) -> FullEffectsResult
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let request = build_tx_status_request(tx_digest, true);

        match timeout(
            WAIT_FOR_EFFECTS_TIMEOUT,
            client.get_tx_status(request, options.forwarded_client_addr),
        )
        .await
        {
            Ok(Ok(statuses)) => match statuses.into_iter().next() {
                Some((
                    _,
                    TxStatusUpdate::Executed {
                        effects_digest,
                        details,
                    },
                )) => {
                    if let Some(details) = details {
                        tracing::Span::current()
                            .record("ret_effects_digest", format!("{effects_digest:?}"));
                        Ok((effects_digest, details))
                    } else {
                        tracing::debug!("Execution data not found, retrying...");
                        Err(TransactionRequestError::ValidatorInternal(
                            "Execution data not found".to_string(),
                        ))
                    }
                }
                Some((_, TxStatusUpdate::Rejected { error })) => {
                    Err(TransactionRequestError::RejectedAtValidator(error))
                }
                Some((_, TxStatusUpdate::Expired { epoch })) => {
                    Err(TransactionRequestError::StatusExpired(epoch))
                }
                Some((_, TxStatusUpdate::Submitted)) => {
                    Err(TransactionRequestError::ValidatorInternal(
                        "Transaction still pending".to_string(),
                    ))
                }
                None => Err(TransactionRequestError::ValidatorInternal(
                    "Empty response from validator".to_string(),
                )),
            },
            Ok(Err(e)) => Err(TransactionRequestError::Aborted(e)),
            Err(_) => Err(TransactionRequestError::TimedOutGettingFullEffectsAtValidator),
        }
    }

    /// Gets full effects from a validator, with speculative fallback to other
    /// validators.
    ///
    /// If the initial validator doesn't respond within
    /// GET_FULL_EFFECTS_FALLBACK_DELAY, we start parallel requests to
    /// validators that have already acknowledged the effects (received via
    /// the acked_validators channel from wait_for_acknowledgments).
    ///
    /// This prevents slow validators from blocking the entire operation when
    /// faster validators are available, while still preferring the initial
    /// validator if it responds quickly.
    #[instrument(level = "debug", skip_all, fields(tx_digest = ?tx_digest, initial_validator = ?initial_target
    ))]
    async fn get_full_effects_with_fallback<A>(
        &self,
        authority_aggregator: &AuthorityAggregator<A>,
        initial_client: Arc<SafeClient<A>>,
        initial_target: AuthorityName,
        tx_digest: Option<TransactionDigest>,
        options: &SubmitTransactionOptions,
        mut acked_validators_rx: Receiver<AuthorityName>,
    ) -> (FullEffectsResult, AuthorityName)
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let mut pending_requests: FuturesUnordered<
            BoxFuture<'_, (AuthorityName, FullEffectsResult)>,
        > = FuturesUnordered::new();

        // Add initial request to the pending set alongside fallbacks for uniform
        // handling
        let initial_request = self.get_full_effects(initial_client, tx_digest, options);
        pending_requests.push(Box::pin(
            async move { (initial_target, initial_request.await) },
        ));

        let mut fallback_delay = tokio::time::interval(GET_FULL_EFFECTS_FALLBACK_DELAY);
        fallback_delay.reset();

        loop {
            tokio::select! {
                Some((validator, result)) = pending_requests.next() => {
                    // Return as soon as any request (including fallback) completes - the caller handles retries for errors
                    return (result, validator);
                }

                // After delay, try to start a fallback request to an acked validator
                _ = fallback_delay.tick() => {
                    // Drain all available acked validators and pick one we haven't tried
                    while let Ok(acked_validator) = acked_validators_rx.try_recv() {
                        // We send ack requests to all validators, so skip if the acked validator was the initial target
                        if acked_validator == initial_target {
                            continue;
                        }

                        let Some(client) = authority_aggregator.authority_clients.get(&acked_validator) else {
                            continue;
                        };

                        tracing::debug!(
                            ?acked_validator,
                            "Starting fallback get_full_effects request"
                        );

                        let fut = self.get_full_effects(
                            client.clone(),
                            tx_digest,
                            options,
                        );

                        pending_requests.push(Box::pin(async move {
                            (acked_validator, fut.await)
                        }));

                        // Only start one fallback per interval
                        break;
                    }
                }
            }
        }
    }

    /// Corroborate a single-validator effects-fetch error in the
    /// skip-certification path by querying all other validators for the tx
    /// status.
    ///
    /// Behavior:
    /// - If f+1 stake (including the initial validator if its error was a
    ///   non-retriable rejection) report a non-retriable rejection, return
    ///   [`TransactionDriverError::RejectedByValidators`] so the driver's outer
    ///   loop surfaces a terminal error to the user.
    /// - If the f+1 rejection threshold becomes unreachable (or the broadcast
    ///   ends without reaching it), record bad client-monitor feedback for the
    ///   initial validator and return
    ///   [`TransactionDriverError::SubmittedButFetchFailed`]. That error is
    ///   classified retriable, so the outer `drive_transaction` loop reissues
    ///   submission; the bad feedback deprioritizes the suspect in the next
    ///   `RequestRetrier`'s ranking.
    ///
    /// TODO: this returns `SubmittedButFetchFailed` (retriable) for every
    /// inconclusive outcome, which makes the driver reissue submission even
    /// when the initial validator was honest and just had a transient fetch
    /// failure — in that case the tx is still in consensus and would land in a
    /// checkpoint without our help. A future improvement would split the
    /// inconclusive bucket: an f+1 "unknown to validator" majority signals
    /// "tx wasn't disseminated, try another validator" (retriable), while a
    /// mix of seen-and-executed responses signals "in flight, just wait for
    /// checkpoint" (non-retriable; the gRPC handler then rebuilds from cache
    /// or returns `DeadlineExceeded`).
    #[instrument(level = "debug", skip_all, fields(tx_digest = ?tx_digest, initial_validator = ?initial_validator))]
    async fn corroborate_single_validator_error<A>(
        &self,
        authority_aggregator: &AuthorityAggregator<A>,
        client_monitor: &Arc<ValidatorClientMonitor>,
        tx_digest: Option<TransactionDigest>,
        initial_validator: AuthorityName,
        initial_error: TransactionRequestError,
        options: &SubmitTransactionOptions,
    ) -> TransactionDriverError
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let committee = authority_aggregator.committee.clone();
        let total_votes = committee.total_votes();
        let validity_threshold = committee.validity_threshold();
        let initial_display_name = authority_aggregator.get_display_name(&initial_validator);

        let mut non_retriable_rejected =
            StatusAggregator::<TransactionRequestError>::new(committee.clone());
        // Tracks total responded stake (incl. the initial validator) so the
        // unreachability check has a "validators not yet heard from" tally —
        // `non_retriable_rejected` only counts rejection votes.
        let mut responded = StatusAggregator::<()>::new(committee.clone());
        responded.insert(initial_validator, ());

        // Only count the initial validator's error as a rejection vote if it
        // actually claimed rejection. Transport/RPC errors do not.
        let initial_is_rejection = matches!(
            &initial_error,
            TransactionRequestError::RejectedAtValidator(_)
        ) && !initial_error.is_submission_retriable();
        if initial_is_rejection {
            non_retriable_rejected.insert(initial_validator, initial_error.clone());
        }

        // Short-circuit for committees small enough that the seed alone meets
        // the validity threshold (single-validator tests).
        if non_retriable_rejected.reached_validity_threshold() {
            self.metrics.skip_cert_corroborated_rejections.inc();
            return TransactionDriverError::RejectedByValidators {
                submission_non_retriable_errors: aggregate_request_errors(
                    non_retriable_rejected.status_by_authority(),
                ),
                submission_retriable_errors: aggregate_request_errors(vec![]),
            };
        }

        let request = build_tx_status_request(tx_digest, false);

        let mut futures = FuturesUnordered::new();
        for (name, client) in authority_aggregator.authority_clients.iter() {
            let name = *name;
            if name == initial_validator {
                continue;
            }
            let client = client.clone();
            let request = request.clone();
            let display_name = authority_aggregator.get_display_name(&name);
            let monitor = client_monitor.clone();
            let fut = async move {
                let started = Instant::now();
                let raw = timeout(
                    WAIT_FOR_EFFECTS_TIMEOUT,
                    client.get_tx_status(request, options.forwarded_client_addr),
                )
                .await;

                let feedback_builder =
                    OperationFeedback::builder(name, display_name, OperationType::Effects);
                let mapped = match raw {
                    Ok(Ok(mut statuses)) => {
                        let update = statuses.pop().map(|(_, u)| Ok(u)).unwrap_or_else(|| {
                            Err(TransactionRequestError::ValidatorInternal(
                                "Empty response from validator".to_string(),
                            ))
                        });
                        monitor
                            .record_interaction_result(feedback_builder.ok_now(started.elapsed()));
                        update
                    }
                    Ok(Err(e)) => Err(TransactionRequestError::Aborted(e)),
                    Err(_) => {
                        monitor.record_interaction_result(feedback_builder.err_now());
                        Err(TransactionRequestError::TimedOutGettingFullEffectsAtValidator)
                    }
                };

                (name, mapped)
            };
            futures.push(fut);
        }

        while let Some((name, response)) = futures.next().await {
            responded.insert(name, ());

            if let Ok(TxStatusUpdate::Rejected { error }) = &response {
                let wrapped = TransactionRequestError::RejectedAtValidator(error.clone());
                if !wrapped.is_submission_retriable() {
                    non_retriable_rejected.insert(name, wrapped);
                }
            }

            if non_retriable_rejected.reached_validity_threshold() {
                self.metrics.skip_cert_corroborated_rejections.inc();
                return TransactionDriverError::RejectedByValidators {
                    submission_non_retriable_errors: aggregate_request_errors(
                        non_retriable_rejected.status_by_authority(),
                    ),
                    submission_retriable_errors: aggregate_request_errors(vec![]),
                };
            }
            // Even if every still-unheard validator rejected non-retriably,
            // the f+1 threshold can no longer be reached — stop early and let
            // the caller fall through to the retriable-error path.
            let unseen_stake = total_votes - responded.total_votes();
            if non_retriable_rejected.total_votes() + unseen_stake < validity_threshold {
                break;
            }
        }

        // Record bad feedback for the suspect so the next retry's shuffled
        // ranking deprioritizes it.
        client_monitor.record_interaction_result(
            OperationFeedback::builder(
                initial_validator,
                initial_display_name,
                OperationType::Effects,
            )
            .err_now(),
        );
        self.metrics.skip_cert_corroboration_unreachable.inc();
        TransactionDriverError::SubmittedButFetchFailed {
            validator: initial_validator,
            error: format!("{initial_error} (corroboration inconclusive)"),
        }
    }

    #[instrument(level = "debug", skip_all, err(level = "debug"), ret)]
    async fn wait_for_acknowledgments<A>(
        &self,
        authority_aggregator: &AuthorityAggregator<A>,
        client_monitor: &ValidatorClientMonitor,
        tx_digest: Option<TransactionDigest>,
        options: &SubmitTransactionOptions,
        _submitted_tx_to_validator: AuthorityName,
        acked_validators_tx: Sender<AuthorityName>,
    ) -> Result<TransactionEffectsDigest, TransactionDriverError>
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        self.metrics.certified_effects_ack_attempts.inc();
        let timer = tokio::time::Instant::now();
        let clients = authority_aggregator
            .authority_clients
            .iter()
            .collect::<Vec<_>>();
        let committee = authority_aggregator.committee.clone();

        // Broadcast requests for digest acknowledgments against all validators.
        let mut futures = FuturesUnordered::new();
        for (name, client) in clients {
            let client = client.clone();
            let name = *name;
            let display_name = authority_aggregator.get_display_name(&name);

            let request = build_tx_status_request(tx_digest, false);

            let future = async move {
                match timeout(
                    WAIT_FOR_EFFECTS_TIMEOUT,
                    self.wait_for_acknowledgment_rpc(
                        name,
                        display_name.clone(),
                        &client,
                        client_monitor,
                        request,
                        options,
                    ),
                )
                .await
                {
                    Ok(result) => (name, result),
                    Err(_) => {
                        let feedback =
                            OperationFeedback::builder(name, display_name, OperationType::Effects)
                                .err_now();
                        client_monitor.record_interaction_result(feedback);
                        (name, Err(IotaError::Timeout))
                    }
                }
            };

            futures.push(future);
        }

        let mut effects_digest_aggregators: BTreeMap<
            TransactionEffectsDigest,
            StatusAggregator<()>,
        > = BTreeMap::new();
        // Collect responses from validators which observed the transaction getting
        // rejected, and rejected the transaction with errors non-retriable with
        // new transaction submissions.
        let mut non_retriable_errors_aggregator =
            StatusAggregator::<TransactionRequestError>::new(committee.clone());
        // Collect responses from validators which observed the transaction getting
        // rejected, and rejected the transaction with errors retriable with new
        // transaction submissions.
        let mut retriable_errors_aggregator =
            StatusAggregator::<TransactionRequestError>::new(committee.clone());
        // Collect responses from validators which observed the transaction getting
        // rejected, but do not have a local reason to reject the transaction.
        let reason_not_found_aggregator = StatusAggregator::<()>::new(committee.clone());
        // Every validator returns at most one TxStatusUpdate.
        while let Some((name, response)) = futures.next().await {
            // Extract the first per-item result from the batch response.
            let single_result = response.map(|statuses| statuses.into_iter().next());
            match single_result {
                Ok(Some((
                    _,
                    TxStatusUpdate::Executed {
                        effects_digest,
                        details: _,
                    },
                ))) => {
                    // Notify that this validator has successfully executed the transaction.
                    let _ = acked_validators_tx.try_send(name);

                    let aggregator = effects_digest_aggregators
                        .entry(effects_digest)
                        .or_insert_with(|| StatusAggregator::<()>::new(committee.clone()));
                    aggregator.insert(name, ());

                    if aggregator.reached_quorum_threshold() {
                        let quorum_weight = aggregator.total_votes();
                        for (other_digest, other_aggregator) in effects_digest_aggregators {
                            if other_digest != effects_digest && other_aggregator.total_votes() > 0
                            {
                                tracing::warn!(
                                    ?name,
                                    "Effects digest inconsistency detected: quorum digest {effects_digest:?} (weight {quorum_weight}), other digest {other_digest:?} (weight {})",
                                    other_aggregator.total_votes()
                                );
                                self.metrics.effects_digest_mismatches.inc();
                            }
                        }
                        // Record success and latency
                        self.metrics.certified_effects_ack_successes.inc();
                        self.metrics
                            .certified_effects_ack_latency
                            .observe(timer.elapsed().as_secs_f64());

                        return Ok(effects_digest);
                    }
                }
                Ok(Some((_, TxStatusUpdate::Rejected { error }))) => {
                    tracing::trace!(name = ?name.concise(), "Rejected at validator: {:?}", error);
                    let error = TransactionRequestError::RejectedAtValidator(error);
                    if error.is_submission_retriable() {
                        retriable_errors_aggregator.insert(name, error);
                    } else {
                        non_retriable_errors_aggregator.insert(name, error);
                    }
                    self.metrics.rejection_acks.inc();
                }
                Ok(Some((_, TxStatusUpdate::Expired { epoch }))) => {
                    let error = TransactionRequestError::StatusExpired(epoch);
                    // Expired status is submission retriable.
                    retriable_errors_aggregator.insert(name, error);
                    self.metrics.expiration_acks.inc();
                }
                Ok(Some((_, TxStatusUpdate::Submitted))) => {
                    // Still pending — treat as retriable.
                    let error = TransactionRequestError::ValidatorInternal(
                        "Transaction still pending".to_string(),
                    );
                    retriable_errors_aggregator.insert(name, error);
                }
                Ok(None) => {
                    // Empty response from validator — treat as retriable error.
                    let error = TransactionRequestError::ValidatorInternal(
                        "Empty response from validator".to_string(),
                    );
                    retriable_errors_aggregator.insert(name, error);
                }
                Err(error) => {
                    let error = TransactionRequestError::Aborted(error);
                    if error.is_submission_retriable() {
                        retriable_errors_aggregator.insert(name, error);
                    } else {
                        non_retriable_errors_aggregator.insert(name, error);
                    }
                }
            };

            // Adding vote up between different StatusAggregators without de-duplication is
            // ok, because each authority only returns one response.
            let executed_weight: u64 = effects_digest_aggregators
                .values()
                .map(|agg| agg.total_votes())
                .sum();
            let total_weight = executed_weight
                + reason_not_found_aggregator.total_votes()
                + non_retriable_errors_aggregator.total_votes()
                + retriable_errors_aggregator.total_votes();
            let remaining_weight = committee.total_votes() - total_weight;

            // Wait for a quorum of responses, to not summarize the responses too early.
            if total_weight >= committee.quorum_threshold() {
                // Try returning non-retriable aggregated error first.
                if non_retriable_errors_aggregator.total_votes() >= committee.validity_threshold() {
                    return Err(TransactionDriverError::RejectedByValidators {
                        submission_non_retriable_errors: aggregate_request_errors(
                            non_retriable_errors_aggregator.status_by_authority(),
                        ),
                        submission_retriable_errors: aggregate_request_errors(
                            retriable_errors_aggregator.status_by_authority(),
                        ),
                    });
                }
                // Return a retriable aggregated error only if it becomes impossible to gather
                // enough non-retriable errors.
                if non_retriable_errors_aggregator.total_votes() + remaining_weight
                    < committee.validity_threshold()
                    && retriable_errors_aggregator.total_votes()
                        + non_retriable_errors_aggregator.total_votes()
                        >= committee.validity_threshold()
                {
                    let mut observed_effects_digests =
                        Vec::<(TransactionEffectsDigest, Vec<AuthorityName>, StakeUnit)>::new();
                    for (effects_digest, aggregator) in effects_digest_aggregators {
                        observed_effects_digests.push((
                            effects_digest,
                            aggregator.authorities(),
                            aggregator.total_votes(),
                        ));
                    }
                    return Err(TransactionDriverError::Aborted {
                        submission_non_retriable_errors: aggregate_request_errors(
                            non_retriable_errors_aggregator.status_by_authority(),
                        ),
                        submission_retriable_errors: aggregate_request_errors(
                            retriable_errors_aggregator.status_by_authority(),
                        ),
                        observed_effects_digests: AggregatedEffectsDigests {
                            digests: observed_effects_digests,
                        },
                    });
                }
            }
        }

        // At this point, no effects digest has reached quorum. But failed responses do
        // not reach validity threshold either.
        let retriable_weight =
            retriable_errors_aggregator.total_votes() + reason_not_found_aggregator.total_votes();
        // Whether the transaction is retriable regardless of known effects.
        let mut submission_retriable = retriable_weight >= committee.quorum_threshold();
        let mut observed_effects_digests =
            Vec::<(TransactionEffectsDigest, Vec<AuthorityName>, StakeUnit)>::new();
        for (effects_digest, aggregator) in effects_digest_aggregators {
            // This effects digest can still get certified, so the transaction is retriable.
            if aggregator.total_votes() + retriable_weight >= committee.quorum_threshold() {
                submission_retriable = true;
            }
            observed_effects_digests.push((
                effects_digest,
                aggregator.authorities(),
                aggregator.total_votes(),
            ));
        }
        if submission_retriable {
            Err(TransactionDriverError::Aborted {
                submission_non_retriable_errors: aggregate_request_errors(
                    non_retriable_errors_aggregator.status_by_authority(),
                ),
                submission_retriable_errors: aggregate_request_errors(
                    retriable_errors_aggregator.status_by_authority(),
                ),
                observed_effects_digests: AggregatedEffectsDigests {
                    digests: observed_effects_digests,
                },
            })
        } else {
            if observed_effects_digests.len() <= 1 {
                debug_fatal!(
                    "Expect at least 2 effects digests, but got {:?}",
                    observed_effects_digests
                );
            }
            Err(TransactionDriverError::ForkedExecution {
                observed_effects_digests: AggregatedEffectsDigests {
                    digests: observed_effects_digests,
                },
                submission_non_retriable_errors: aggregate_request_errors(
                    non_retriable_errors_aggregator.status_by_authority(),
                ),
                submission_retriable_errors: aggregate_request_errors(
                    retriable_errors_aggregator.status_by_authority(),
                ),
            })
        }
    }

    #[instrument(level = "debug", skip_all, err(level = "debug"), ret, fields(validator_display_name = ?display_name
    ))]
    async fn wait_for_acknowledgment_rpc<A>(
        &self,
        name: AuthorityName,
        display_name: String,
        client: &Arc<SafeClient<A>>,
        client_monitor: &ValidatorClientMonitor,
        request: GetTxStatusRequest,
        options: &SubmitTransactionOptions,
    ) -> IotaResult<Vec<(TransactionDigest, TxStatusUpdate)>>
    where
        A: AuthorityAPI + Send + Sync + 'static,
    {
        let effects_start = Instant::now();
        let backoff =
            ExponentialBackoff::new(Duration::from_millis(100), MAX_WAIT_FOR_EFFECTS_RETRY_DELAY);
        // This loop should only retry errors that are retriable without new submission.
        for (attempt, delay) in backoff.enumerate() {
            let result = client
                .get_tx_status(request.clone(), options.forwarded_client_addr)
                .await;
            let feedback_builder =
                OperationFeedback::builder(name, display_name.clone(), OperationType::Effects);
            match result {
                Ok(response) => {
                    let latency = effects_start.elapsed();
                    client_monitor.record_interaction_result(feedback_builder.ok_now(latency));
                    return Ok(response);
                }
                Err(e) => {
                    client_monitor.record_interaction_result(feedback_builder.err_now());
                    if !matches!(e, IotaError::Rpc(_, _)) {
                        return Err(e);
                    }
                    tracing::trace!(
                        ?name,
                        "Wait for effects acknowledgement (attempt {attempt}): rpc error: {:?}",
                        e
                    );
                }
            };
            sleep(delay).await;
        }
        Err(IotaError::Timeout)
    }

    /// Creates the final full response.
    fn get_quorum_transaction_response(
        &self,
        effects_digest: TransactionEffectsDigest,
        executed_data: ExecutedData,
    ) -> QuorumTransactionResponse {
        self.metrics.executed_transactions.inc();

        tracing::debug!("Transaction executed with effects digest: {effects_digest}",);

        let epoch = executed_data.effects.epoch();
        let details = FinalizedEffects {
            effects: executed_data.effects,
            finality_info: EffectsFinalityInfo::QuorumExecuted(epoch),
        };

        QuorumTransactionResponse {
            effects: details,
            events: executed_data.events,
            input_objects: if !executed_data.input_objects.is_empty() {
                Some(executed_data.input_objects)
            } else {
                None
            },
            output_objects: if !executed_data.output_objects.is_empty() {
                Some(executed_data.output_objects)
            } else {
                None
            },
            auxiliary_data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_types::{
        digests::TransactionDigest,
        error::{IotaError, UserInputError},
        messages_grpc::TxStatusUpdate,
    };

    use super::*;
    use crate::{
        authority_aggregator::AuthorityAggregatorBuilder, test_authority_clients::MockAuthorityApi,
        validator_client_monitor::ValidatorClientMonitor,
    };

    fn make_aggregator(size: usize) -> Arc<AuthorityAggregator<MockAuthorityApi>> {
        Arc::new(
            AuthorityAggregatorBuilder::from_committee_size(size).build_mock_authority_aggregator(),
        )
    }

    fn set_validator_status(
        agg: &AuthorityAggregator<MockAuthorityApi>,
        name: &AuthorityName,
        digest: TransactionDigest,
        update: TxStatusUpdate,
    ) {
        let client = agg.authority_clients.get(name).unwrap();
        client
            .authority_client()
            .stub_tx_status(Ok(vec![(digest, update)]));
    }

    fn options() -> SubmitTransactionOptions {
        SubmitTransactionOptions::default()
    }

    fn rejection_iota_error() -> IotaError {
        IotaError::UserInput {
            error: UserInputError::EmptyCommandInput,
        }
    }

    #[tokio::test]
    async fn corroborate_returns_rejected_when_threshold_reached() {
        let agg = make_aggregator(4);
        let metrics = Arc::new(TransactionDriverMetrics::new_for_tests());
        let certifier = EffectsCertifier::new(metrics.clone());
        let monitor = Arc::new(ValidatorClientMonitor::new_for_test());
        let digest = TransactionDigest::random();

        let names: Vec<_> = agg.committee.names().copied().collect();
        // Initial validator (already counted via pre-seed). Other validators
        // include one that confirms with a non-retriable rejection — that
        // brings stake to f+1.
        let initial = names[0];
        set_validator_status(
            &agg,
            &names[1],
            digest,
            TxStatusUpdate::Rejected {
                error: rejection_iota_error(),
            },
        );
        // Remaining validators return `Submitted` — non-rejection so they
        // count toward the "responded but not rejection" tally.
        for name in &names[2..] {
            set_validator_status(&agg, name, digest, TxStatusUpdate::Submitted);
        }

        let initial_error = TransactionRequestError::RejectedAtValidator(rejection_iota_error());
        let err = certifier
            .corroborate_single_validator_error(
                &agg,
                &monitor,
                Some(digest),
                initial,
                initial_error,
                &options(),
            )
            .await;
        assert!(
            matches!(err, TransactionDriverError::RejectedByValidators { .. }),
            "expected RejectedByValidators, got {err:?}"
        );
        assert_eq!(metrics.skip_cert_corroborated_rejections.get(), 1);
    }

    #[tokio::test]
    async fn corroborate_returns_fetch_failed_when_unreachable() {
        let agg = make_aggregator(4);
        let metrics = Arc::new(TransactionDriverMetrics::new_for_tests());
        let certifier = EffectsCertifier::new(metrics.clone());
        let monitor = Arc::new(ValidatorClientMonitor::new_for_test());
        let digest = TransactionDigest::random();

        let names: Vec<_> = agg.committee.names().copied().collect();
        let initial = names[0];
        // All other validators report `Submitted` (have not seen a rejection),
        // so the rejection threshold becomes unreachable.
        for name in &names[1..] {
            set_validator_status(&agg, name, digest, TxStatusUpdate::Submitted);
        }

        let initial_error = TransactionRequestError::RejectedAtValidator(rejection_iota_error());
        let err = certifier
            .corroborate_single_validator_error(
                &agg,
                &monitor,
                Some(digest),
                initial,
                initial_error,
                &options(),
            )
            .await;
        assert!(
            matches!(err, TransactionDriverError::SubmittedButFetchFailed { .. }),
            "expected SubmittedButFetchFailed, got {err:?}"
        );
        assert_eq!(metrics.skip_cert_corroboration_unreachable.get(), 1);
    }

    #[tokio::test]
    async fn corroborate_ignores_retriable_initial_error() {
        let agg = make_aggregator(4);
        let metrics = Arc::new(TransactionDriverMetrics::new_for_tests());
        let certifier = EffectsCertifier::new(metrics.clone());
        let monitor = Arc::new(ValidatorClientMonitor::new_for_test());
        let digest = TransactionDigest::random();

        let names: Vec<_> = agg.committee.names().copied().collect();
        let initial = names[0];
        // Two of the three other validators reject non-retriably (f+1 of three
        // non-initial = 2). Since the initial error is retriable it is NOT
        // pre-seeded into the aggregator — the broadcast itself must reach
        // f+1 on its own.
        for name in &names[1..3] {
            set_validator_status(
                &agg,
                name,
                digest,
                TxStatusUpdate::Rejected {
                    error: rejection_iota_error(),
                },
            );
        }
        set_validator_status(&agg, &names[3], digest, TxStatusUpdate::Submitted);

        let initial_error = TransactionRequestError::TimedOutGettingFullEffectsAtValidator;
        let err = certifier
            .corroborate_single_validator_error(
                &agg,
                &monitor,
                Some(digest),
                initial,
                initial_error,
                &options(),
            )
            .await;
        assert!(
            matches!(err, TransactionDriverError::RejectedByValidators { .. }),
            "expected RejectedByValidators, got {err:?}"
        );
    }

    #[tokio::test]
    async fn get_effects_without_certification_corroborates_rejection() {
        let agg = make_aggregator(4);
        let metrics = Arc::new(TransactionDriverMetrics::new_for_tests());
        let certifier = EffectsCertifier::new(metrics.clone());
        let monitor = Arc::new(ValidatorClientMonitor::new_for_test());
        let digest = TransactionDigest::random();

        let names: Vec<_> = agg.committee.names().copied().collect();
        let initial = names[0];
        set_validator_status(
            &agg,
            &initial,
            digest,
            TxStatusUpdate::Rejected {
                error: rejection_iota_error(),
            },
        );
        for name in &names[1..] {
            set_validator_status(
                &agg,
                name,
                digest,
                TxStatusUpdate::Rejected {
                    error: rejection_iota_error(),
                },
            );
        }

        let err = certifier
            .get_effects_without_certification(
                &agg,
                &monitor,
                Some(digest),
                initial,
                TxStatusUpdate::Submitted,
                &options(),
            )
            .await
            .expect_err("rejection should propagate");
        assert!(
            matches!(err, TransactionDriverError::RejectedByValidators { .. }),
            "expected RejectedByValidators, got {err:?}"
        );
        assert_eq!(metrics.skip_cert_corroborated_rejections.get(), 1);
    }

    /// Inconclusive corroboration returns the retriable
    /// `SubmittedButFetchFailed` so the outer driver loop reissues submission
    /// rather than surfacing a terminal error to the user.
    #[tokio::test]
    async fn get_effects_without_certification_returns_fetch_failed_when_inconclusive() {
        let agg = make_aggregator(4);
        let metrics = Arc::new(TransactionDriverMetrics::new_for_tests());
        let certifier = EffectsCertifier::new(metrics.clone());
        let monitor = Arc::new(ValidatorClientMonitor::new_for_test());
        let digest = TransactionDigest::random();

        let names: Vec<_> = agg.committee.names().copied().collect();
        let initial = names[0];
        set_validator_status(
            &agg,
            &initial,
            digest,
            TxStatusUpdate::Rejected {
                error: rejection_iota_error(),
            },
        );
        for name in &names[1..] {
            set_validator_status(&agg, name, digest, TxStatusUpdate::Submitted);
        }

        let err = certifier
            .get_effects_without_certification(
                &agg,
                &monitor,
                Some(digest),
                initial,
                TxStatusUpdate::Submitted,
                &options(),
            )
            .await
            .expect_err("fetch should fail");
        assert!(
            matches!(err, TransactionDriverError::SubmittedButFetchFailed { .. }),
            "expected SubmittedButFetchFailed, got {err:?}"
        );
        assert_eq!(metrics.skip_cert_corroboration_unreachable.get(), 1);
    }
}
