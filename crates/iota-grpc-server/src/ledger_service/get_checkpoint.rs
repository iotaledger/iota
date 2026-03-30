// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Implementation of the `get_checkpoint` and `stream_checkpoints` methods
//! of the LedgerService.
//!
//! # Available Read Mask Fields
//!
//! All checkpoint query methods support the following `read_mask` fields to
//! control which data is included in the response:
//!
//! ## Checkpoint Fields
//! - `checkpoint` - includes all checkpoint fields
//!   - `checkpoint.sequence_number` - the sequence number of the checkpoint
//!   - `checkpoint.summary` - includes all checkpoint summary fields
//!     - `checkpoint.summary.digest` - the digest of the checkpoint summary
//!     - `checkpoint.summary.bcs` - the full BCS-encoded checkpoint summary
//!   - `checkpoint.contents` - includes all checkpoint contents fields
//!     - `checkpoint.contents.digest` - the digest of the checkpoint contents
//!     - `checkpoint.contents.bcs` - the full BCS-encoded checkpoint contents
//!   - `checkpoint.signature` - the validator aggregated signature for the
//!     checkpoint
//!
//! ## Transaction Fields
//! - `transactions` - includes all executed transaction fields
//!   - `transactions.transaction` - includes all transaction fields
//!     - `transactions.transaction.digest` - the transaction digest
//!     - `transactions.transaction.bcs` - the full BCS-encoded transaction
//!   - `transactions.signatures` - includes all signature fields
//!     - `transactions.signatures.bcs` - the full BCS-encoded signature
//!   - `transactions.effects` - includes all effects fields
//!     - `transactions.effects.digest` - the effects digest
//!     - `transactions.effects.bcs` - the full BCS-encoded effects
//!   - `transactions.events` - includes all event fields (all events of the
//!     transaction)
//!     - `transactions.events.digest` - the events digest
//!     - `transactions.events.events` - includes all event fields
//!       - `transactions.events.events.bcs` - the full BCS-encoded event
//!       - `transactions.events.events.package_id` - the ID of the package that
//!         emitted the event
//!       - `transactions.events.events.module` - the module that emitted the
//!         event
//!       - `transactions.events.events.sender` - the sender that triggered the
//!         event
//!       - `transactions.events.events.event_type` - the type of the event
//!       - `transactions.events.events.bcs_contents` - the full BCS-encoded
//!         contents of the event
//!       - `transactions.events.events.json_contents` - the JSON-encoded
//!         contents of the event
//!   - `transactions.checkpoint` - the checkpoint that included the transaction
//!   - `transactions.timestamp` - the timestamp of the checkpoint that included
//!     the transaction
//!   - `transactions.input_objects` - includes all input object fields
//!     - `transactions.input_objects.reference` - includes all reference fields
//!       - `transactions.input_objects.reference.object_id` - the ID of the
//!         input object
//!       - `transactions.input_objects.reference.version` - the version of the
//!         input object
//!       - `transactions.input_objects.reference.digest` - the digest of the
//!         input object contents
//!     - `transactions.input_objects.bcs` - the full BCS-encoded object
//!   - `transactions.output_objects` - includes all output object fields
//!     - `transactions.output_objects.reference` - includes all reference
//!       fields
//!       - `transactions.output_objects.reference.object_id` - the ID of the
//!         output object
//!       - `transactions.output_objects.reference.version` - the version of the
//!         output object
//!       - `transactions.output_objects.reference.digest` - the digest of the
//!         output object contents
//!     - `transactions.output_objects.bcs` - the full BCS-encoded object
//!
//! ## Event Fields
//! - `events` - includes all event fields (all events of all transactions in
//!   the checkpoint)
//!   - `events.bcs` - the full BCS-encoded event
//!   - `events.package_id` - the ID of the package that emitted the event
//!   - `events.module` - the module that emitted the event
//!   - `events.sender` - the sender that triggered the event
//!   - `events.event_type` - the type of the event
//!   - `events.bcs_contents` - the full BCS-encoded contents of the event
//!   - `events.json_contents` - the JSON-encoded contents of the event

