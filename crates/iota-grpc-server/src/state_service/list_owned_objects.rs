// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    google::rpc::bad_request::FieldViolation,
    read_masks::LIST_OWNED_OBJECTS_READ_MASK,
    v1::{
        error_reason::ErrorReason,
        object::Object,
        state_service::{ListOwnedObjectsRequest, ListOwnedObjectsResponse},
    },
};
use iota_types::base_types::IotaAddress;
use prost::Message;
use serde::{Deserialize, Serialize};

use crate::{
    constants::validate_max_message_size,
    error::RpcError,
    merge::Merge,
    types::{GrpcReader, OwnedObjectV2Cursor},
    validation::{
        decode_page_token, encode_page_token, page_token_mismatch, require_address,
        validate_page_size, validate_read_mask,
    },
};

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 1000;

#[derive(Serialize, Deserialize)]
struct PageToken {
    owner: IotaAddress,
    object_type: Option<move_core_types::language_storage::StructTag>,
    cursor: OwnedObjectV2Cursor,
}

#[tracing::instrument(skip(reader))]
pub(crate) fn list_owned_objects(
    reader: Arc<GrpcReader>,
    ListOwnedObjectsRequest {
        owner,
        page_size,
        page_token,
        read_mask,
        object_type,
        max_message_size_bytes,
        ..
    }: ListOwnedObjectsRequest,
) -> Result<ListOwnedObjectsResponse, RpcError> {
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

    let page_size = validate_page_size(page_size, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE);
    let max_message_size = validate_max_message_size(max_message_size_bytes)?;

    let page_token: Option<PageToken> = decode_page_token(&page_token)?;
    if let Some(ref t) = page_token {
        if t.owner != owner_address || t.object_type != type_filter {
            return Err(page_token_mismatch());
        }
    }

    let cursor = page_token.as_ref().map(|t| &t.cursor);

    let mut iter =
        reader.account_owned_objects_info_iter_v2(owner_address, cursor, type_filter.clone())?;

    let mut objects = Vec::with_capacity(page_size);
    let mut size_bytes = 0usize;
    let mut last_cursor: Option<OwnedObjectV2Cursor> = None;

    for result in iter.by_ref() {
        let (info, item_cursor) = result.map_err(RpcError::from)?;

        let object = match reader.get_object_by_key(&info.object_id, info.version) {
            Ok(Some(obj)) => obj,
            Ok(None) => {
                tracing::debug!(
                    "object {}:{} not found while iterating owned objects, skipping",
                    info.object_id,
                    info.version,
                );

                // Skip any object that is no longer found (e.g. transferred or deleted between
                // the index scan and the fetch).
                continue;
            }
            Err(e) => Err(RpcError::from(e))?,
        };

        let merged = Object::merge_from(object, &read_mask)
            .map_err(|e| e.with_context("failed to merge object"))?;

        let item_size = merged.encoded_len();

        if !objects.is_empty() && size_bytes + item_size > max_message_size {
            let response = ListOwnedObjectsResponse::default()
                .with_objects(objects)
                .with_next_page_token(encode_page_token(&PageToken {
                    owner: owner_address,
                    object_type: type_filter,
                    cursor: last_cursor.expect("objects is non-empty"),
                }));
            return Ok(response);
        }

        last_cursor = Some(item_cursor);
        objects.push(merged);
        size_bytes += item_size;

        if objects.len() >= page_size {
            break;
        }
    }

    // Check if there are more items.
    let has_more = iter.next().transpose().map_err(RpcError::from)?.is_some();

    let mut response = ListOwnedObjectsResponse::default().with_objects(objects);
    if has_more {
        if let Some(cursor) = last_cursor {
            response = response.with_next_page_token(encode_page_token(&PageToken {
                owner: owner_address,
                object_type: type_filter,
                cursor,
            }));
        }
    }

    Ok(response)
}
