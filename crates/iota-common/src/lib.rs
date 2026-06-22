// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(target_arch = "wasm32"))]
pub mod backoff;
pub mod logging;
pub mod moving_window;
pub mod random;
#[cfg(not(target_arch = "wasm32"))]
pub mod random_util;
#[cfg(not(target_arch = "wasm32"))]
pub mod stream_ext;
#[cfg(not(target_arch = "wasm32"))]
pub mod sync;
pub mod try_iterator_ext;
#[cfg(not(target_arch = "wasm32"))]
pub use random_util::tempdir;

#[inline(always)]
pub fn in_test_configuration() -> bool {
    cfg!(msim) || cfg!(debug_assertions)
}
