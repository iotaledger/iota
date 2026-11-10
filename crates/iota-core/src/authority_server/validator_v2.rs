// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::{StreamExt, future::Either, stream};
use iota_common::debug_fatal;
use iota_metrics::spawn_monitored_task;
use iota_network::api::{
    GetTxStatusRequest, HealthCheckRequest, HealthCheckResponse, NotifyCapabilitiesRequest,
    NotifyCapabilitiesResponse, SubmitTxRequest, TxStatus, ValidatorV2,
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, ObjectId, TransactionDigest, TransactionEffects};
use iota_types::{
    attestation::{Attestation, AttestedTransaction},
    deny_rule_governance::DenyRuleConfig,
    effects::TransactionEffectsAPI,
    error::{IotaError, IotaResult, UserInputError},
    fp_ensure,
    messages_consensus::ConsensusTransaction,
    messages_grpc::{
        ExecutedData, GetTxStatusRequest as DomainGetTxStatusRequest,
        HandleCapabilityNotificationRequestV1, HandleCapabilityNotificationResponseV1,
        TxStatusUpdate, ValidatorHealthRequest, ValidatorHealthResponse,
    },
    traffic_control::Weight,
    transaction::{SenderSignedTransactionAPI, TransactionEnvelope},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    authority_server::{
        StreamResponse, ValidatorService, ValidatorServiceMetrics, normalize,
        soft_lock::PreConsensusSoftLocks,
    },
    consensus_adapter::ConsensusAdapter,
    epoch::reconfiguration::ReconfigState,
    execution_scheduler::ExecutionSchedulerAPI,
};

/// Maximum number of transactions allowed in a single `submit_tx` request,
/// bounding the traffic tallies and execution work one request can create.
const MAX_TRANSACTIONS_PER_SUBMIT: usize = 32;

/// Maximum number of queries allowed in a single `get_tx_status` request.
/// Matches `MAX_TRANSACTIONS_PER_SUBMIT` so one status poll can cover a full
/// submit batch.
const MAX_QUERIES_PER_GET_TX_STATUS: usize = 32;

/// Timeout for waiting on transaction execution in `get_tx_status`.
const GET_TX_STATUS_TIMEOUT_SECS: u64 = 30;

/// The union of two deny rule sources: denies whatever either source denies.
struct DenyRuleUnion<'a> {
    first: &'a dyn DenyRuleConfig,
    second: &'a dyn DenyRuleConfig,
}

impl DenyRuleConfig for DenyRuleUnion<'_> {
    fn is_address_denied(&self, address: &Address) -> bool {
        self.first.is_address_denied(address) || self.second.is_address_denied(address)
    }

    fn is_object_denied(&self, id: &ObjectId) -> bool {
        self.first.is_object_denied(id) || self.second.is_object_denied(id)
    }

    fn is_package_denied(&self, id: &ObjectId) -> bool {
        self.first.is_package_denied(id) || self.second.is_package_denied(id)
    }

    fn has_denied_addresses(&self) -> bool {
        self.first.has_denied_addresses() || self.second.has_denied_addresses()
    }

    fn has_denied_objects(&self) -> bool {
        self.first.has_denied_objects() || self.second.has_denied_objects()
    }

    fn has_denied_packages(&self) -> bool {
        self.first.has_denied_packages() || self.second.has_denied_packages()
    }

    fn package_publish_disabled(&self) -> bool {
        self.first.package_publish_disabled() || self.second.package_publish_disabled()
    }

    fn package_upgrade_disabled(&self) -> bool {
        self.first.package_upgrade_disabled() || self.second.package_upgrade_disabled()
    }

    fn shared_object_disabled(&self) -> bool {
        self.first.shared_object_disabled() || self.second.shared_object_disabled()
    }

    fn user_transaction_disabled(&self) -> bool {
        self.first.user_transaction_disabled() || self.second.user_transaction_disabled()
    }

    fn receiving_objects_disabled(&self) -> bool {
        self.first.receiving_objects_disabled() || self.second.receiving_objects_disabled()
    }

    fn move_authenticator_disabled(&self) -> bool {
        self.first.move_authenticator_disabled() || self.second.move_authenticator_disabled()
    }
}

