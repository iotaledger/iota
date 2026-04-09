// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Get a random number generator.
#[inline(always)]
pub fn get_rng() -> impl rand::Rng {
    rand::thread_rng()
}
