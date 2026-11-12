// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{google::rpc::bad_request::FieldViolation, v1::error_reason::ErrorReason};

use crate::error::RpcError;

/// Name of the response header carrying
/// [`MIN_IOTA_SDK_GRPC_CLIENT_VERSION`].
// TODO(https://github.com/iotaledger/iota/issues/13598): replace with
// `iota_grpc_types::headers::X_MIN_IOTA_SDK_GRPC_CLIENT_VERSION`
// once an `iota-sdk-grpc-types` release has it, and test that the bundled
// `iota-sdk-grpc-client` accepts the minimum.
pub const X_MIN_IOTA_SDK_GRPC_CLIENT_VERSION: &str = "x-min-iota-sdk-grpc-client-version";

/// Lowest `iota-sdk-grpc-client` version able to decode this node's gRPC
/// responses, sent in the [`X_MIN_IOTA_SDK_GRPC_CLIENT_VERSION`] header of
/// every response.
///
/// Raise it to the SDK release that ships a wire-visible change (a new enum
/// variant, a new `oneof` case, ...) once that release is out, so clients on
/// an older SDK fail their calls instead of choking on values they cannot
/// decode. `scripts/check_min_iota_sdk_client_version.py` fails in CI when a
/// change to the BCS schema of `iota-sdk-types` requires raising it.
pub const MIN_IOTA_SDK_GRPC_CLIENT_VERSION: &str = "1.0.0-rc.1";

/// Maximum number of nested `All`, `Any` and `Not` filters in a transaction or
/// event filter. The event filter of a transaction filter adds one level.
pub const MAX_FILTER_DEPTH: usize = 10;

/// Maximum number of filters in a transaction or event filter, counting `All`,
/// `Any` and `Not` themselves, every filter inside them, and every transaction
/// kind of a `TransactionKind` filter.
pub const MAX_FILTER_NODES: usize = 256;

/// Adds `added` filters at nesting level `depth` to `nodes`, the number of
/// filters converted so far. Fails if `depth` is over [`MAX_FILTER_DEPTH`] or
/// `nodes` goes over [`MAX_FILTER_NODES`].
pub(crate) fn count_filters(depth: usize, nodes: &mut usize, added: usize) -> Result<(), String> {
    if depth > MAX_FILTER_DEPTH {
        return Err(format!(
            "filter nesting exceeds the limit of {MAX_FILTER_DEPTH} levels"
        ));
    }
    *nodes += added;
    if *nodes > MAX_FILTER_NODES {
        return Err(format!(
            "filter exceeds the limit of {MAX_FILTER_NODES} nodes"
        ));
    }
    Ok(())
}

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
    fn min_iota_sdk_grpc_client_version_is_valid_semver() {
        Version::parse(MIN_IOTA_SDK_GRPC_CLIENT_VERSION).unwrap();
    }
}
