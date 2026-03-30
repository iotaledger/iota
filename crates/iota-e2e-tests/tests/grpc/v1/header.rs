// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::headers;
use tonic::metadata::MetadataMap;

/// Helper function to verify that all expected IOTA headers are present in the
/// response metadata
pub fn verify_iota_headers(metadata: &MetadataMap, operation_name: &str) {
    let required_headers = [
        headers::X_IOTA_CHAIN,
        headers::X_IOTA_CHAIN_ID,
        headers::X_IOTA_CHECKPOINT_HEIGHT,
        headers::X_IOTA_EPOCH,
        headers::X_IOTA_TIMESTAMP_MS,
        headers::X_IOTA_TIMESTAMP,
        headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT,
        headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT_OBJECTS,
        headers::X_IOTA_SERVER,
    ];

    for header_name in &required_headers {
        assert!(
            metadata.get(*header_name).is_some(),
            "{operation_name} response should contain {header_name} header",
        );
    }
}

/// Helper function to parse a u64 header value
pub fn parse_u64_header(metadata: &MetadataMap, header_name: &str) -> u64 {
    metadata
        .get(header_name)
        .unwrap_or_else(|| panic!("{header_name} header should be present"))
        .to_str()
        .unwrap_or_else(|_| panic!("{header_name} header should be valid UTF-8"))
        .parse()
        .unwrap_or_else(|_| panic!("{header_name} header should be a valid u64"))
}
