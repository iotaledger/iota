// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod simulate;
mod transaction;

use std::{sync::Arc, time::Duration};

use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil, MessageFields},
    google::rpc::bad_request::FieldViolation,
    proto::timestamp_ms_to_proto,
    read_masks::EXECUTE_TRANSACTIONS_READ_MASK,
    v1::{
        error_reason::ErrorReason,
        transaction::{ExecutedTransaction, Transaction as ProtoTransaction},
        transaction_execution_service::{
            self as grpc_tx_service, ExecuteTransactionItem, ExecuteTransactionResult,
            ExecuteTransactionsRequest, ExecuteTransactionsResponse, SimulateTransactionsRequest,
            SimulateTransactionsResponse, execute_transaction_result,
        },
    },
};
use iota_types::{
    digests::TransactionDigest,
    effects::TransactionEffectsAPI,
    quorum_driver_types::{ExecuteTransactionRequestV1, ExecuteTransactionResponseV1},
    transaction_executor::TransactionExecutor,
};
use prost_types::FieldMask;
use tonic::{Request, Response};
pub use transaction::{CommandResultsReadSource, TransactionReadSource};

use crate::{error::RpcError, merge::Merge, types::GrpcReader};

pub struct TransactionExecutionGrpcService {
    pub config: iota_config::node::GrpcApiConfig,
    pub reader: Arc<GrpcReader>,
    pub executor: Arc<dyn TransactionExecutor>,
}

