// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::Stream;
use iota_grpc_types::{
    google::rpc::bad_request::FieldViolation,
    read_masks::LIST_OWNED_OBJECTS_READ_MASK,
    v0::{
        error_reason::ErrorReason,
        object::Object,
        state_service::{ListOwnedObjectsRequest, ListOwnedObjectsResponse},
    },
};
use prost::Message;

use crate::{
    constants::validate_max_message_size,
    error::RpcError,
    merge::Merge,
    types::{GrpcReader, ListOwnedObjectsStreamResult},
    validation::{collect_iter, require_address, validate_limit, validate_read_mask},
};

/// Default limit for owned object listing
const DEFAULT_LIMIT: usize = 50;
/// Maximum limit for owned object listing
const MAX_LIMIT: usize = 1000;

#[tracing::instrument(skip(reader))]
pub(crate) fn list_owned_objects(
    reader: Arc<GrpcReader>,
    ListOwnedObjectsRequest {
        owner,
        limit,
        read_mask,
        object_type,
        max_message_size_bytes,
        ..
    }: ListOwnedObjectsRequest,
) -> Result<impl Stream<Item = ListOwnedObjectsStreamResult> + Send, RpcError> {
    let owner_address = require_address(&owner, "owner")?;

    let read_mask = validate_read_mask::<Object>(read_mask, LIST_OWNED_OBJECTS_READ_MASK)?;

    // Parse optional object type filter
    let type_filter = object_type
        .as_deref()
        .map(|t| {
            iota_types::parse_iota_struct_tag(t).map_err(|e| {
                FieldViolation::new("object_type")
                    .with_description(format!("invalid object_type: {e}"))
                    .with_reason(ErrorReason::FieldInvalid)
            })
        })
        .transpose()?;

    let limit = validate_limit(limit, DEFAULT_LIMIT, MAX_LIMIT);
    let max_message_size = validate_max_message_size(max_message_size_bytes)?;

    // Streaming handles pagination; limit cap is applied for safety.
    //
    // Note: each item requires an individual `get_object` call below (N+1 pattern).
    // Acceptable for now given the limit cap, but consider batch fetching if perf
    // becomes a concern.
    let items = collect_iter(
        reader
            .account_owned_objects_info_iter(owner_address, None, type_filter)?
            .take(limit),
    )?;

    // Fetch and merge objects. Skip any object that is no longer found
    // (e.g. transferred or deleted between the index scan and the fetch).
    let objects: Vec<(Object, usize)> = items
        .into_iter()
        .filter_map(|info| {
            let object = match reader.get_object_by_key(&info.object_id, info.version) {
                Ok(Some(obj)) => obj,
                Ok(None) => {
                    tracing::debug!(
                        "object {}:{} not found while iterating owned objects, skipping",
                        info.object_id,
                        info.version,
                    );
                    return None;
                }
                Err(e) => return Some(Err(RpcError::from(e))),
            };
            let merged = Object::merge_from(object, &read_mask)
                .map_err(|e| e.with_context("failed to merge object"));
            Some(merged.map(|m| {
                let size = m.encoded_len();
                (m, size)
            }))
        })
        .collect::<Result<Vec<_>, RpcError>>()?;

    Ok(crate::create_batching_stream!(
        objects.into_iter(),
        (object, size),
        { (object, size) },
        max_message_size,
        ListOwnedObjectsResponse,
        objects,
        has_next
    ))
}
