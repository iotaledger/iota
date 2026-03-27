// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::{
    field::FieldMaskTree,
    read_masks::LIST_DYNAMIC_FIELDS_READ_MASK,
    v1::{
        bcs::BcsData,
        dynamic_field::DynamicField,
        state_service::{ListDynamicFieldsRequest, ListDynamicFieldsResponse},
    },
};
use iota_types::{base_types::ObjectID, dynamic_field::visitor as DFV};
use prost::Message;
use serde::{Deserialize, Serialize};

use crate::{
    constants::validate_max_message_size,
    error::RpcError,
    merge::Merge,
    types::GrpcReader,
    validation::{
        decode_page_token, encode_page_token, page_token_mismatch, require_object_id,
        validate_page_size, validate_read_mask,
    },
};

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 1000;

#[derive(Serialize, Deserialize)]
struct PageToken {
    parent: ObjectID,
    cursor: ObjectID,
}

/// Check whether the read mask requests any field that requires loading the
/// actual field object from storage (as opposed to index-only fields).
fn should_load_field(mask: &FieldMaskTree) -> bool {
    // These fields can only be populated by loading and deserializing the
    // `Field<Name, Value>` object.
    [
        DynamicField::VALUE_FIELD.name,
        DynamicField::VALUE_TYPE_FIELD.name,
        DynamicField::FIELD_OBJECT_FIELD.name,
        DynamicField::CHILD_OBJECT_FIELD.name,
    ]
    .into_iter()
    .any(|field| mask.contains(field))
}

/// Load the field object and populate heavy fields (`value`, `value_type`,
/// `field_object`, `child_object`) on the proto message based on the read mask.
///
/// On recoverable errors (missing object, missing layout, missing child
/// object), logs a warning and returns `Ok(())` — the caller should still
/// include the item with whatever index-only fields are already set.
/// Only returns `Err` for hard storage errors that should abort the request.
fn load_dynamic_field(
    reader: &GrpcReader,
    field_id: &ObjectID,
    read_mask: &FieldMaskTree,
    message: &mut DynamicField,
) -> Result<(), RpcError> {
    let Some(field_object) = reader.get_object(field_id).map_err(RpcError::from)? else {
        tracing::debug!(
            "dynamic field object {field_id} referenced by index but not found in object store"
        );
        return Ok(());
    };

    let Some(move_object) = field_object.data.try_as_move() else {
        return Ok(());
    };

    // Only proceed if this is actually a `Field<Name, Value>` type.
    if !move_object.type_().is_dynamic_field() {
        return Ok(());
    }

    let struct_tag: move_core_types::language_storage::StructTag =
        move_object.type_().clone().into();
    let layout = match reader
        .get_type_layout(&iota_types::TypeTag::Struct(Box::new(struct_tag)))
        .map_err(RpcError::from)?
    {
        Some(layout) => layout,
        None => {
            tracing::warn!(
                "unable to load layout for dynamic field object {field_id}, \
                 returning index-only fields"
            );
            return Ok(());
        }
    };

    let field = DFV::FieldVisitor::deserialize(move_object.contents(), &layout)
        .map_err(|e| RpcError::from(e).with_context("failed to deserialize dynamic field"))?;

    if read_mask.contains(DynamicField::VALUE_FIELD.name) {
        message.value = Some(BcsData::default().with_data(field.value_bytes.to_vec()));
    }

    if let Some(submask) = read_mask.subtree(DynamicField::FIELD_OBJECT_FIELD.name) {
        let merged = crate::merge::Merge::merge_from(field_object.clone(), &submask)
            .map_err(|e: RpcError| e.with_context("failed to merge field object"))?;
        message.field_object = Some(merged);
    }

    match field
        .value_metadata()
        .map_err(|e| RpcError::from(anyhow::Error::from(e)))?
    {
        DFV::ValueMetadata::DynamicField(type_tag) => {
            if read_mask.contains(DynamicField::VALUE_TYPE_FIELD.name) {
                message.value_type = Some(type_tag.to_canonical_string(true));
            }
        }
        DFV::ValueMetadata::DynamicObjectField(object_id) => {
            if read_mask.contains(DynamicField::VALUE_TYPE_FIELD.name)
                || read_mask.contains(DynamicField::CHILD_OBJECT_FIELD.name)
            {
                // Missing child object is recoverable (eventually-consistent
                // index) — return the item with index-only fields.
                let Some(child_object) = reader.get_object(&object_id).map_err(RpcError::from)?
                else {
                    tracing::warn!(
                        "child object {object_id} referenced by dynamic field {field_id} \
                         not found, returning index-only fields"
                    );
                    return Ok(());
                };

                // For DynamicObjectField entries, `value` contains the
                // BCS-encoded ObjectID of the child (the on-chain
                // `Field<Name, ID>` wrapper), while `value_type` is set to
                // the child object's actual type (e.g.
                // `0x2::coin::Coin<0x2::iota::IOTA>`).
                //
                // Clients should use `child_object` to access the full
                // object, not BCS-decode `value` using `value_type`.
                if read_mask.contains(DynamicField::VALUE_TYPE_FIELD.name) {
                    if let Some(struct_tag) = child_object.struct_tag() {
                        let type_tag = iota_types::TypeTag::from(struct_tag);
                        message.value_type = Some(type_tag.to_canonical_string(true));
                    }
                }

                if let Some(submask) = read_mask.subtree(DynamicField::CHILD_OBJECT_FIELD.name) {
                    let merged = crate::merge::Merge::merge_from(child_object, &submask)
                        .map_err(|e: RpcError| e.with_context("failed to merge child object"))?;
                    message.child_object = Some(merged);
                }
            }
        }
    }

    Ok(())
}

