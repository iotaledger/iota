// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Re-exports of the [`iota-rust-sdk`](https://github.com/iotaledger/iota-rust-sdk)
//! crates at the version this repository pins.
//!
//! Depend on this crate (instead of declaring your own `iota-rust-sdk`
//! dependency) to use those crates version-matched to the version this
//! repository pins. The module layout and feature flags mirror the upstream
//! `iota-sdk` meta-crate.
//!
//! To actually get version unification, reach these crates only through this
//! crate's re-exports; adding a separate `iota-rust-sdk` dependency at a
//! different version reintroduces incompatible duplicate types.

#[cfg(feature = "crypto")]
pub use rust_sdk::crypto;
#[cfg(feature = "graphql")]
pub use rust_sdk::graphql_client;
#[cfg(feature = "grpc")]
pub use rust_sdk::grpc_client;
#[cfg(feature = "grpc")]
pub use rust_sdk::grpc_types;
#[cfg(feature = "transaction-builder")]
pub use rust_sdk::transaction_builder;
#[cfg(feature = "types")]
pub use rust_sdk::types;
