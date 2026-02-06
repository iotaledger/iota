// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use crate::GrpcReader;

pub fn render_json(
    grpc_reader: Arc<GrpcReader>,
    max_json_move_value_size: usize,
    type_tag: &iota_types::TypeTag,
    contents: &[u8],
) -> Option<prost_types::Value> {
    let layout = grpc_reader.get_type_layout(type_tag).ok().flatten()?;

    iota_types::proto_value::ProtoVisitorBuilder::new(max_json_move_value_size)
        .deserialize_value(contents, &layout)
        .map_err(|e| tracing::debug!("unable to convert move value to JSON: {e}"))
        .ok()
}