#[tracing::instrument(skip(reader))]
pub(crate) fn list_dynamic_fields(
    reader: Arc<GrpcReader>,
    ListDynamicFieldsRequest {
        parent,
        page_size,
        page_token,
        read_mask,
        max_message_size_bytes,
        ..
    }: ListDynamicFieldsRequest,
) -> Result<ListDynamicFieldsResponse, RpcError> {
    let parent_id = require_object_id(&parent, "parent")?;
    let read_mask = validate_read_mask::<DynamicField>(read_mask, LIST_DYNAMIC_FIELDS_READ_MASK)?;
    let page_size = validate_page_size(page_size, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE);
    let max_message_size = validate_max_message_size(max_message_size_bytes)?;

    let page_token: Option<PageToken> = decode_page_token(&page_token)?;
    if let Some(ref t) = page_token {
        if t.parent != parent_id {
            return Err(page_token_mismatch());
        }
    }

    let cursor = page_token.map(|t| t.cursor);
    let load_field = should_load_field(&read_mask);

    let mut iter = reader.dynamic_field_iter(parent_id, cursor)?;

    let mut items = Vec::with_capacity(page_size);
    let mut size_bytes = 0usize;
    let mut last_field_id: Option<ObjectID> = None;

    for result in iter.by_ref() {
        let (key, info) = result.map_err(RpcError::from)?;
        let field_id = key.field_id;

        let mut df = DynamicField::merge_from((key, info), &read_mask)
            .map_err(|e| e.with_context("failed to merge dynamic field"))?;

        // Conditionally load the field object to populate heavy fields.
        // On recoverable errors (missing layout, deserialization failure),
        // the item is still returned with index-only fields populated so
        // that clients see all items and can detect partial data via the
        // absence of the requested heavy fields.
        if load_field {
            if let Err(e) = load_dynamic_field(&reader, &field_id, &read_mask, &mut df) {
                tracing::warn!("error loading dynamic field object {field_id}: {e}");
                // Return the item with index-only fields rather than
                // silently dropping it.
            }
        }

        let item_size = df.encoded_len();

        // If adding this item would exceed the message size limit, stop.
        // Always include at least one item to guarantee forward progress.
        if !items.is_empty() && size_bytes + item_size > max_message_size {
            // The current item doesn't fit — it becomes the start of the next page.
            // Use last_field_id as cursor (the last successfully added item).
            let response = ListDynamicFieldsResponse::default()
                .with_dynamic_fields(items)
                .with_next_page_token(encode_page_token(&PageToken {
                    parent: parent_id,
                    cursor: last_field_id.expect("items is non-empty"),
                }));
            return Ok(response);
        }

        items.push(df);
        size_bytes += item_size;
        last_field_id = Some(field_id);

        if items.len() >= page_size {
            break;
        }
    }

    // Check if there are more items beyond what we returned.
    let has_more = iter.next().transpose().map_err(RpcError::from)?.is_some();

    let mut response = ListDynamicFieldsResponse::default().with_dynamic_fields(items);
    if has_more {
        if let Some(cursor_id) = last_field_id {
            response = response.with_next_page_token(encode_page_token(&PageToken {
                parent: parent_id,
                cursor: cursor_id,
            }));
        }
    }

    Ok(response)
}