use futures::Stream;
use iota_grpc_types::{
    field::{FieldMaskTree, MessageField, MessageFields},
    read_masks::GET_CHECKPOINT_READ_MASK,
    v1::{
        checkpoint::Checkpoint, event::Event, ledger_service as grpc_ledger_service,
        transaction::ExecutedTransaction,
    },
};
use tonic::{Request, Status};
use tracing::debug;

use super::LedgerGrpcService;
use crate::{
    error::RpcError, event_filter::EventFilter, transaction_filter::TransactionFilter,
    types::CheckpointStreamResult, validation::validate_read_mask,
};

/// Helper function to convert proto filters to internal filters and validate
/// their complexity
fn convert_and_validate_filters(
    transactions_filter: Option<iota_grpc_types::v1::filter::TransactionFilter>,
    events_filter: Option<iota_grpc_types::v1::filter::EventFilter>,
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
        is_map: false,
        message_fields: Some(Checkpoint::FIELDS),
    };

    pub const TRANSACTIONS_FIELD: &'static MessageField = &MessageField {
        name: "transactions",
        json_name: "transactions",
        number: 2i32,
        is_optional: true,
        is_map: false,
        message_fields: Some(ExecutedTransaction::FIELDS),
    };

    pub const EVENTS_FIELD: &'static MessageField = &MessageField {
        name: "events",
        json_name: "events",
        number: 3i32,
        is_optional: true,
        is_map: false,
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
) -> Result<(FieldMaskTree, Option<FieldMaskTree>, Option<FieldMaskTree>), RpcError> {
    let read_mask =
        validate_read_mask::<CheckpointDataResponse>(read_mask, GET_CHECKPOINT_READ_MASK)?;

    // Extract checkpoint-related fields mask
    let checkpoint_mask = read_mask.subtree("checkpoint").unwrap_or_default();

    // Extract transactions mask if requested
    let transactions_mask = read_mask.subtree("transactions");

    // Extract events mask if requested
    let events_mask = read_mask.subtree("events");

    Ok((checkpoint_mask, transactions_mask, events_mask))
}

