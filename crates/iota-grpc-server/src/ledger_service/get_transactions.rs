// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::Stream;
use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    read_masks::GET_TRANSACTIONS_READ_MASK,
    v1::{
        error_reason::ErrorReason,
        ledger_service::{GetTransactionsRequest, GetTransactionsResponse, TransactionResult},
        transaction::ExecutedTransaction,
    },
};
use iota_types::digests::TransactionDigest;
use prost::Message;
use prost_types::FieldMask;

use crate::{
    constants::validate_max_message_size,
    error::RpcError,
    merge::Merge,
    transaction_execution_service::TransactionReadSource,
    types::{GrpcReader, TransactionReadFields, TransactionsStreamResult},
    validation::validate_read_mask,
};

type ValidationResult = Result<(Vec<TransactionDigest>, FieldMaskTree), RpcError>;

pub(crate) fn validate_get_transaction_requests(
    requests: Vec<Option<Vec<u8>>>,
    read_mask: Option<FieldMask>,
) -> ValidationResult {
    let read_mask =
        validate_read_mask::<ExecutedTransaction>(read_mask, GET_TRANSACTIONS_READ_MASK)?;

    let requests = requests
        .into_iter()
        .enumerate()
        .map(|(idx, digest_bytes)| {
            let digest_bytes = digest_bytes.ok_or_else(|| {
                FieldViolation::new("digest")
                    .with_reason(ErrorReason::FieldMissing)
                    .nested_at("requests", idx)
            })?;

            TransactionDigest::try_from(digest_bytes.as_slice()).map_err(|e| {
                FieldViolation::new("digest")
                    .with_description(format!("invalid digest: {e}"))
                    .with_reason(ErrorReason::FieldInvalid)
                    .nested_at("requests", idx)
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((requests, read_mask))
}

/// Available Read Mask Fields
///
/// The `get_transactions` function supports the following `read_mask` fields to
/// control which data is included in the response:
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
/// - `events` - includes all event fields
///   - `events.digest` - the events digest
///   - `events.events` - includes all event fields
///     - `events.events.bcs` - the full BCS-encoded event
///     - `events.events.package_id` - the ID of the package that emitted the
///       event
///     - `events.events.module` - the module that emitted the event
///     - `events.events.sender` - the sender that triggered the event
///     - `events.events.event_type` - the type of the event
///     - `events.events.bcs_contents` - the full BCS-encoded contents of the
///       event
///     - `events.events.json_contents` - the JSON-encoded contents of the event
///
/// ## Timing Fields
/// - `checkpoint` - the checkpoint that included the transaction
/// - `timestamp` - the timestamp of the checkpoint that included the
///   transaction
///
/// ## Object Fields
/// - `input_objects` - includes all input object fields
///   - `input_objects.reference` - includes all reference fields
///     - `input_objects.reference.object_id` - the ID of the input object
///     - `input_objects.reference.version` - the version of the input object,
///       which can be used to fetch a specific historical version or the latest
///       version if not provided
///     - `input_objects.reference.digest` - the digest of the input object
///       contents, which can be used for integrity verification
///   - `input_objects.bcs` - the full BCS-encoded object
/// - `output_objects` - includes all output object fields
///   - `output_objects.reference` - includes all reference fields
///     - `output_objects.reference.object_id` - the ID of the output object
///     - `output_objects.reference.version` - the version of the output object,
///       which can be used to fetch a specific historical version or the latest
///       version if not provided
///     - `output_objects.reference.digest` - the digest of the output object
///       contents, which can be used for integrity verification
///   - `output_objects.bcs` - the full BCS-encoded object
#[tracing::instrument(skip(reader))]
pub(crate) fn get_transactions(
    reader: Arc<GrpcReader>,
    config: iota_config::node::GrpcApiConfig,
    GetTransactionsRequest {
        requests,
        read_mask,
        max_message_size_bytes,
        ..
    }: GetTransactionsRequest,
) -> Result<impl Stream<Item = TransactionsStreamResult> + Send, RpcError> {
    let requests = requests
        .map(|r| r.requests)
        .unwrap_or_default()
        .into_iter()
        .map(|req| req.digest.map(|d| d.digest.to_vec()))
        .collect();

    let (digests, read_mask) = validate_get_transaction_requests(requests, read_mask)?;
    let max_message_size = validate_max_message_size(max_message_size_bytes)?;

    Ok(crate::create_batching_stream!(
        digests.into_iter(),
        digest,
        {
            let tx_result = match get_transaction_impl(&reader, &config, digest, &read_mask) {
                Ok(tx) => TransactionResult::default().with_executed_transaction(tx),
                Err(error) => TransactionResult::default().with_error(error.into_status_proto()),
            };

            let tx_size = tx_result.encoded_len();
            (tx_result, tx_size)
        },
        max_message_size,
        GetTransactionsResponse,
        transaction_results,
        has_next
    ))
}

#[tracing::instrument(skip(reader))]
fn get_transaction_impl(
    reader: &Arc<GrpcReader>,
    config: &iota_config::node::GrpcApiConfig,
    digest: TransactionDigest,
    read_mask: &FieldMaskTree,
) -> Result<ExecutedTransaction, RpcError> {
    // Derive which optional fields to fetch based on the read_mask
    let fields = TransactionReadFields::from_mask(read_mask);

    // Get transaction data from storage, skipping unrequested fields
    let tx_read = reader.get_transaction_read(&digest, &fields)?;

    // Create a source for the merge
    let source = TransactionReadSource {
        reader: reader.clone(),
        config,
        transaction: tx_read.transaction,
        signatures: tx_read.signatures,
        effects: tx_read.effects,
        events: tx_read.events,
        checkpoint: tx_read.checkpoint,
        timestamp_ms: tx_read.timestamp_ms,
        input_objects: tx_read.input_objects,
        output_objects: tx_read.output_objects,
    };

    ExecutedTransaction::merge_from(&source, read_mask)
        .map_err(|e| e.with_context("failed to merge transaction"))
}
