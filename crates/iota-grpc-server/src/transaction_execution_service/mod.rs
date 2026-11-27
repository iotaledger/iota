// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod simulate;
mod transaction;
mod view;

use std::{sync::Arc, time::Duration};

use futures::stream::{FuturesUnordered, StreamExt as _};
use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    proto::timestamp_ms_to_proto,
    read_masks::EXECUTE_TRANSACTIONS_READ_MASK,
    v1::{
        error_reason::ErrorReason,
        transaction::{ExecutedTransaction, Transaction as ProtoTransaction},
        transaction_execution_service::{
            self as grpc_tx_service, ExecuteTransactionItem, ExecuteTransactionResult,
            ExecuteTransactionsRequest, ExecuteTransactionsResponse, SimulateTransactionsRequest,
            SimulateTransactionsResponse, ViewFunctionCallsRequest, ViewFunctionCallsResponse,
            execute_transaction_result, simulate_transaction_result, view_function_call_result,
        },
    },
};
use iota_node_transaction_builder::NodeTransactionBuilderLedgerClient;
use iota_sdk_types::TransactionDigest;
use iota_types::{
    effects::TransactionEffectsAPI,
    error::IotaError,
    quorum_driver_types::{ExecuteTransactionRequestV1, ExecuteTransactionResponseV1},
    transaction_executor::{SimulateTransactionResult, TransactionExecutor, VmChecks},
};
use tonic::{Request, Response};
pub(crate) use transaction::CommandOutputsReadSource;
pub use transaction::{CommandResultsReadSource, TransactionReadSource};

use crate::{
    error::RpcError, merge::Merge, traffic_control::TallyHandle, types::GrpcReader,
    validation::validate_read_mask,
};

pub struct TransactionExecutionGrpcService {
    pub config: iota_config::node::GrpcApiConfig,
    pub reader: Arc<GrpcReader>,
    pub executor: Arc<dyn TransactionExecutor>,
    pub builder_client: NodeTransactionBuilderLedgerClient,
}

impl TransactionExecutionGrpcService {
    pub fn new(
        config: iota_config::node::GrpcApiConfig,
        reader: Arc<GrpcReader>,
        executor: Arc<dyn TransactionExecutor>,
    ) -> Self {
        let builder_client = NodeTransactionBuilderLedgerClient::new(reader.state_reader());
        Self {
            config,
            reader,
            executor,
            builder_client,
        }
    }
}