/// Get checkpoint data based on the provided checkpoint ID (sequence number,
/// digest, or latest) and read mask.
///
/// # Request parameters
/// * `read_mask` - Optional field mask specifying which fields to include. If
///   `None`, uses [`GET_CHECKPOINT_READ_MASK`] as default. See [module-level
///   documentation](crate::ledger_service::get_checkpoint) for all available
///   fields.
/// * `transactions_filter` - Optional filter to apply to transactions included
///   in the checkpoint. Only transactions matching the filter will be included
///   in the response.
/// * `events_filter` - Optional filter to apply to events included in the
///   checkpoint. Only events matching the filter will be included in the
///   response.
/// * `max_message_size_bytes` - Optional maximum message size in bytes that the
///   client can handle. The server will use this to limit the size of the
///   response and avoid sending messages that are too large.
/// * `checkpoint_id` - The identifier for the checkpoint to fetch. This can be
///   one of:
///   - `sequence_number` - the sequence number of the checkpoint to fetch
///   - `digest` - the digest of the checkpoint to fetch
///   - `latest` - if set, fetches the latest checkpoint
pub(crate) fn get_checkpoint(
    service: &LedgerGrpcService,
    request: Request<grpc_ledger_service::GetCheckpointRequest>,
) -> Result<impl Stream<Item = CheckpointStreamResult> + Send, RpcError> {
    let req = request.into_inner();

    // determine if we need to get the checkpoint based on the sequential number,
    // digest or the latest one.
    let sequence_number = match req.checkpoint_id {
        Some(grpc_ledger_service::get_checkpoint_request::CheckpointId::SequenceNumber(seq)) => seq,
        Some(grpc_ledger_service::get_checkpoint_request::CheckpointId::Digest(digest)) => {
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
        Some(grpc_ledger_service::get_checkpoint_request::CheckpointId::Latest(_)) => service
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

    // Check if the requested checkpoint has been pruned
    let lowest_available = service
        .reader
        .get_lowest_available_checkpoint()
        .map_err(|e| Status::internal(format!("failed to get lowest available checkpoint: {e}")))?;
    if sequence_number < lowest_available {
        return Err(Status::not_found(format!(
            "Requested checkpoint {sequence_number} is below the lowest available checkpoint {lowest_available}"
        ))
        .into());
    }

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

    if transaction_filter.is_some() && transactions_mask.is_none() {
        return Err(Status::invalid_argument(
            "transactions_filter requires transactions in read_mask",
        )
        .into());
    }
    if event_filter.is_some() && events_mask.is_none() {
        return Err(Status::invalid_argument("events_filter requires events in read_mask").into());
    }

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

/// Stream checkpoint data based on the provided start and end sequence numbers
/// and read mask. This will continuously stream new checkpoints as they are
/// produced until the end sequence number is reached (if provided) or the
/// client disconnects.
///
/// # Request parameters
/// * `start_sequence_number` - Optional sequence number to start streaming
///   from. If not provided, starts from the next checkpoint produced after the
///   request is received.
/// * `end_sequence_number` - Optional sequence number to end streaming at. If
///   not provided, continues streaming indefinitely until the client
///   disconnects.
/// * `read_mask` - Optional field mask specifying which fields to include. If
///   `None`, uses [`GET_CHECKPOINT_READ_MASK`] as default. See [module-level
///   documentation](crate::ledger_service::get_checkpoint) for all available
///   fields.
/// * `transactions_filter` - Optional filter to apply to transactions included
///   in the streamed checkpoints. Only transactions matching the filter will be
///   included in the response.
/// * `events_filter` - Optional filter to apply to events included in the
///   streamed checkpoints. Only events matching the filter will be included in
///   the response.
/// * `max_message_size_bytes` - Optional maximum message size in bytes that the
///   client can handle. The server will use this to limit the size of the
///   response and avoid sending messages that are too large.
pub(crate) fn stream_checkpoints(
    service: &LedgerGrpcService,
    request: Request<grpc_ledger_service::StreamCheckpointsRequest>,
) -> Result<impl Stream<Item = CheckpointStreamResult> + Send, RpcError> {
    let req = request.into_inner();
    let start_sequence_number = req.start_sequence_number;
    let end_sequence_number = req.end_sequence_number;
    let client_max_message_size_bytes = req.max_message_size_bytes;
    let filter_checkpoints = req.filter_checkpoints.unwrap_or(false);
    let progress_interval =
        std::time::Duration::from_millis(req.progress_interval_ms.unwrap_or(2000).max(500) as u64);

    debug!(
        "stream_checkpoints called with start={:?}, end={:?}, max_size={:?}, filter_checkpoints={}, progress_interval={:?}",
        start_sequence_number,
        end_sequence_number,
        client_max_message_size_bytes,
        filter_checkpoints,
        progress_interval
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

    // Check if the requested checkpoint has been pruned
    if let Some(start) = start_sequence_number {
        let lowest_available = service
            .reader
            .get_lowest_available_checkpoint()
            .map_err(|e| {
                Status::internal(format!("failed to get lowest available checkpoint: {e}"))
            })?;
        if start < lowest_available {
            return Err(Status::not_found(format!(
                "Requested checkpoint {} is below the lowest available checkpoint {}",
                start, lowest_available
            ))
            .into());
        }
    }

    // Convert proto filters to internal filters and validate complexity
    let (transaction_filter, event_filter) =
        convert_and_validate_filters(req.transactions_filter, req.events_filter)?;

    // Validate filter_checkpoints constraints
    if filter_checkpoints && transaction_filter.is_none() && event_filter.is_none() {
        return Err(Status::invalid_argument(
            "filter_checkpoints requires at least one of transactions_filter or events_filter",
        )
        .into());
    }

    if transaction_filter.is_some() && transactions_mask.is_none() {
        return Err(Status::invalid_argument(
            "transactions_filter requires transactions in read_mask",
        )
        .into());
    }
    if event_filter.is_some() && events_mask.is_none() {
        return Err(Status::invalid_argument("events_filter requires events in read_mask").into());
    }

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
        filter_checkpoints,
        progress_interval,
    ));
    Ok(stream)
}
