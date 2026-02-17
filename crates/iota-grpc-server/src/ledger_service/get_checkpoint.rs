// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::Stream;
use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil, MessageField, MessageFields},
    v0::{
        checkpoint::Checkpoint, event::Event, ledger_service as grpc_ledger_service,
        transaction::ExecutedTransaction,
    },
};
use tonic::{Request, Status};
use tracing::debug;

use super::LedgerGrpcService;
use crate::{
    error::RpcError, event_filter::EventFilter, transaction_filter::TransactionFilter,
    types::CheckpointStreamResult,
};

/// Default read_mask value when none is provided.
pub const CHECKPOINT_READ_MASK_DEFAULT: &str = "checkpoint.summary";

/// Helper function to convert proto filters to internal filters and validate
/// their complexity
fn convert_and_validate_filters(
    transactions_filter: Option<iota_grpc_types::v0::filter::TransactionFilter>,
    events_filter: Option<iota_grpc_types::v0::filter::EventFilter>,
) -> Result<(Option<TransactionFilter>, Option<EventFilter>), Status> {
    // Convert proto filters to internal filters
    let transaction_filter = transactions_filter
        .map(TransactionFilter::try_from)
        .transpose()
        .map_err(|e| Status::invalid_argument(format!("invalid transaction filter: {e}")))?;

    let event_filter = events_filter
        .map(EventFilter::try_from)
        .transpose()
        .map_err(|e| Status::invalid_argument(format!("invalid event filter: {e}")))?;

    // Validate filter complexity
    if let Some(ref filter) = transaction_filter {
        filter
            .validate_complexity()
            .map_err(Status::invalid_argument)?;
    }
    if let Some(ref filter) = event_filter {
        filter
            .validate_complexity()
            .map_err(Status::invalid_argument)?;
    }

    Ok((transaction_filter, event_filter))
}

/// Represents the structure of checkpoint data response for read_mask
/// validation. This is not a proto type but a helper struct to define valid
/// read_mask paths.
pub struct CheckpointDataResponse;

impl CheckpointDataResponse {
    pub const CHECKPOINT_FIELD: &'static MessageField = &MessageField {
        name: "checkpoint",
        json_name: "checkpoint",
        number: 1i32,
        is_optional: true,
        message_fields: Some(Checkpoint::FIELDS),
    };

    pub const TRANSACTIONS_FIELD: &'static MessageField = &MessageField {
        name: "transactions",
        json_name: "transactions",
        number: 2i32,
        is_optional: true,
        message_fields: Some(ExecutedTransaction::FIELDS),
    };

    pub const EVENTS_FIELD: &'static MessageField = &MessageField {
        name: "events",
        json_name: "events",
        number: 3i32,
        is_optional: true,
        message_fields: Some(Event::FIELDS),
    };
}

impl MessageFields for CheckpointDataResponse {
    const FIELDS: &'static [&'static MessageField] = &[
        Self::CHECKPOINT_FIELD,
        Self::TRANSACTIONS_FIELD,
        Self::EVENTS_FIELD,
    ];
}

/// Parse read_mask from request and extract component masks for checkpoint,
/// transactions, and events.
fn parse_checkpoint_read_mask(
    read_mask: Option<prost_types::FieldMask>,
) -> Result<(FieldMaskTree, Option<FieldMaskTree>, Option<FieldMaskTree>), Status> {
    let field_mask =
        read_mask.unwrap_or_else(|| prost_types::FieldMask::from_str(CHECKPOINT_READ_MASK_DEFAULT));

    // Validate the read_mask paths
    FieldMaskUtil::validate::<CheckpointDataResponse>(&field_mask)
        .map_err(|path| Status::invalid_argument(format!("invalid read_mask path: {path}")))?;

    // Convert to FieldMaskTree after validation
    let read_mask = FieldMaskTree::from(field_mask);

    // Extract checkpoint-related fields mask
    let checkpoint_mask = read_mask.subtree("checkpoint").unwrap_or_default();

    // Extract transactions mask if requested
    let transactions_mask = read_mask.subtree("transactions");

    // Extract events mask if requested
    let events_mask = read_mask.subtree("events");

    Ok((checkpoint_mask, transactions_mask, events_mask))
}

