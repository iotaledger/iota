// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::Stream;
use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil},
    google::rpc::bad_request::FieldViolation,
    merge::Merge,
    v0::{
        error_reason::ErrorReason,
        ledger_service::{
            GetTransactionsRequest, GetTransactionsResponse, TransactionResult, transaction_result,
        },
        object::Objects,
        transaction::{ExecutedTransaction, TransactionEvents, TransactionReadSource},
    },
};
use iota_types::{digests::TransactionDigest, iota_sdk_types_conversions::SdkTypeConversionError};
use prost::Message;
use prost_types::FieldMask;

use crate::{
    constants::validate_max_message_size,
    error::RpcError,
    types::{GrpcReader, TransactionsStreamResult},
};

pub const READ_MASK_DEFAULT: &str = crate::field_mask!("digest");

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
    reader: GrpcReader,
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
            let tx_result = match get_transaction_impl(&reader, digest, &read_mask) {
                Ok(tx) => TransactionResult {
                    result: Some(transaction_result::Result::Transaction(Box::new(tx))),
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
    reader: &GrpcReader,
    digest: TransactionDigest,
    read_mask: &FieldMaskTree,
) -> Result<ExecutedTransaction, RpcError> {
    // Get transaction data from storage
    let tx_read = reader.get_transaction_read(&digest)?;

    // Convert to iota_sdk2 types - create owned data in local scope
    // Clone the inner transaction from Arc and convert to SDK type
    let sdk_transaction: iota_sdk2::types::SignedTransaction = (*tx_read.transaction)
        .clone()
        .into_inner()
        .try_into()
        .map_err(|e: SdkTypeConversionError| anyhow::Error::msg(e.to_string()))?;

    let sdk_effects: iota_sdk2::types::TransactionEffects = tx_read
        .effects
        .clone()
        .try_into()
        .map_err(|e: SdkTypeConversionError| anyhow::Error::msg(e.to_string()))?;

    let sdk_events: Option<iota_sdk2::types::TransactionEvents> = tx_read
        .events
        .as_ref()
        .map(|events| {
            events
                .clone()
                .try_into()
                .map_err(|e: SdkTypeConversionError| anyhow::Error::msg(e.to_string()))
        })
        .transpose()?;

    let sdk_digest: iota_sdk2::types::TransactionDigest = tx_read.digest.into();

    // Create TransactionReadSource with references to local owned data
    let sdk_source = TransactionReadSource {
        digest: sdk_digest,
        transaction: &sdk_transaction,
        effects: &sdk_effects,
        events: sdk_events.as_ref(),
        checkpoint: tx_read.checkpoint,
        timestamp_ms: tx_read.timestamp_ms,
    };

    // Build response using Merge trait
    let mut message = ExecutedTransaction::default();
    Merge::merge(&mut message, &sdk_source, read_mask);

    // Handle events separately (as noted in TransactionReadSource impl)
    // Events are handled here because they need the events_digest from effects
    if let Some(events_mask) = read_mask.subtree(ExecutedTransaction::EVENTS_FIELD.name) {
        if let Some(ref sdk_events) = sdk_events {
            let mut proto_events = TransactionEvents::default();
            Merge::merge(&mut proto_events, sdk_events, &events_mask);
            message.events = Some(proto_events);
        }
    }

    // Handle input_objects separately
    if let Some(input_objects_mask) =
        read_mask.subtree(ExecutedTransaction::INPUT_OBJECTS_FIELD.name)
    {
        // Convert input objects to SDK types
        let sdk_input_objects: Vec<iota_sdk2::types::Object> = tx_read
            .input_objects
            .into_iter()
            .filter_map(|obj| obj.try_into().ok())
            .collect();

        if !sdk_input_objects.is_empty() {
            let mut proto_objects = Objects::default();
            Merge::merge(
                &mut proto_objects,
                sdk_input_objects.as_slice(),
                &input_objects_mask,
            );
            message.input_objects = Some(proto_objects);
        }
    }

    // Handle output_objects separately
    if let Some(output_objects_mask) =
        read_mask.subtree(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name)
    {
        // Convert output objects to SDK types
        let sdk_output_objects: Vec<iota_sdk2::types::Object> = tx_read
            .output_objects
            .into_iter()
            .filter_map(|obj| obj.try_into().ok())
            .collect();

        if !sdk_output_objects.is_empty() {
            let mut proto_objects = Objects::default();
            Merge::merge(
                &mut proto_objects,
                sdk_output_objects.as_slice(),
                &output_objects_mask,
            );
            message.output_objects = Some(proto_objects);
        }
    }

    Ok(message)
}
