// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{google::rpc::bad_request::FieldViolation, v1::error_reason::ErrorReason};

use crate::error::RpcError;

/// Lowest `iota-sdk-grpc-client` version able to decode this node's gRPC
/// responses, sent in the
/// [`X_IOTA_MIN_SDK_VERSION`](iota_grpc_types::headers::X_IOTA_MIN_SDK_VERSION)
/// header of every response.
///
/// Raise it to the SDK release that ships a wire-visible change (a new enum
/// variant, a new `oneof` case, ...) once that release is out, so clients on
/// an older SDK fail their calls instead of choking on values they cannot
/// decode.
pub const MIN_SDK_VERSION: &str = "1.0.0-beta.1";

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

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::*;

    #[test]
    fn min_sdk_version_is_valid_semver() {
        Version::parse(MIN_SDK_VERSION).unwrap();
    }

    /// The e2e tests reach the node through the bundled SDK client, so a
    /// minimum above its version would fail every one of them.
    #[test]
    fn min_sdk_version_is_satisfied_by_the_bundled_client() {
        let minimum = Version::parse(MIN_SDK_VERSION).unwrap();
        let bundled = Version::parse(iota_grpc_client::VERSION).unwrap();
        assert!(
            minimum <= bundled,
            "MIN_SDK_VERSION {minimum} exceeds the bundled iota-sdk-grpc-client {bundled}"
        );
    }
}
