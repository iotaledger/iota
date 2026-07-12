// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Skip-effect-certification futures plus the msim scheduler layers push
// rustc's monomorphization query depth past the default 128 in this test
// binary. See the same attribute in `iota-json-rpc/src/lib.rs` for the
// underlying explanation.
#![recursion_limit = "256"]

mod client;
mod utils;
mod v1;
mod wallet_context;
mod wallet_context_read_methods;