/// Maximum number of `submit_single_tx` futures allowed to run concurrently
/// per `submit_tx` request. Caps pre-consensus work (signature verification,
/// validation, DB reads) and contention on `consensus_adapter`'s submit
/// semaphore.
const MAX_CONCURRENT_SUBMIT_TASKS: usize = 16;

/// A single streamed item in a V2 RPC response. The `Weight` is the per-item
/// spam-policy contribution decided by the producing code path.
type TxUpdateItem = Result<((TransactionDigest, TxStatusUpdate), Weight), tonic::Status>;

/// Whether admission checks the transaction's inputs against local state: that
/// they load at the declared versions, pass the deny checks and get their soft
/// locks.
///
/// Tests can register the `admit-without-input-checks` fail point to have a
/// validator submit to consensus a transaction it would otherwise reject, as a
/// faulty validator can.
fn checks_admitted_inputs() -> bool {
    let checks = std::cell::Cell::new(true);
    iota_macros::fail_point_if!("admit-without-input-checks", || checks.set(false));
    checks.get()
}

/// Whether the validator admits new user transactions in `reconfig_state`. It
/// stops once its epoch starts closing.
///
/// Tests can register the `admit-user-transactions-after-epoch-close` fail
/// point to keep a validator admitting them, and submitting them to consensus,
/// after that point.
fn admits_user_transactions(reconfig_state: &ReconfigState) -> bool {
    // A `Cell` keeps the binding immutable in builds where the fail point
    // compiles to nothing.
    let admits = std::cell::Cell::new(reconfig_state.should_accept_user_certs());
    iota_macros::fail_point_if!("admit-user-transactions-after-epoch-close", || {
        admits.set(true)
    });
    admits.get()
}

/// Checks that `consensus_tx` is no larger than consensus accepts: its BCS size
/// against [`ProtocolConfig::max_transaction_size_bytes()`], above which
/// Starfish refuses a submitted transaction.
fn check_consensus_transaction_size(
    consensus_tx: &ConsensusTransaction,
    protocol_config: &ProtocolConfig,
) -> IotaResult {
    let size = bcs::serialized_size(consensus_tx).map_err(|error| {
        IotaError::TransactionSerialization {
            error: error.to_string(),
        }
    })?;
    let max_size = protocol_config.max_transaction_size_bytes();
    fp_ensure!(
        size as u64 <= max_size,
        IotaError::UserInput {
            error: UserInputError::SizeLimitExceeded {
                limit: format!(
                    "serialized consensus transaction size exceeded maximum of {max_size}"
                ),
                value: size.to_string(),
            }
        }
    );

    Ok(())
}

/// Rejects a submission because the validator is overloaded, and counts it in
/// `num_rejected_tx_during_overload`. The rejection weighs nothing in the
/// traffic controller's tally: the client turned away is not necessarily the
/// one keeping the validator busy.
fn overload_rejection(
    metrics: &ValidatorServiceMetrics,
    error: IotaError,
) -> (TxStatusUpdate, Weight) {
    metrics
        .num_rejected_tx_during_overload
        .with_label_values(&[error.as_ref()])
        .inc();

    (TxStatusUpdate::Rejected { error }, Weight::zero())
}

/// Counts a refused soft lock under its cause.
fn count_soft_lock_refusal(metrics: &ValidatorServiceMetrics, error: &IotaError) {
    match error {
        IotaError::RecentlyResubmitted { .. } => {
            metrics.num_rejected_tx_recently_resubmitted.inc();
        }
        IotaError::ObjectLockConflict { .. } => {
            metrics.num_rejected_tx_soft_lock_conflict.inc();
        }
        _ => {}
    }
}

