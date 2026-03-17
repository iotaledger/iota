// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Helper for adding IOTA-specific metadata headers to gRPC responses

use std::sync::Arc;

use iota_grpc_types::headers;

use crate::GrpcReader;

/// Helper struct to add IOTA-specific metadata headers to responses
/// Use this in service handlers to add checkpoint and blockchain metadata
pub fn append_info_headers<T>(
    grpc_reader: Arc<GrpcReader>,
    mut response: tonic::Response<T>,
) -> tonic::Response<T> {
    let headers = response.metadata_mut();

    if let Ok(chain_id) = grpc_reader.get_chain_identifier() {
        if let Ok(chain_id_value) = chain_id.digest().base58_encode().parse() {
            headers.insert(headers::X_IOTA_CHAIN_ID, chain_id_value);
        }

        if let Ok(chain) = chain_id.chain().as_str().parse() {
            headers.insert(headers::X_IOTA_CHAIN, chain);
        }
    }

    if let Ok(latest_checkpoint) = grpc_reader.get_latest_checkpoint() {
        if let Ok(epoch_value) = latest_checkpoint.epoch().to_string().parse() {
            headers.insert(headers::X_IOTA_EPOCH, epoch_value);
        }

        if let Ok(height_value) = latest_checkpoint.sequence_number.to_string().parse() {
            headers.insert(headers::X_IOTA_CHECKPOINT_HEIGHT, height_value);
        }

        if let Ok(timestamp_value) = latest_checkpoint.timestamp_ms.to_string().parse() {
            headers.insert(headers::X_IOTA_TIMESTAMP_MS, timestamp_value);
        }

        headers.insert(
            headers::X_IOTA_TIMESTAMP,
            iota_grpc_types::proto::timestamp_ms_to_proto(latest_checkpoint.timestamp_ms)
                .to_string()
                .try_into()
                .expect("timestamp is a valid MetadataValue<Ascii>"),
        );
    }

    // Add lowest available checkpoint header
    if let Ok(lowest_checkpoint) = grpc_reader.get_lowest_available_checkpoint() {
        if let Ok(lowest_value) = lowest_checkpoint.to_string().parse() {
            headers.insert(headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT, lowest_value);
        }
    }

    // Add lowest available checkpoint objects header
    if let Ok(lowest_objects) = grpc_reader.get_lowest_available_checkpoint_objects() {
        if let Ok(lowest_objects_value) = lowest_objects.to_string().parse() {
            headers.insert(
                headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT_OBJECTS,
                lowest_objects_value,
            );
        }
    }

    if let Some(server_version) = grpc_reader
        .server_version()
        .and_then(|version| version.parse().ok())
    {
        headers.insert(headers::X_IOTA_SERVER, server_version);
    }

    response
}