impl TransactionExecutionGrpcService {
    pub fn new(
        config: iota_config::node::GrpcApiConfig,
        reader: Arc<GrpcReader>,
        executor: Arc<dyn TransactionExecutor>,
    ) -> Self {
        Self {
            config,
            reader,
            executor,
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
        let response = execute_transactions(
            &self.reader,
            &self.executor,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn simulate_transactions(
        &self,
        request: Request<SimulateTransactionsRequest>,
    ) -> Result<Response<SimulateTransactionsResponse>, tonic::Status> {
        let response = simulate::simulate_transactions(
            &self.reader,
            &self.executor,
            &self.config,
            request.into_inner(),
        )
        .await
        .map(Response::new)
        .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }
}

// === Shared helpers for execute and simulate ===

/// Validate that a batch is non-empty and within the size limit.
fn validate_batch_size(items_len: usize, max_batch: usize) -> Result<(), RpcError> {
    if items_len == 0 {
        return Err(RpcError::new(
            tonic::Code::InvalidArgument,
            "transactions list must not be empty",
        ));
    }
    if items_len > max_batch {
        return Err(RpcError::new(
            tonic::Code::InvalidArgument,
            format!(
                "batch size {} exceeds maximum allowed ({})",
                items_len, max_batch
            ),
        ));
    }
    Ok(())
}

/// Parse, validate, and convert a field mask with a default fallback.
fn parse_read_mask<T: MessageFields>(
    mask: Option<FieldMask>,
    default: &str,
) -> Result<FieldMaskTree, RpcError> {
    let read_mask = mask.unwrap_or_else(|| FieldMask::from_str(default));
    read_mask.validate::<T>().map_err(|path| {
        FieldViolation::new("read_mask")
            .with_description(format!("invalid read_mask path: {path}"))
            .with_reason(ErrorReason::FieldInvalid)
    })?;
    Ok(FieldMaskTree::from(read_mask))
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
) -> Result<iota_sdk_types::transaction::Transaction, RpcError> {
    let transaction_proto = transaction
        .ok_or_else(|| FieldViolation::new("transaction").with_reason(ErrorReason::FieldMissing))?;

    let transaction_bcs = transaction_proto.bcs.as_ref().ok_or_else(|| {
        FieldViolation::new("transaction.bcs")
            .with_description("transaction BCS is required")
            .with_reason(ErrorReason::FieldMissing)
    })?;

    let sdk_transaction: iota_sdk_types::transaction::Transaction =
        bcs::from_bytes(&transaction_bcs.data).map_err(|e| {
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

        if computed_digest.inner() != &provided_digest_bytes {
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

/// Execute a batch of transactions sequentially.
///
/// Each transaction is executed independently — failure of one does not abort
/// the rest. Results are returned in the same order as the input.
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
/// [`ExecutedTransaction`](iota_grpc_types::v1::transaction::ExecutedTransaction)
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
///   - `events.events.package_id` - the ID of the package that emitted the
///     event
///   - `events.events.module` - the module that emitted the event
///   - `events.events.sender` - the sender that triggered the event
///   - `events.events.event_type` - the type of the event
///   - `events.events.bcs_contents` - the full BCS-encoded contents of the
///     event
///   - `events.events.json_contents` - the JSON-encoded contents of the event
///
/// ## Checkpoint Fields
/// - `checkpoint` - the checkpoint that included the transaction. Requires
///   `checkpoint_inclusion_timeout_ms` to be set.
/// - `timestamp` - the timestamp of the checkpoint. Requires
///   `checkpoint_inclusion_timeout_ms` to be set.
///
/// ## Object Fields
/// - `input_objects` - includes all input object fields
///   - `input_objects.reference` - includes all reference fields
///     - `input_objects.reference.object_id` - the ID of the input object
///     - `input_objects.reference.version` - the version of the input object
///     - `input_objects.reference.digest` - the digest of the input object
///       contents
///   - `input_objects.bcs` - the full BCS-encoded object
/// - `output_objects` - includes all output object fields
///   - `output_objects.reference` - includes all reference fields
///     - `output_objects.reference.object_id` - the ID of the output object
///     - `output_objects.reference.version` - the version of the output object
///     - `output_objects.reference.digest` - the digest of the output object
///       contents
///   - `output_objects.bcs` - the full BCS-encoded object
#[tracing::instrument(skip(reader, executor))]
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
    let read_mask =
        parse_read_mask::<ExecutedTransaction>(request.read_mask, EXECUTE_TRANSACTIONS_READ_MASK)?;

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

    // Execute each transaction sequentially, collecting per-item results and
    // digests for successful executions.
    let mut transaction_results = Vec::with_capacity(request.transactions.len());
    let mut successful_digests: Vec<(usize, TransactionDigest)> = Vec::new();
    for (i, item) in request.transactions.iter().enumerate() {
        let result = match execute_single_transaction(reader, executor, config, item, &read_mask)
            .await
        {
            Ok((digest, tx)) => {
                successful_digests.push((i, digest));
                ExecuteTransactionResult::default().with_executed_transaction(tx)
            }
            Err(error) => ExecuteTransactionResult::default().with_error(error.into_status_proto()),
        };
        transaction_results.push(result);
    }

    // Optionally wait for checkpoint inclusion and populate checkpoint/timestamp
    // on the already-built results.
    if let Some(timeout) = checkpoint_timeout {
        if !successful_digests.is_empty() {
            let digests: Vec<_> = successful_digests.iter().map(|(_, d)| *d).collect();
            match executor
                .wait_for_checkpoint_inclusion(&digests, timeout)
                .await
            {
                Ok(checkpoint_map) => {
                    let needs_checkpoint =
                        read_mask.contains(ExecutedTransaction::CHECKPOINT_FIELD.name);
                    let needs_timestamp =
                        read_mask.contains(ExecutedTransaction::TIMESTAMP_FIELD.name);

                    if !needs_checkpoint && !needs_timestamp {
                        // No need to update results if fields aren't requested in read mask
                        return Ok(ExecuteTransactionsResponse::default()
                            .with_transaction_results(transaction_results));
                    }

                    for (i, digest) in &successful_digests {
                        if let Some((seq, ts)) = checkpoint_map.get(digest) {
                            if let Some(execute_transaction_result::Result::ExecutedTransaction(
                                ref mut tx,
                            )) = transaction_results[*i].result
                            {
                                if needs_checkpoint {
                                    tx.checkpoint = Some(*seq);
                                }
                                if needs_timestamp && *ts > 0 {
                                    tx.timestamp = Some(timestamp_ms_to_proto(*ts));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("wait_for_checkpoint_inclusion failed: {e}");
                }
            }
        }
    }

    Ok(ExecuteTransactionsResponse::default().with_transaction_results(transaction_results))
}

/// Validate, execute, and merge a single transaction item.
async fn execute_single_transaction(
    reader: &Arc<GrpcReader>,
    executor: &Arc<dyn TransactionExecutor>,
    config: &iota_config::node::GrpcApiConfig,
    item: &ExecuteTransactionItem,
    read_mask: &FieldMaskTree,
) -> Result<(TransactionDigest, ExecutedTransaction), RpcError> {
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

    let transaction = iota_types::transaction::Transaction::try_from(sdk_signed_transaction)
        .map_err(|e| {
            RpcError::new(
                tonic::Code::InvalidArgument,
                format!("failed to convert transaction to internal type: {e}"),
            )
        })?;

    // Determine what to include in the request based on read mask.
    let include_events = read_mask.contains(ExecutedTransaction::EVENTS_FIELD.name);
    let include_input_objects = read_mask.contains(ExecutedTransaction::INPUT_OBJECTS_FIELD.name);
    let include_output_objects = read_mask.contains(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name);

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
        .execute_transaction(exec_request, None)
        .await
        .map_err(RpcError::from)?;

    let digest = *effects.effects.transaction_digest();

    // Build the merged response
    let sdk_transaction: iota_sdk_types::Transaction =
        transaction.transaction_data().clone().try_into()?;
    let signatures: Vec<iota_sdk_types::UserSignature> = transaction
        .tx_signatures()
        .to_owned()
        .into_iter()
        .map(|sig| sig.try_into())
        .collect::<Result<_, _>>()?;

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
    };

    let executed = ExecutedTransaction::merge_from(&source, read_mask)
        .map_err(|e| e.with_context("failed to merge executed transaction"))?;

    Ok((digest, executed))
}