impl ValidatorService {
    async fn submit_tx_impl(
        &self,
        transactions: Vec<TransactionEnvelope>,
    ) -> Result<ReceiverStream<TxUpdateItem>, tonic::Status> {
        let state = self.state.clone();
        let epoch_store = state.load_epoch_store_one_call_per_task();

        fp_ensure!(
            !state.is_fullnode(&epoch_store),
            IotaError::FullNodeCantHandleValidatorV2.into()
        );

        fp_ensure!(
            epoch_store.protocol_config().enable_pcool_flow(),
            IotaError::UnsupportedFeature {
                error: "P-COOL flow is not enabled in this protocol version".to_string()
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
        let soft_locks = self.soft_locks.clone();

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
                    let soft_locks = soft_locks.clone();
                    spawn_monitored_task!(async move {
                        let tx_digest = *transaction.digest();
                        let (update, weight) = Self::submit_single_tx(
                            &state,
                            &consensus_adapter,
                            &metrics,
                            &epoch_store,
                            &soft_locks,
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
    /// already-executed duplicate (spam-like) and for a transaction too large
    /// for consensus once attested, `normalize(&error)` for any other rejection
    /// (signature/epoch errors weigh, others don't), and `Weight::zero()` for a
    /// successful submission.
    async fn submit_single_tx(
        state: &Arc<AuthorityState>,
        consensus_adapter: &Arc<ConsensusAdapter>,
        metrics: &Arc<ValidatorServiceMetrics>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        soft_locks: &Arc<PreConsensusSoftLocks>,
        transaction: TransactionEnvelope,
    ) -> (TxStatusUpdate, Weight) {
        let tx_digest = *transaction.digest();

        let build_executed = |effects: TransactionEffects| -> TxStatusUpdate {
            let effects_digest = effects.digest();
            if let Err(error) = state.check_effects_against_previously_signed(
                epoch_store,
                &tx_digest,
                &effects_digest,
                "submit_tx",
            ) {
                return TxStatusUpdate::Rejected { error };
            }
            TxStatusUpdate::Executed {
                effects_digest,
                details: Some(Self::build_executed_data(state, &effects)),
            }
        };

        // Take this transaction's slot in the consensus queue and run the
        // overload checks against the queue at that moment, which also counts
        // admissions still in progress. Returning before the submission below
        // drops the slot and releases it.
        let slot = match state.reserve_consensus_queue_slot(consensus_adapter, transaction.data()) {
            Ok(slot) => slot,
            Err(e) => return overload_rejection(metrics, e),
        };

        // Validate transaction.
        if let Err(e) = transaction.validity_check(&epoch_store.tx_validity_check_context()) {
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

        // Suppress duplicate resubmissions of a transaction that is still in
        // flight on this validator (its soft locks are held). Checked before
        // signature verification so resubmission storms are short-circuited
        // cheaply; the soft-lock acquisition below remains the authoritative,
        // atomic gate for duplicates racing past this probe.
        if soft_locks.check_in_flight(&tx_digest) {
            metrics.num_rejected_tx_recently_resubmitted.inc();
            let error = IotaError::RecentlyResubmitted { digest: tx_digest };
            let weight = normalize(&error);
            return (TxStatusUpdate::Rejected { error }, weight);
        }

        // Every validator drops a cut-off validator's attested transactions
        // for the rest of the epoch, so it rejects them here and the client
        // submits through another validator. Before the signature check,
        // which a rejected submission need not pay for.
        //
        // Tests register the `attest-despite-cutoff` fail point to make a
        // cut-off validator attest anyway, as a faulty validator can. A `Cell`
        // keeps the binding immutable in builds where the fail point compiles
        // to nothing.
        let rejects_when_cut_off = std::cell::Cell::new(true);
        iota_macros::fail_point_if!("attest-despite-cutoff", || {
            rejects_when_cut_off.set(false)
        });
        if rejects_when_cut_off.get() {
            if let Some(attestor) = epoch_store.own_index_if_cut_off() {
                let error = IotaError::AttestorCutOff { attestor };
                let weight = normalize(&error);
                return (TxStatusUpdate::Rejected { error }, weight);
            }
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
        if !admits_user_transactions(&epoch_store.get_reconfig_state_read_lock_guard()) {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            let error = IotaError::ValidatorHaltedAtEpochEnd;
            let weight = normalize(&error);
            return (TxStatusUpdate::Rejected { error }, weight);
        }

        // Deny-rule source for admission. With governance enabled, admission
        // checks the active governance rules on top of the local config, so
        // transactions the network will deterministically drop post-consensus
        // don't occupy consensus slots. The local config always applies.
        let governance_rules = epoch_store
            .protocol_config()
            .enable_pcool_flow()
            .then(|| epoch_store.get_active_transaction_deny_rules());

        // With validator attestation enabled, `attest_transaction` runs the
        // deny checks, object loading, input checks, coin deny list and a full
        // dry run in one call and produces the attestation payload; otherwise
        // the validation-only function runs. The attestation may take the soft
        // locks itself, before its dry run.
        let (attestation_data, owned_objects, verified_tx, soft_lock) = if !checks_admitted_inputs()
        {
            (None, Vec::new(), verified_tx, None)
        } else if epoch_store.protocol_config().enable_validator_attestation() {
            let permit = match state.acquire_attestation_permit().await {
                Ok(permit) => permit,
                Err(e) => return overload_rejection(metrics, e),
            };
            let state_for_attest = state.clone();
            let epoch_store_for_attest = epoch_store.clone();
            let governance_for_attest = governance_rules.clone();
            let soft_locks_for_attest = soft_locks.clone();
            let metrics_for_attest = metrics.clone();
            let join_result = tokio::task::spawn_blocking(move || {
                // Moved in so that the dry run frees its place when it ends,
                // also when it panics.
                let _permit = permit;
                // Built inside the closure: `DenyRuleUnion` borrows, and
                // `spawn_blocking` requires `'static` captures.
                let local_deny_config = &state_for_attest.config.transaction_deny_config;
                let combined_deny_config;
                let deny_config: &dyn DenyRuleConfig = match governance_for_attest.as_ref() {
                    Some(rules) => {
                        combined_deny_config = DenyRuleUnion {
                            first: local_deny_config,
                            second: rules.as_ref(),
                        };
                        &combined_deny_config
                    }
                    None => local_deny_config,
                };
                let mut soft_lock = None;
                let result = state_for_attest.attest_transaction(
                    &verified_tx,
                    &epoch_store_for_attest,
                    deny_config,
                    |owned_objects| {
                        let guard = soft_locks_for_attest
                            .try_acquire_guard(tx_digest, owned_objects)
                            .inspect_err(|error| {
                                count_soft_lock_refusal(&metrics_for_attest, error)
                            })?;
                        soft_lock = Some(guard);
                        Ok(())
                    },
                );
                (result, verified_tx, soft_lock)
            })
            .await;
            let (result, verified_tx, soft_lock) = match join_result {
                Ok(outputs) => outputs,
                Err(join_err) => {
                    // A panic inside `attest_transaction` (e.g., an
                    // arithmetic panic in Move VM dry-run) surfaces here
                    // as a `JoinError`. Convert to a rejection rather
                    // than re-panicking on the per-tx task.
                    tracing::error!(?tx_digest, "attest_transaction task failed: {join_err}");
                    let err = IotaError::GenericAuthority {
                        error: format!("attest_transaction task failed: {join_err}"),
                    };
                    let weight = normalize(&err);
                    return (TxStatusUpdate::Rejected { error: err }, weight);
                }
            };
            match result {
                Ok((data, objs)) => (Some(data), objs, verified_tx, soft_lock),
                Err(e) => {
                    let weight = normalize(&e);
                    return (TxStatusUpdate::Rejected { error: e }, weight);
                }
            }
        } else {
            let local_deny_config = &state.config.transaction_deny_config;
            let combined_deny_config;
            let deny_config: &dyn DenyRuleConfig = match governance_rules.as_ref() {
                Some(rules) => {
                    combined_deny_config = DenyRuleUnion {
                        first: local_deny_config,
                        second: rules.as_ref(),
                    };
                    &combined_deny_config
                }
                None => local_deny_config,
            };
            match state
                .handle_transaction_validation_checks(&verified_tx, epoch_store, deny_config)
                .await
            {
                Ok(objs) => (None, objs, verified_tx, None),
                Err(e) => {
                    let weight = normalize(&e);
                    return (TxStatusUpdate::Rejected { error: e }, weight);
                }
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

        // Reconfig check. The guard is held through consensus submission below.
        let reconfiguration_lock = epoch_store.get_reconfig_state_read_lock_guard();
        if !admits_user_transactions(&reconfiguration_lock) {
            metrics.num_rejected_tx_in_epoch_boundary.inc();
            let error = IotaError::ValidatorHaltedAtEpochEnd;
            let weight = normalize(&error);
            return (TxStatusUpdate::Rejected { error }, weight);
        }

        // Build the consensus transaction.
        let consensus_tx = if let Some(attestation_data) = attestation_data {
            // submit_single_tx runs on a validator, so it must be present in its
            // own committee. Treat absence as a fatal bug, not a user error.
            let authority_index = epoch_store
                .committee()
                .authority_index(&state.name)
                .expect("validator must be present in its own consensus committee");
            let authority_index = u8::try_from(authority_index)
                .expect("consensus committee indices should fit in a u8");

            ConsensusTransaction::new_attested_user_transaction(AttestedTransaction::new(
                verified_tx.into_inner(),
                Some(Attestation::new_authority(
                    attestation_data,
                    authority_index,
                )),
            ))
        } else {
            ConsensusTransaction::new_user_transaction(verified_tx.into_inner())
        };

        // The attestation adds object references for the shared inputs and the
        // objects the dry run loaded, after the transaction passed its own size
        // check, so a valid transaction can become larger than consensus
        // accepts. The consensus adapter refuses such a transaction as well,
        // but only after the soft locks below are taken, so it is
        // rejected here. The rejection weighs: the dry run has already run, and
        // with no soft lock kept, the in-flight check does not stop a resend.
        if let Err(error) =
            check_consensus_transaction_size(&consensus_tx, epoch_store.protocol_config())
        {
            return (TxStatusUpdate::Rejected { error }, Weight::one());
        }

        // Soft-lock owned objects to prevent conflicting transactions, unless
        // the attestation took the locks before its dry run.
        // Same-digest resubmission while in flight → rejected (duplicate).
        // Different-digest conflict on same objects → rejected.
        // A rejection from here on drops the guard, which releases the locks.
        // After the size check, so a transaction too large for consensus takes
        // no new lock.
        let soft_lock = match soft_lock {
            Some(soft_lock) => soft_lock,
            None => match soft_locks.try_acquire_guard(tx_digest, &owned_objects) {
                Ok(soft_lock) => soft_lock,
                Err(error) => {
                    count_soft_lock_refusal(metrics, &error);
                    let weight = normalize(&error);
                    return (TxStatusUpdate::Rejected { error }, weight);
                }
            },
        };
        metrics
            .soft_lock_table_size
            .set(soft_locks.lock_count() as i64);

        // Submit to consensus. The slot stays counted until consensus has
        // processed the transaction, and any other outcome releases the slot.
        if let Err(e) = consensus_adapter.submit_with_slot(
            consensus_tx,
            Some(&reconfiguration_lock),
            epoch_store,
            slot,
        ) {
            let weight = normalize(&e);
            // Release soft locks so the transaction can be retried.
            tracing::debug!(
                ?tx_digest,
                error = ?e,
                "releasing soft lock after consensus submit failure",
            );
            drop(soft_lock);
            metrics
                .soft_lock_table_size
                .set(soft_locks.lock_count() as i64);
            return (TxStatusUpdate::Rejected { error: e }, weight);
        }
        soft_lock.keep();

        (TxStatusUpdate::Submitted, Weight::zero())
    }

    /// Waits for one or more previously submitted transactions to reach
    /// finality and streams one status per digest: Executed, Rejected,
    /// Expired when the epoch ended first, or TimedOut when the wait ended
    /// first.
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
            epoch_store.protocol_config().enable_pcool_flow(),
            IotaError::UnsupportedFeature {
                error: "P-COOL flow is not enabled in this protocol version".to_string()
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
                tokio::select! {
                    biased;
                    _ = tx_sender.closed() => {}
                    (update, weight) = Self::wait_for_tx_finality(
                        &state,
                        &epoch_store,
                        query.transaction_digest,
                        query.include_details,
                    ) => {
                        let _ = tx_sender
                            .send(Ok(((query.transaction_digest, update), weight)))
                            .await;
                    }
                }
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
                    Self::build_executed_update(state, epoch_store, effects, include_details),
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
                    effects_digests = cache.notify_read_executed_effects_digests("ValidatorService::notify_read_executed_effects_digests", &digests_to_watch) => {
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
            Ok(Err(_)) => TxStatusUpdate::Expired {
                epoch: epoch_store.epoch(),
            },
            Ok(Ok(Either::Left(effects_digests))) => {
                let Some(effects_digest) = effects_digests.into_iter().next() else {
                    // The notifier returns one digest per requested digest.
                    debug_fatal!("empty effects from notify_read for {tx_digest:?}");
                    return (TxStatusUpdate::TimedOut, Weight::zero());
                };
                if let Err(error) = state.check_effects_against_previously_signed(
                    epoch_store,
                    &tx_digest,
                    &effects_digest,
                    "get_tx_status",
                ) {
                    return (TxStatusUpdate::Rejected { error }, Weight::zero());
                }
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
            Err(_timeout) => TxStatusUpdate::TimedOut,
        };
        (update, Weight::zero())
    }

    /// Builds a `TxStatusUpdate::Executed` from known effects, optionally
    /// including full details.
    fn build_executed_update(
        state: &Arc<AuthorityState>,
        epoch_store: &AuthorityPerEpochStore,
        effects: TransactionEffects,
        include_details: bool,
    ) -> TxStatusUpdate {
        let effects_digest = effects.digest();
        if let Err(error) = state.check_effects_against_previously_signed(
            epoch_store,
            effects.transaction_digest(),
            &effects_digest,
            "get_tx_status",
        ) {
            return TxStatusUpdate::Rejected { error };
        }
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
                    .execution_scheduler()
                    .num_pending_transactions()
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
        self.post_handle_stream(
            ip,
            self.submit_tx_impl(req).await,
            |(_, update)| match update {
                TxStatusUpdate::Rejected { error } => Some(error.clone()),
                _ => None,
            },
        )
    }

    type GetTxStatusStream = StreamResponse<TxStatus>;

    async fn get_tx_status(
        &self,
        request: tonic::Request<GetTxStatusRequest>,
    ) -> Result<tonic::Response<Self::GetTxStatusStream>, tonic::Status> {
        let (req, ip) = self.pre_handle(request).await?;
        // A rejection a status query reports is the outcome of an earlier
        // submission, not an error in this request.
        self.post_handle_stream(ip, self.get_tx_status_impl(req).await, |_| None)
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

#[cfg(test)]
#[path = "../unit_tests/validator_v2_tests.rs"]
mod validator_v2_tests;

#[cfg(test)]
mod tests {
    use iota_sdk_types::{Address, DenyRuleSet, ObjectId};
    use iota_types::deny_rule_governance::DenyRuleConfig;

    use super::DenyRuleUnion;

    /// The union denies whatever either source denies.
    #[test]
    fn deny_rule_union_denies_from_either_source() {
        let first = DenyRuleSet {
            denied_addresses: [Address::new([1u8; 32])].into(),
            user_transaction_disabled: true,
            ..Default::default()
        };
        let second = DenyRuleSet {
            denied_objects: [ObjectId::new([2u8; 32])].into(),
            shared_object_disabled: true,
            ..Default::default()
        };
        let union = DenyRuleUnion {
            first: &first,
            second: &second,
        };
        assert!(union.is_address_denied(&Address::new([1u8; 32])));
        assert!(union.is_object_denied(&ObjectId::new([2u8; 32])));
        assert!(!union.is_package_denied(&ObjectId::new([3u8; 32])));
        assert!(union.has_denied_addresses());
        assert!(union.has_denied_objects());
        assert!(!union.has_denied_packages());
        assert!(union.user_transaction_disabled());
        assert!(union.shared_object_disabled());
        assert!(!union.package_publish_disabled());
    }
}
