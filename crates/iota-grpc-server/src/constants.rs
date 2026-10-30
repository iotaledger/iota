// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{google::rpc::bad_request::FieldViolation, v1::error_reason::ErrorReason};

use crate::error::RpcError;

/// Maximum number of nested `All`, `Any` and `Not` filters in a transaction or
/// event filter. The event filter of a transaction filter adds one level.
pub const MAX_FILTER_DEPTH: usize = 10;

/// Maximum number of filters in a transaction or event filter, counting `All`,
/// `Any` and `Not` themselves, every filter inside them, and every transaction
/// kind of a `TransactionKind` filter.
pub const MAX_FILTER_NODES: usize = 256;

/// Default maximum message size for chunked responses (4MB)
pub const DEFAULT_MAX_MESSAGE_SIZE_BYTES: usize = 4 * 1024 * 1024;

/// Minimum allowed message size (1MB)
pub const MIN_MESSAGE_SIZE_BYTES: usize = 1024 * 1024;

/// Returns the message size to use for a response: the client-requested
/// `client_max_message_size_bytes`, or [`DEFAULT_MAX_MESSAGE_SIZE_BYTES`] when
/// the client sets none, at most the operator-configured ceiling
/// (`GrpcApiConfig::max_message_size_bytes`).
///
/// Accepts `Option<u32>` (the proto field type) and converts internally.
/// Rejects a client value below [`MIN_MESSAGE_SIZE_BYTES`]; a value above the
/// operator ceiling is clamped down rather than rejected, so a client asking
/// for more than the operator allows still gets a response instead of an
/// error.
pub fn effective_max_message_size(
    client_max_message_size_bytes: Option<u32>,
    operator_max_message_size_bytes: u32,
) -> Result<usize, RpcError> {
    match client_max_message_size_bytes {
        Some(size) => {
            let size = size as usize;

            if size < MIN_MESSAGE_SIZE_BYTES {
                return Err(FieldViolation::new("max_message_size_bytes")
                    .with_description(format!("must be at least {MIN_MESSAGE_SIZE_BYTES} bytes"))
                    .with_reason(ErrorReason::FieldInvalid)
                    .into());
            }

            Ok(size.min(operator_max_message_size_bytes as usize))
        }
        None => Ok(DEFAULT_MAX_MESSAGE_SIZE_BYTES.min(operator_max_message_size_bytes as usize)),
    }
}
