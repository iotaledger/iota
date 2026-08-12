// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A `Multiaddr` newtype over the `multiaddr` crate.
//!
//! The public API differs by target. On wasm32 only the parts needed to
//! type-check and serialize on-chain validator metadata are available; the
//! address-parsing and networking helpers are native-only, since they pull in
//! dependencies that do not build for wasm. See the module docs of [`wasm`]
//! for what the wasm build provides and why.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;