pub(crate) fn get_checkpoint_data(
    service: &LedgerGrpcService,
    request: Request<grpc_ledger_service::GetCheckpointDataRequest>,
) -> Result<impl Stream<Item = CheckpointStreamResult> + Send, RpcError> {
    let req = request.into_inner();

    // determine if we need to get the checkpoint based on the sequential number,
    // digest or the latest one.
    let sequence_number = match req.checkpoint_id {
        Some(grpc_ledger_service::get_checkpoint_data_request::CheckpointId::SequenceNumber(
            seq,
        )) => seq,
        Some(grpc_ledger_service::get_checkpoint_data_request::CheckpointId::Digest(digest)) => {
            let sdk_digest: iota_sdk_types::Digest = (&digest)
                .try_into()
                .map_err(|e| Status::invalid_argument(format!("invalid checkpoint digest: {e}")))?;
            let digest: iota_types::digests::CheckpointDigest = sdk_digest.into();
            service
                .reader
                .get_checkpoint_sequence_number_by_digest(&digest)
                .map_err(|e| Status::internal(format!("failed to get checkpoint by digest: {e}")))?
                .ok_or(Status::not_found("checkpoint not found"))?
        }
        Some(grpc_ledger_service::get_checkpoint_data_request::CheckpointId::Latest(_)) => service
            .reader
            .get_latest_checkpoint_sequence_number()
            .map_err(|e| Status::internal(format!("failed to get latest checkpoint: {e}")))?
            .ok_or(Status::not_found("latest checkpoint not found"))?,
        None => {
            return Err(Status::invalid_argument("checkpoint_id must be provided").into());
        }
        Some(_) => {
            return Err(Status::invalid_argument("unknown checkpoint_id type").into());
        }
    };

    let client_max_message_size_bytes = req.max_message_size_bytes;

    debug!(
        "get_checkpoint called for seq={} with max_size={:?}",
        sequence_number, client_max_message_size_bytes
    );

    let max_message_size_bytes = service
        .config
        .max_message_size_client_bytes(client_max_message_size_bytes);

    // Parse the read_mask to determine what data to include
    let (checkpoint_mask, transactions_mask, events_mask) =
        parse_checkpoint_read_mask(req.read_mask)?;

    debug!(
        "Parsed read_mask: checkpoint_mask={}, transactions={}, events={}",
        checkpoint_mask,
        transactions_mask.is_some(),
        events_mask.is_some()
    );

    // Convert proto filters to internal filters and validate complexity
    let (transaction_filter, event_filter) =
        convert_and_validate_filters(req.transactions_filter, req.events_filter)?;

    Ok(service.reader.get_checkpoint_data(
        sequence_number,
        checkpoint_mask,
        transactions_mask,
        events_mask,
        max_message_size_bytes,
        transaction_filter,
        event_filter,
    ))
}

pub(crate) fn stream_checkpoint_data(
    service: &LedgerGrpcService,
    request: Request<grpc_ledger_service::CheckpointDataStreamRequest>,
) -> Result<impl Stream<Item = CheckpointStreamResult> + Send, RpcError> {
    let req = request.into_inner();
    let start_sequence_number = req.start_sequence_number;
    let end_sequence_number = req.end_sequence_number;
    let client_max_message_size_bytes = req.max_message_size_bytes;

    debug!(
        "stream_checkpoints called with start={:?}, end={:?}, max_size={:?}",
        start_sequence_number, end_sequence_number, client_max_message_size_bytes
    );

    let max_message_size_bytes = service
        .config
        .max_message_size_client_bytes(client_max_message_size_bytes);

    // Parse the read_mask to determine what data to include
    let (checkpoint_mask, transactions_mask, events_mask) =
        parse_checkpoint_read_mask(req.read_mask)?;

    debug!(
        "Parsed read_mask: checkpoint_mask={}, transactions={}, events={}",
        checkpoint_mask,
        transactions_mask.is_some(),
        events_mask.is_some()
    );

    // Convert proto filters to internal filters and validate complexity
    let (transaction_filter, event_filter) =
        convert_and_validate_filters(req.transactions_filter, req.events_filter)?;

    let rx = service.checkpoint_data_broadcaster.subscribe();
    let stream = Box::pin(service.reader.create_checkpoint_data_stream(
        rx,
        start_sequence_number,
        end_sequence_number,
        checkpoint_mask,
        transactions_mask,
        events_mask,
        max_message_size_bytes,
        service.cancellation_token.clone(),
        transaction_filter,
        event_filter,
    ));
    Ok(stream)
}
