// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use crate::GrpcReader;

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
    type_tag: &iota_types::TypeTag,
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
