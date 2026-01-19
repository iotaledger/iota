// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod backoff;
pub mod logging;
#[cfg(feature = "metrics")]
pub mod metrics;
pub mod random;
pub mod stream_ext;
pub mod sync;
pub mod try_iterator_ext;
pub mod util;

pub use iota_types::scoring_metrics;

#[inline(always)]
pub fn in_test_configuration() -> bool {
    cfg!(msim) || cfg!(debug_assertions)
}
