// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use prost_types::Timestamp;

/// Convert a millisecond timestamp to a protobuf Timestamp
pub fn timestamp_ms_to_proto(timestamp_ms: u64) -> Timestamp {
    let timestamp = std::time::Duration::from_millis(timestamp_ms);
    Timestamp {
        seconds: timestamp.as_secs() as i64,
        nanos: timestamp.subsec_nanos() as i32,
    }
}
