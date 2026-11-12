// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::{Identifier, MAX_IDENTIFIER_LENGTH, StructTag, TypeParseError, TypeTag};
use iota_types::transaction::{
    move_core_identifier_validity_check, type_tags_identifiers_validity_check,
};

use crate::GrpcReader;

/// Parses the optional module or function name of a filter. `field` names it
/// in the error.
pub(crate) fn parse_filter_identifier(
    name: Option<String>,
    field: &str,
) -> Result<Option<Identifier>, String> {
    name.map(|name| {
        // The move-core check rejects `<SELF>`, which the SDK parser accepts,
        // and keeps invalid names away from the SDK parser, whose errors copy
        // the rest of the input.
        if name.len() <= MAX_IDENTIFIER_LENGTH
            && move_core_types::identifier::Identifier::is_valid(&name)
        {
            Ok(Identifier::new_unchecked(name))
        } else {
            Err(format!(
                "invalid {field} name: {}",
                quote_filter_input(&name)
            ))
        }
    })
    .transpose()
}

/// Parses the struct tag of an event filter. For a syntax error, the error
/// gives the byte offset and the input where parsing stopped.
pub(crate) fn parse_filter_struct_tag(struct_tag: &str) -> Result<StructTag, String> {
    let tag: StructTag = struct_tag.parse().map_err(|error| match error {
        // `input` is the rest of the struct tag from where parsing stopped.
        TypeParseError::Parse { input, .. } => format!(
            "invalid struct tag: unexpected input at byte {}: {}",
            struct_tag.len().saturating_sub(input.len()),
            quote_filter_input(&input)
        ),
        error => format!("invalid struct tag: {error}"),
    })?;
    // The SDK parser accepts `<SELF>`, which never names a module or type.
    move_core_identifier_validity_check(tag.module())
        .and_then(|()| move_core_identifier_validity_check(tag.name()))
        .and_then(|()| type_tags_identifiers_validity_check(tag.type_params()))
        .map_err(|_| format!("invalid struct tag: {}", quote_filter_input(struct_tag)))?;
    Ok(tag)
}

/// Quotes a filter value for an error message. Values longer than
/// [`MAX_IDENTIFIER_LENGTH`] bytes are cut to that length before quoting,
/// which escapes control characters.
pub(crate) fn quote_filter_input(value: &str) -> String {
    if value.len() <= MAX_IDENTIFIER_LENGTH {
        return format!("{value:?}");
    }
    let end = value.floor_char_boundary(MAX_IDENTIFIER_LENGTH);
    format!("{:?}... ({} bytes)", &value[..end], value.len())
}

/// Per-item overhead when an item is placed in a protobuf `repeated`
/// length-delimited field (field number 1-15):
///   field_tag (1 byte) + length_delimiter_varint
pub(crate) fn repeated_field_item_overhead(item_encoded_len: usize) -> usize {
    1 + prost::length_delimiter_len(item_encoded_len)
}

/// Wrapper overhead for a `CheckpointData` oneof message wrapping an inner
/// message (e.g. `ExecutedTransactions` or `Events`).
///
/// Encoding: field_tag (1 byte) + length_delimiter(inner_encoded_len).
pub(crate) fn checkpoint_data_wrapper_overhead(inner_encoded_len: usize) -> usize {
    1 + prost::length_delimiter_len(inner_encoded_len)
}

/// Exact overhead for `has_next: true` in `GetObjectsResponse` /
/// `GetTransactionsResponse`. A proto3 bool field set to `true` encodes as
/// field_tag (1 byte) + varint(1) (1 byte) = 2 bytes.
pub(crate) const HAS_NEXT_TRUE_OVERHEAD: usize = 2;

pub(crate) fn render_json(
    grpc_reader: Arc<GrpcReader>,
    max_json_move_value_size: usize,
    type_tag: &TypeTag,
    contents: &[u8],
) -> Option<prost_types::Value> {
    // JSON rendering is best-effort - log errors but don't fail the request
    let layout = grpc_reader
        .get_type_layout(type_tag)
        .map_err(|e| tracing::debug!("unable to get type layout for JSON rendering: {e}"))
        .ok()
        .flatten()?;

    iota_types::proto_value::ProtoVisitorBuilder::new(max_json_move_value_size)
        .deserialize_value(contents, &layout)
        .map_err(|e| tracing::debug!("unable to convert move value to JSON: {e}"))
        .ok()
}