#[tonic::async_trait]
impl grpc_tx_service::transaction_execution_service_server::TransactionExecutionService
    for TransactionExecutionGrpcService
{
    async fn execute_transactions(
        &self,
        request: Request<ExecuteTransactionsRequest>,
    ) -> Result<Response<ExecuteTransactionsResponse>, tonic::Status> {
        let tally_handle = request.extensions().get::<TallyHandle>().cloned();
        let response = execute_transactions(
            &self.reader,
            &self.executor,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(tonic::Status::from)?;
        // Each batch item is invisible to the transport-level traffic control:
        // account for every item so a batch is charged per item, and its
        // embedded per-item errors feed the error policy.
        if let Some(tally_handle) = &tally_handle {
            tally_handle.tally_items(response.get_ref().transaction_results.iter().map(|result| {
                match &result.result {
                    Some(execute_transaction_result::Result::Error(error)) => {
                        tonic::Code::from(error.code)
                    }
                    _ => tonic::Code::Ok,
                }
            }));
        }
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn simulate_transactions(
        &self,
        request: Request<SimulateTransactionsRequest>,
    ) -> Result<Response<SimulateTransactionsResponse>, tonic::Status> {
        let tally_handle = request.extensions().get::<TallyHandle>().cloned();
        let response = simulate::simulate_transactions(
            &self.reader,
            &self.executor,
            tally_handle.as_ref(),
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(tonic::Status::from)?;
        // Each batch item is invisible to the transport-level traffic control:
        // account for every item so a batch is charged per item, and its
        // embedded per-item errors feed the error policy.
        if let Some(tally_handle) = &tally_handle {
            tally_handle.tally_items(response.get_ref().transaction_results.iter().map(|result| {
                match &result.result {
                    Some(simulate_transaction_result::Result::Error(error)) => {
                        tonic::Code::from(error.code)
                    }
                    _ => tonic::Code::Ok,
                }
            }));
        }
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn view_function_calls(
        &self,
        request: Request<ViewFunctionCallsRequest>,
    ) -> Result<Response<ViewFunctionCallsResponse>, tonic::Status> {
        let tally_handle = request.extensions().get::<TallyHandle>().cloned();
        let response = view::view_function_calls(
            &self.reader,
            &self.executor,
            tally_handle.as_ref(),
            &self.builder_client,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(tonic::Status::from)?;
        // Each batch item is invisible to the transport-level traffic control:
        // account for every item so a batch is charged per item, and its
        // embedded per-item errors feed the error policy.
        if let Some(tally_handle) = &tally_handle {
            tally_handle.tally_items(response.get_ref().call_results.iter().map(|result| {
                match &result.result {
                    Some(view_function_call_result::Result::Error(error)) => {
                        tonic::Code::from(error.code)
                    }
                    _ => tonic::Code::Ok,
                }
            }));
        }
        Ok(append_info_headers!(response, self.reader.clone()))
    }
}

// === Shared helpers for execute and simulate ===

/// Simulates `transaction` on the blocking thread pool, as a simulation can
/// run long enough to stall the requests that share a serving thread with it.
/// Refused with `RESOURCE_EXHAUSTED` while the client of `tally_handle` is over
/// its simulation gas limit, or while the executor runs as many simulations as
/// it allows. Neither refusal counts as a client error.
async fn run_simulation(
    executor: &Arc<dyn TransactionExecutor>,
    tally_handle: Option<&TallyHandle>,
    transaction: iota_sdk_types::Transaction,
    checks: VmChecks,
) -> Result<SimulateTransactionResult, RpcError> {
    let reservation = match tally_handle {
        Some(tally_handle) => tally_handle.reserve_simulation_gas()?,
        None => None,
    };
    let permit = executor.try_acquire_simulation_permit().ok_or_else(|| {
        RpcError::new(
            tonic::Code::ResourceExhausted,
            "the node is running as many simulations as it allows, retry later",
        )
    })?;
    let executor = executor.clone();
    tokio::task::spawn_blocking(move || {
        // Held until the simulation ends, even if the request is dropped
        // before.
        let _permit = permit;
        let simulation = executor.simulate_transaction(transaction, checks);
        // A simulation that returns an error did not execute, so it gets its
        // whole reservation back when the reservation is dropped.
        if let (Some(reservation), Ok(simulation)) = (reservation, &simulation) {
            reservation.settle(
                simulation.effects.gas_cost_summary().computation_cost,
                simulation.gas_data.price,
            );
        }
        simulation
    })
    .await
    .map_err(|e| {
        RpcError::new(
            tonic::Code::Internal,
            format!("transaction simulation task failed: {e}"),
        )
    })?
    .map_err(|e| {
        RpcError::new(
            if matches!(e, IotaError::UserInput { .. }) {
                tonic::Code::InvalidArgument
            } else {
                tonic::Code::Internal
            },
            format!("transaction simulation failed: {e}"),
        )
    })
}

/// Validate that a batch is non-empty and within the size limit.
fn validate_batch_size(items_len: usize, max_batch: usize) -> Result<(), RpcError> {
    if items_len == 0 {
        return Err(RpcError::new(
            tonic::Code::InvalidArgument,
            "batch must not be empty",
        ));
    }
    if items_len > max_batch {
        return Err(RpcError::new(
            tonic::Code::InvalidArgument,
            format!("batch size {items_len} exceeds maximum allowed ({max_batch})"),
        ));
    }
    Ok(())
}

/// Extract, deserialize, and validate a transaction from its proto
/// representation.
///
/// This performs the common validation steps shared by both execute and
/// simulate:
/// 1. Ensure the transaction field is present
/// 2. Extract and deserialize the BCS data
/// 3. Validate the digest if provided
fn parse_transaction_proto(
    transaction: Option<&ProtoTransaction>,
) -> Result<iota_sdk_types::Transaction, RpcError> {
    let transaction_proto = transaction
        .ok_or_else(|| FieldViolation::new("transaction").with_reason(ErrorReason::FieldMissing))?;

    let transaction_bcs = transaction_proto.bcs.as_ref().ok_or_else(|| {
        FieldViolation::new("transaction.bcs")
            .with_description("transaction BCS is required")
            .with_reason(ErrorReason::FieldMissing)
    })?;

    let sdk_transaction: iota_sdk_types::Transaction = bcs::from_bytes(&transaction_bcs.data)
        .map_err(|e| {
            FieldViolation::new("transaction.bcs")
                .with_description(format!("invalid transaction BCS: {e}"))
                .with_reason(ErrorReason::FieldInvalid)
        })?;

    if let Some(provided_digest) = &transaction_proto.digest {
        let computed_digest = sdk_transaction.digest();
        let provided_digest_bytes: [u8; 32] =
            provided_digest.digest.as_ref().try_into().map_err(|_| {
                FieldViolation::new("transaction.digest")
                    .with_description("digest must be exactly 32 bytes")
                    .with_reason(ErrorReason::FieldInvalid)
            })?;

        if computed_digest.bytes() != &provided_digest_bytes {
            let provided_digest_typed = iota_sdk_types::Digest::new(provided_digest_bytes);
            return Err(FieldViolation::new("transaction.digest")
                .with_description(format!(
                    "provided digest does not match computed digest: provided={provided_digest_typed}, computed={computed_digest}"
                ))
                .with_reason(ErrorReason::FieldInvalid)
                .into());
        }
    }

    Ok(sdk_transaction)
}

/// Execute a batch of transactions concurrently.
///
/// Each transaction is executed independently — failure of one does not abort
/// the rest. Results are returned in the same order as the input, but the
/// relative execution order of the transactions is not guaranteed.
///
/// ## Checkpoint Inclusion
///
/// If `checkpoint_inclusion_timeout_ms` is set in the request, the server will
/// wait (after executing all transactions) for them to be included in a
/// checkpoint. On success the `checkpoint` and `timestamp` fields of each
/// `ExecutedTransaction` are populated. If the timeout expires, partial results
/// are returned: transactions that were already checkpointed will have the
/// fields set, while the rest will have them unset.
///
/// Note: `checkpoint` and `timestamp` must also be included in the `read_mask`
/// for them to appear in the response.
///
/// ## Read Mask
///
/// The read mask paths apply directly to
/// [`ExecutedTransaction`]
/// fields (e.g. `"effects"`, not `"executed_transaction.effects"`).
///
/// ## Available Read Mask Fields
///
/// The `execute_transactions` function supports the following `read_mask`
/// fields to control which data is included in each `ExecutedTransaction`
/// result:
///
/// ## Transaction Fields
/// - `transaction` - includes all transaction fields
///   - `transaction.digest` - the transaction digest
///   - `transaction.bcs` - the full BCS-encoded transaction
/// - `signatures` - includes all signature fields
///   - `signatures.bcs` - the full BCS-encoded signature
/// - `effects` - includes all effects fields
///   - `effects.digest` - the effects digest
///   - `effects.bcs` - the full BCS-encoded effects
///
/// ## Event Fields
/// - `events` - includes all event fields (all events of the transaction)
///   - `events.digest` - the events digest
///   - `events.events.bcs` - the full BCS-encoded event
///   - `events.events.package_id` - the ID of the package that emitted the event
///   - `events.events.module` - the module that emitted the event
///   - `events.events.sender` - the sender that triggered the event
///   - `events.events.event_type` - the type of the event
///   - `events.events.bcs_contents` - the full BCS-encoded contents of the event
///   - `events.events.json_contents` - the JSON-encoded contents of the event
///
/// ## Checkpoint Fields
/// - `checkpoint` - the checkpoint that included the transaction. Requires
///   `checkpoint_inclusion_timeout_ms` to be set.
/// - `timestamp` - the timestamp of the checkpoint. Requires `checkpoint_inclusion_timeout_ms` to
///   be set.
///
/// ## Object Fields
/// - `input_objects` - includes all input object fields
///   - `input_objects.reference` - includes all reference fields
///     - `input_objects.reference.object_id` - the ID of the input object
///     - `input_objects.reference.version` - the version of the input object
///     - `input_objects.reference.digest` - the digest of the input object contents
///   - `input_objects.bcs` - the full BCS-encoded object
/// - `output_objects` - includes all output object fields
///   - `output_objects.reference` - includes all reference fields
///     - `output_objects.reference.object_id` - the ID of the output object
///     - `output_objects.reference.version` - the version of the output object
///     - `output_objects.reference.digest` - the digest of the output object contents
///   - `output_objects.bcs` - the full BCS-encoded object
///
/// ## Derived Change Fields
/// Derived from the transaction's effects and input/output objects.
/// - `balance_changes` - per-owner, per-coin-type balance deltas. For a failed transaction this
///   contains only the gas charge.
/// - `object_changes` - structured object changes (created, mutated, deleted, wrapped, unwrapped,
///   published)
#[tracing::instrument(skip_all, fields(batch_size = request.transactions.len()))]
pub async fn execute_transactions(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    request: ExecuteTransactionsRequest,
) -> Result<ExecuteTransactionsResponse, RpcError> {
    validate_batch_size(
        request.transactions.len(),
        config.max_execute_transaction_batch_size as usize,
    )?;
    let read_mask = validate_read_mask::<ExecutedTransaction>(
        request.read_mask,
        EXECUTE_TRANSACTIONS_READ_MASK,
    )?;

    // Parse and clamp checkpoint inclusion timeout.
    // If None or 0 is provided, we won't wait for checkpoint inclusion and the
    // response will be returned immediately after execution with
    // checkpoint/timestamp fields unset. If a positive value is provided, the
    // server will wait up to the specified duration for the transaction to be
    // included in a checkpoint before returning. The timeout is clamped by the
    // server's max_checkpoint_inclusion_timeout_ms config to prevent excessively
    // long waits.
    let checkpoint_timeout = request
        .checkpoint_inclusion_timeout_ms
        .filter(|&ms| ms > 0)
        .map(|ms| Duration::from_millis(ms.min(config.max_checkpoint_inclusion_timeout_ms)));

    // If the client opted into waiting for checkpoint inclusion, the
    // subsequent `wait_for_checkpoint_inclusion` call below provides
    // finality, so we instruct the executor to skip its 2f+1 effects
    // certification. Otherwise the executor performs the full certification.
    let skip_certification = checkpoint_timeout.is_some();
    let flags = ReadFlags::from_mask(&read_mask);
    // The input and output objects are read from the cache once a transaction
    // is checkpointed: when finalizing after the inclusion wait, and for a
    // duplicate submission whose first submission's driver was cancelled.
    let _cached_object_retention = (flags.include_input_objects || flags.include_output_objects)
        .then(|| executor.retain_cached_objects());

    // Drive the per-tx executions concurrently so a slow validator submission
    // on one item doesn't stall the rest of the batch. Results are collected
    // back into per-index slots so the response order matches the request
    // order regardless of completion order. The post-batch
    // `wait_for_checkpoint_inclusion` + `finalize_item` gate still applies
    // unchanged — uncertified single-validator data is never returned to the
    // client without checkpoint corroboration (or a `DeadlineExceeded` error
    // in its place).
    let mut transaction_results: Vec<ExecuteTransactionResult> = (0..request.transactions.len())
        .map(|_| ExecuteTransactionResult::default())
        .collect();
    // For each successful execution: (index, digest, rebuild_ctx). `rebuild_ctx`
    // is `Some` only when the response carried uncertified single-validator
    // data and must be rebuilt from cache after checkpoint inclusion.
    let mut successful_digests: Vec<(usize, TransactionDigest, Option<RebuildCtx>)> = Vec::new();
    let read_mask_ref = &read_mask;
    let mut driver_futs: FuturesUnordered<_> = request
        .transactions
        .iter()
        .enumerate()
        .map(|(i, item)| async move {
            let result = execute_single_transaction(
                reader,
                executor,
                config,
                item,
                read_mask_ref,
                skip_certification,
            )
            .await;
            (i, result)
        })
        .collect();
    while let Some((i, result)) = driver_futs.next().await {
        transaction_results[i] = match result {
            Ok((digest, tx, rebuild_ctx)) => {
                successful_digests.push((i, digest, rebuild_ctx));
                ExecuteTransactionResult::default().with_executed_transaction(tx)
            }
            Err(error) => ExecuteTransactionResult::default().with_error(error.into_status_proto()),
        };
    }

    // Optionally wait for checkpoint inclusion, then finalize each successful
    // item: rebuild from cache for skip-effect-cert items (so we don't return
    // uncertified single-validator data) or patch checkpoint/timestamp on the
    // already-built response for the cert path.
    if let (Some(timeout), false) = (checkpoint_timeout, successful_digests.is_empty()) {
        let digests: Vec<_> = successful_digests.iter().map(|(_, d, _)| *d).collect();
        let (checkpoint_map, wait_error) = match executor
            .wait_for_checkpoint_inclusion(&digests, timeout)
            .await
        {
            Ok(m) => (m, None),
            Err(e) => {
                tracing::warn!("wait_for_checkpoint_inclusion failed: {e}");
                (std::collections::BTreeMap::new(), Some(format!("{e}")))
            }
        };
        let flags_ref = &flags;
        let read_mask_ref = &read_mask;
        let wait_error_ref = wait_error.as_deref();

        // Run per-tx finalization (which calls into `rebuild_from_cache` →
        // cache reads + object derivation) in parallel; results land back
        // in per-index slots so the response order matches the request.
        let mut finalize_futs: FuturesUnordered<_> = successful_digests
            .into_iter()
            .map(|(i, digest, rebuild_ctx)| {
                let original = std::mem::take(&mut transaction_results[i]);
                let checkpoint_entry = checkpoint_map.get(&digest).copied();
                async move {
                    let result = finalize_item(
                        reader,
                        executor,
                        config,
                        read_mask_ref,
                        flags_ref,
                        original,
                        &digest,
                        rebuild_ctx,
                        checkpoint_entry,
                        wait_error_ref,
                    )
                    .await;
                    (i, result)
                }
            })
            .collect();
        while let Some((i, result)) = finalize_futs.next().await {
            transaction_results[i] = result;
        }
    }

    Ok(ExecuteTransactionsResponse::default().with_transaction_results(transaction_results))
}

/// Read-mask flags extracted once and passed into the per-item finalizer.
struct ReadFlags {
    include_events: bool,
    include_input_objects: bool,
    include_output_objects: bool,
    needs_checkpoint: bool,
    needs_timestamp: bool,
}

impl ReadFlags {
    fn from_mask(mask: &FieldMaskTree) -> Self {
        // Balance/object changes are derived from the input/output objects, so
        // the executor must return them whenever the derived fields are
        // requested.
        let derives_changes = mask.contains(ExecutedTransaction::BALANCE_CHANGES_FIELD.name)
            || mask.contains(ExecutedTransaction::OBJECT_CHANGES_FIELD.name);

        Self {
            include_events: mask.contains(ExecutedTransaction::EVENTS_FIELD.name),
            include_input_objects: mask.contains(ExecutedTransaction::INPUT_OBJECTS_FIELD.name)
                || derives_changes,
            include_output_objects: mask.contains(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name)
                || derives_changes,
            needs_checkpoint: mask.contains(ExecutedTransaction::CHECKPOINT_FIELD.name),
            needs_timestamp: mask.contains(ExecutedTransaction::TIMESTAMP_FIELD.name),
        }
    }
}

/// Patch `checkpoint` / `timestamp` on an already-built executed transaction
/// result when requested by the read mask. No-op if the result isn't an
/// `ExecutedTransaction`.
fn patch_checkpoint_timestamp(
    result: &mut ExecuteTransactionResult,
    seq: u64,
    ts_ms: u64,
    flags: &ReadFlags,
) {
    if let Some(execute_transaction_result::Result::ExecutedTransaction(ref mut tx)) = result.result
    {
        if flags.needs_checkpoint {
            tx.checkpoint = Some(seq);
        }
        if flags.needs_timestamp && ts_ms > 0 {
            tx.timestamp = Some(timestamp_ms_to_proto(ts_ms));
        }
    }
}

fn error_result(code: tonic::Code, message: String) -> ExecuteTransactionResult {
    ExecuteTransactionResult::default().with_error(RpcError::new(code, message).into_status_proto())
}

/// Produce the final result for one item after the checkpoint wait. Handles
/// all four combinations of (cert vs skip-cert) × (checkpointed vs not).
#[allow(clippy::too_many_arguments)]
async fn finalize_item(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    read_mask: &FieldMaskTree,
    flags: &ReadFlags,
    mut original: ExecuteTransactionResult,
    digest: &TransactionDigest,
    rebuild_ctx: Option<RebuildCtx>,
    checkpoint_entry: Option<(u64, u64)>,
    wait_error: Option<&str>,
) -> ExecuteTransactionResult {
    let Some((seq, ts)) = checkpoint_entry else {
        // Not checkpointed within the timeout (or the wait call itself failed).
        // Cert path: leave the 2f+1-safe response as-is; the client can poll
        // for the checkpoint/timestamp fields later. Skip-effect-cert path:
        // the response holds uncertified single-validator data — return an
        // error instead of leaking it.
        if rebuild_ctx.is_none() {
            return original;
        }
        return match wait_error {
            Some(e) => error_result(
                tonic::Code::Internal,
                format!("wait_for_checkpoint_inclusion failed for tx {digest:?}: {e}"),
            ),
            None => error_result(
                tonic::Code::DeadlineExceeded,
                format!(
                    "transaction {digest:?} was submitted but not included in a checkpoint \
                     within the timeout"
                ),
            ),
        };
    };

    let Some(ctx) = rebuild_ctx else {
        // Cert path, checkpointed: just patch checkpoint/timestamp.
        patch_checkpoint_timestamp(&mut original, seq, ts, flags);
        return original;
    };

    // Was the original (single-validator) response claiming events? Captured
    // here so we can detect a submitter-vs-cache events disagreement after
    // the rebuild (mirrors the orchestrator's `skip_effect_cert_events_cache_miss`
    // invariant).
    let submitter_claimed_events = matches!(
        &original.result,
        Some(execute_transaction_result::Result::ExecutedTransaction(tx)) if tx.events.is_some(),
    );

    // Skip-effect-cert path, checkpointed: rebuild the response from the
    // authoritative local cache to replace the single-validator data.
    match rebuild_from_cache(
        reader, executor, config, read_mask, ctx, digest, seq, ts, flags,
    )
    .await
    {
        Ok(Some(tx)) => {
            if submitter_claimed_events && flags.include_events && tx.events.is_none() {
                tracing::warn!(
                    ?digest,
                    "submitter claimed events but local cache has none — discarding \
                     (possible byzantine submitter)"
                );
            }
            ExecuteTransactionResult::default().with_executed_transaction(tx)
        }
        Ok(None) => {
            // Executor has no cache (e.g. simulacrum). The skip-cert path
            // here means `original` still carries uncertified single-validator
            // data — never return that to the client. In production the
            // orchestrator impl always returns `Some`; an executor that opts
            // into `skip_certification` without providing a cache is a
            // configuration error.
            tracing::error!(
                ?digest,
                "skip-cert finalize: executor returned no cached data despite checkpoint \
                 inclusion — refusing to return uncertified response"
            );
            error_result(
                tonic::Code::Internal,
                format!(
                    "executor cannot rebuild tx {digest:?} from local cache; \
                     skip-certification requires a cache-backed executor"
                ),
            )
        }
        Err(e) => {
            tracing::warn!(?digest, "failed to rebuild executed tx from cache: {e}");
            error_result(
                tonic::Code::Internal,
                format!(
                    "failed to rebuild tx {digest:?} from local cache after checkpoint \
                     inclusion: {e}"
                ),
            )
        }
    }
}

/// Pre-parsed transaction identity carried forward from
/// `execute_single_transaction` so that `rebuild_from_cache` does not need to
/// re-parse the proto request.
struct RebuildCtx {
    transaction: iota_sdk_types::Transaction,
    signatures: Vec<iota_sdk_types::UserSignature>,
}

/// Rebuild an `ExecutedTransaction` from the local cache for a tx that has
/// just been observed in a checkpoint. Returns `Ok(None)` if the executor
/// does not have cache data for this tx (e.g. simulacrum).
async fn rebuild_from_cache(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    read_mask: &FieldMaskTree,
    ctx: RebuildCtx,
    digest: &TransactionDigest,
    checkpoint_seq: u64,
    checkpoint_ts_ms: u64,
    flags: &ReadFlags,
) -> Result<Option<ExecutedTransaction>, RpcError> {
    let Some(cached) = executor
        .read_transaction_from_cache(
            digest,
            flags.include_events,
            flags.include_input_objects,
            flags.include_output_objects,
        )
        .map_err(|e| {
            RpcError::new(
                tonic::Code::Internal,
                format!("failed to read tx {digest:?} from cache: {e:?}"),
            )
        })?
    else {
        return Ok(None);
    };

    let source = TransactionReadSource {
        reader: reader.clone(),
        config,
        transaction: Some(ctx.transaction),
        signatures: Some(ctx.signatures),
        effects: Some(cached.effects),
        events: cached.events,
        checkpoint: Some(checkpoint_seq),
        timestamp_ms: if checkpoint_ts_ms > 0 {
            Some(checkpoint_ts_ms)
        } else {
            None
        },
        input_objects: cached.input_objects,
        output_objects: cached.output_objects,
        mocked_coin: None,
    };

    let executed = ExecutedTransaction::merge_from(&source, read_mask)
        .map_err(|e| e.with_context("failed to merge executed transaction from cache"))?;

    Ok(Some(executed))
}

/// Validate, execute, and merge a single transaction item.
async fn execute_single_transaction(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    item: &ExecuteTransactionItem,
    read_mask: &FieldMaskTree,
    skip_certification: bool,
) -> Result<(TransactionDigest, ExecutedTransaction, Option<RebuildCtx>), RpcError> {
    let sdk_transaction = parse_transaction_proto(item.transaction.as_ref())?;

    // Extract and validate signatures
    let signatures_proto = item
        .signatures
        .as_ref()
        .ok_or_else(|| FieldViolation::new("signatures").with_reason(ErrorReason::FieldMissing))?;

    let sdk_signatures = signatures_proto
        .signatures
        .iter()
        .enumerate()
        .map(|(i, sig)| {
            let bcs_data = sig.bcs.as_ref().ok_or_else(|| {
                FieldViolation::new_at("signatures", i)
                    .with_description("signature BCS is required")
                    .with_reason(ErrorReason::FieldMissing)
            })?;

            bcs::from_bytes::<iota_sdk_types::UserSignature>(&bcs_data.data).map_err(|e| {
                FieldViolation::new_at("signatures", i)
                    .with_description(format!("invalid signature: {e}"))
                    .with_reason(ErrorReason::FieldInvalid)
            })
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Create signed transaction
    let sdk_signed_transaction = iota_sdk_types::SignedTransaction {
        transaction: sdk_transaction,
        signatures: sdk_signatures,
    };

    let transaction = iota_types::transaction::TransactionEnvelope::from(sdk_signed_transaction);

    // Determine what to include in the request based on read mask.
    // Balance/object changes are derived from the input/output objects, so the
    // executor must return them whenever the derived fields are requested.
    let derives_changes = read_mask.contains(ExecutedTransaction::BALANCE_CHANGES_FIELD.name)
        || read_mask.contains(ExecutedTransaction::OBJECT_CHANGES_FIELD.name);
    let include_events = read_mask.contains(ExecutedTransaction::EVENTS_FIELD.name);
    let include_input_objects =
        read_mask.contains(ExecutedTransaction::INPUT_OBJECTS_FIELD.name) || derives_changes;
    let include_output_objects =
        read_mask.contains(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name) || derives_changes;

    // Create execution request
    let exec_request = ExecuteTransactionRequestV1 {
        transaction: transaction.clone(),
        include_events,
        include_input_objects,
        include_output_objects,
        include_auxiliary_data: false,
    };

    // Execute the transaction
    let ExecuteTransactionResponseV1 {
        effects,
        events,
        input_objects,
        output_objects,
        auxiliary_data: _,
    } = executor
        .execute_transaction(exec_request, skip_certification, None)
        .await
        .map_err(RpcError::from)?;

    let digest = *effects.effects.transaction_digest();

    // Build the merged response
    let sdk_transaction: iota_sdk_types::Transaction = transaction.transaction().clone();
    let signatures = transaction.signatures().to_owned();

    // Keep a pre-parsed copy for the rebuild-from-cache path so it doesn't
    // have to re-parse the proto request. Only materialised when the response
    // carries uncertified single-validator data (skip-effect-cert path).
    let rebuild_ctx = matches!(
        effects.finality_info,
        iota_types::quorum_driver_types::EffectsFinalityInfo::UncertifiedSingleValidator(_)
    )
    .then(|| RebuildCtx {
        transaction: sdk_transaction.clone(),
        signatures: signatures.clone(),
    });

    let source = TransactionReadSource {
        reader: reader.clone(),
        config,
        transaction: Some(sdk_transaction),
        signatures: Some(signatures),
        effects: Some(effects.effects),
        events,
        checkpoint: None,
        timestamp_ms: None,
        input_objects,
        output_objects,
        mocked_coin: None,
    };

    let executed = ExecutedTransaction::merge_from(&source, read_mask)
        .map_err(|e| e.with_context("failed to merge executed transaction"))?;

    Ok((digest, executed, rebuild_ctx))
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr},
        sync::{Mutex, mpsc},
    };

    use iota_sdk_types::{
        Address, GasPayment, ProgrammableTransaction, Transaction, TransactionExpiration,
        TransactionKind, TransactionV1,
    };
    use iota_traffic_controller::{ClientAccess, TrafficController};
    use iota_types::{
        messages_checkpoint::CheckpointSequenceNumber,
        quorum_driver_types::QuorumDriverError,
        traffic_control::{PolicyConfig, SimulationGasLimitConfig},
    };
    use tokio::sync::{Notify, Semaphore};

    use super::*;

    /// A `TransactionExecutor` whose simulations each take one of `permits`.
    /// The first simulation waits until the sender of `gate` sends or is
    /// dropped, and every simulation fails.
    struct GatedExecutor {
        permits: Arc<Semaphore>,
        started: Notify,
        gate: Mutex<Option<mpsc::Receiver<()>>>,
    }

    #[async_trait::async_trait]
    impl TransactionExecutor for GatedExecutor {
        async fn execute_transaction(
            &self,
            _request: ExecuteTransactionRequestV1,
            _skip_certification: bool,
            _client_addr: Option<std::net::SocketAddr>,
        ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
            unreachable!("the test only simulates transactions")
        }

        fn simulate_transaction(
            &self,
            _transaction: Transaction,
            _checks: VmChecks,
        ) -> Result<SimulateTransactionResult, IotaError> {
            self.started.notify_one();
            let gate = self.gate.lock().unwrap().take();
            if let Some(gate) = gate {
                // Either a send or a dropped sender opens the gate.
                let _ = gate.recv();
            }
            Err(IotaError::Unknown("test simulations fail".to_string()))
        }

        fn try_acquire_simulation_permit(&self) -> Option<Box<dyn Send + Sync>> {
            self.permits
                .clone()
                .try_acquire_owned()
                .ok()
                .map(|permit| Box::new(permit) as Box<dyn Send + Sync>)
        }

        async fn wait_for_checkpoint_inclusion(
            &self,
            _digests: &[TransactionDigest],
            _timeout: Duration,
        ) -> Result<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>, IotaError>
        {
            unreachable!("the test only simulates transactions")
        }
    }

    fn transaction() -> Transaction {
        Transaction::V1(TransactionV1 {
            kind: TransactionKind::new_programmable(ProgrammableTransaction {
                inputs: vec![],
                commands: vec![],
            }),
            sender: Address::ZERO,
            gas_payment: GasPayment {
                objects: vec![],
                owner: Address::ZERO,
                price: 0,
                budget: 0,
            },
            expiration: TransactionExpiration::None,
        })
    }

    #[tokio::test]
    async fn a_dropped_simulation_keeps_its_permit_until_it_ends() {
        let (open_gate, gate) = mpsc::channel();
        let gated = Arc::new(GatedExecutor {
            permits: Arc::new(Semaphore::new(1)),
            started: Notify::new(),
            gate: Mutex::new(Some(gate)),
        });
        let executor: Arc<dyn TransactionExecutor> = gated.clone();

        let simulation = tokio::spawn({
            let executor = executor.clone();
            async move { run_simulation(&executor, None, transaction(), VmChecks::Enabled).await }
        });
        gated.started.notified().await;
        simulation.abort();
        assert!(matches!(simulation.await, Err(e) if e.is_cancelled()));

        let refused = run_simulation(&executor, None, transaction(), VmChecks::Enabled)
            .await
            .err()
            .map(|e| tonic::Status::from(e).code());
        assert_eq!(
            refused,
            Some(tonic::Code::ResourceExhausted),
            "the dropped simulation gave up its permit before it ended"
        );

        open_gate.send(()).unwrap();
        wait_for_a_free_permit(&gated).await;
    }

    /// The gas that a simulation reserved is held until the simulation ends,
    /// also when its request is dropped before.
    #[tokio::test]
    async fn a_simulation_whose_request_is_dropped_keeps_its_gas_until_it_ends() {
        let (open_gate, gate) = mpsc::channel();
        let gated = Arc::new(GatedExecutor {
            permits: Arc::new(Semaphore::new(1)),
            started: Notify::new(),
            gate: Mutex::new(Some(gate)),
        });
        let executor: Arc<dyn TransactionExecutor> = gated.clone();
        let traffic_controller = Arc::new(TrafficController::init_for_test(PolicyConfig {
            simulation_gas_limit: Some(SimulationGasLimitConfig {
                gas_per_sec: 1,
                burst_gas: 1_000,
            }),
            dry_run: false,
            ..Default::default()
        }));
        traffic_controller.set_max_simulation_computation(1_000);
        let tally_handle = TallyHandle::new(
            traffic_controller,
            Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            ClientAccess::Limited,
        );

        let simulation = tokio::spawn({
            let executor = executor.clone();
            let tally_handle = tally_handle.clone();
            async move {
                run_simulation(
                    &executor,
                    Some(&tally_handle),
                    transaction(),
                    VmChecks::Enabled,
                )
                .await
            }
        });
        gated.started.notified().await;
        simulation.abort();
        assert!(matches!(simulation.await, Err(e) if e.is_cancelled()));
        assert!(
            tally_handle.reserve_simulation_gas().is_err(),
            "the dropped simulation gave its gas back before it ended"
        );

        open_gate.send(()).unwrap();
        wait_for_a_free_permit(&gated).await;
        assert!(
            matches!(tally_handle.reserve_simulation_gas(), Ok(Some(_))),
            "the simulation kept its gas after it ended"
        );
    }

    /// Waits until the simulation that holds the permit of `gated` ends.
    async fn wait_for_a_free_permit(gated: &GatedExecutor) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while gated.permits.available_permits() == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the simulation should end once the gate opens");
    }
}
