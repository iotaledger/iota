// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::Stream;
use iota_grpc_types::{field::FieldMaskTree, v0::ledger_service as grpc_ledger_service};
use tonic::{Request, Status};
use tracing::debug;

use super::LedgerGrpcService;
use crate::{
    error::RpcError, event_filter::EventFilter, transaction_filter::TransactionFilter,
    types::CheckpointStreamResult,
};

/// Default read_mask value when none is provided.
/// As per proto comment: "If no mask is specified, defaults to `summary`."
pub const CHECKPOINT_READ_MASK_DEFAULT: &str = "summary";

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

/// Parse read_mask from request and extract component masks for checkpoint,
/// transactions, and events.
fn parse_read_mask(
    read_mask: Option<prost_types::FieldMask>,
) -> (FieldMaskTree, Option<FieldMaskTree>, Option<FieldMaskTree>) {
    let read_mask = read_mask.map(FieldMaskTree::from).unwrap_or_else(|| {
        CHECKPOINT_READ_MASK_DEFAULT
            .parse()
            .expect("valid default mask")
    });

    // Extract checkpoint-related fields mask
    // The Checkpoint message has: sequence_number, summary, contents, signature
    let checkpoint_mask = {
        let mut mask = FieldMaskTree::default();
        for field in &["sequence_number", "summary", "contents", "signature"] {
            if read_mask.contains(field) {
                mask.add_field_path(field);
            }
        }
        // If any checkpoint field is requested (or if it's a wildcard), use the full
        // mask
        if mask.to_field_mask().paths.is_empty()
            && !read_mask.contains("transactions")
            && !read_mask.contains("events")
        {
            // Default to summary if nothing specific requested
            mask.add_field_path("summary");
        }
        mask
    };

    // Extract transactions mask if requested
    let transactions_mask = read_mask.subtree("transactions");

    // Extract events mask if requested
    let events_mask = if read_mask.contains("events") {
        Some(
            read_mask
                .subtree("events")
                .unwrap_or_else(FieldMaskTree::new_wildcard),
        )
    } else {
        None
    };

    (checkpoint_mask, transactions_mask, events_mask)
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
        Some(grpc_ledger_service::get_checkpoint_data_request::CheckpointId::Digest(_digest)) => {
            // TODO: do we have a lookup table for that?
            return Err(Status::unimplemented(
                "checkpoint lookup by digest is not yet implemented",
            )
            .into());
        }
        Some(grpc_ledger_service::get_checkpoint_data_request::CheckpointId::Latest(_)) => service
            .reader
            .get_latest_checkpoint_sequence_number()
            .ok_or(Status::not_found("latest checkpoint not found"))?,
        None => {
            return Err(Status::invalid_argument("checkpoint_id must be provided").into());
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
    let (checkpoint_mask, transactions_mask, events_mask) = parse_read_mask(req.read_mask);

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
    let (checkpoint_mask, transactions_mask, events_mask) = parse_read_mask(req.read_mask);

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
