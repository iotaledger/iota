// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::Stream;
use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil},
    google::rpc::bad_request::FieldViolation,
    v0::{
        error_reason::ErrorReason,
        ledger_service::{
            GetTransactionsRequest, GetTransactionsResponse, TransactionResult, transaction_result,
        },
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
};

pub const READ_MASK_DEFAULT: &str = crate::field_mask!("transaction.digest");

type ValidationResult = Result<(Vec<TransactionDigest>, FieldMaskTree), RpcError>;

pub fn validate_get_transaction_requests(
    requests: Vec<Option<Vec<u8>>>,
    read_mask: Option<FieldMask>,
) -> ValidationResult {
    let read_mask = {
        let read_mask = read_mask.unwrap_or_else(|| FieldMask::from_str(READ_MASK_DEFAULT));
        read_mask
            .validate::<ExecutedTransaction>()
            .map_err(|path| {
                FieldViolation::new("read_mask")
                    .with_description(format!("invalid read_mask path: {path}"))
                    .with_reason(ErrorReason::FieldInvalid)
            })?;
        FieldMaskTree::from(read_mask)
    };

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

#[tracing::instrument(skip(reader))]
pub(crate) fn get_transactions(
    reader: Arc<GrpcReader>,
    config: iota_config::node::GrpcApiConfig,
    GetTransactionsRequest {
        requests,
        read_mask,
        max_message_size_bytes,
    }: GetTransactionsRequest,
) -> Result<impl Stream<Item = TransactionsStreamResult> + Send, RpcError> {
    let requests = requests
        .map(|r| r.requests)
        .unwrap_or_default()
        .into_iter()
        .map(|req| req.digest.map(|d| d.digest.to_vec()))
        .collect();

    let (digests, read_mask) = validate_get_transaction_requests(requests, read_mask)?;
    let max_message_size = validate_max_message_size(max_message_size_bytes.map(|v| v as u64))?;

    Ok(crate::create_batching_stream!(
        digests.into_iter(),
        digest,
        {
            let tx_result = match get_transaction_impl(&reader, &config, digest, &read_mask) {
                Ok(tx) => TransactionResult {
                    result: Some(transaction_result::Result::Transaction(tx)),
                },
                Err(error) => TransactionResult {
                    result: Some(transaction_result::Result::Error(error.into_status_proto())),
                },
            };

            let tx_size = tx_result.encoded_len();
            (tx_result, tx_size)
        },
        max_message_size,
        GetTransactionsResponse,
        transactions,
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

    ExecutedTransaction::merge_from(&source, read_mask).map_err(|e| {
        RpcError::new(
            tonic::Code::Internal,
            format!("failed to build executed transaction in get_transaction response: {e}"),
        )
    })
}
