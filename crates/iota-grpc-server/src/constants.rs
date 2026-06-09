// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{google::rpc::bad_request::FieldViolation, v1::error_reason::ErrorReason};

use crate::error::RpcError;

/// Default maximum message size for chunked responses (4MB)
pub const DEFAULT_MAX_MESSAGE_SIZE_BYTES: usize = 4 * 1024 * 1024;

/// Minimum allowed message size (1MB)
pub const MIN_MESSAGE_SIZE_BYTES: usize = 1024 * 1024;

/// Maximum allowed message size (128MB)
pub const MAX_MESSAGE_SIZE_BYTES: usize = 128 * 1024 * 1024;

/// Validates and converts the max_message_size_bytes parameter.
///
/// Accepts `Option<u32>` (the proto field type) and converts internally.
pub fn validate_max_message_size(max_message_size_bytes: Option<u32>) -> Result<usize, RpcError> {
    match max_message_size_bytes {
        Some(size) => {
            let size = size as usize;

            match size {
                s if s < MIN_MESSAGE_SIZE_BYTES => {
                    Err(FieldViolation::new("max_message_size_bytes")
                        .with_description(format!(
                            "must be at least {MIN_MESSAGE_SIZE_BYTES} bytes"
                        ))
                        .with_reason(ErrorReason::FieldInvalid)
                        .into())
                }
                s if s > MAX_MESSAGE_SIZE_BYTES => {
                    Err(FieldViolation::new("max_message_size_bytes")
                        .with_description(format!("must be at most {MAX_MESSAGE_SIZE_BYTES} bytes"))
                        .with_reason(ErrorReason::FieldInvalid)
                        .into())
                }
                s => Ok(s),
            }
        }
        None => Ok(DEFAULT_MAX_MESSAGE_SIZE_BYTES),
    }
}
