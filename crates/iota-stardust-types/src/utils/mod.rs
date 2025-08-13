// Copyright 2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "serde")]
pub mod serde;

#[cfg(feature = "instant")]
pub fn unix_timestamp_now() -> core::time::Duration {
    instant::SystemTime::now()
        .duration_since(instant::SystemTime::UNIX_EPOCH)
        .expect("time went backwards")
}
