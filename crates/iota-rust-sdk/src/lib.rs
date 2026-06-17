// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Re-exports of the [`iota-rust-sdk`](https://github.com/iotaledger/iota-rust-sdk)
//! crates pinned by this repository.
//!
//! Depend on this crate (instead of declaring your own `iota-rust-sdk` git
//! dependency) to use those crates version-matched to the revision this
//! repository pins. The module layout and feature flags mirror the upstream
//! `iota-sdk` meta-crate.
//!
//! To actually get version unification, reach these crates only through this
//! crate's re-exports; adding a separate `iota-rust-sdk` git dependency at a
//! different revision reintroduces incompatible duplicate types.

#[cfg(feature = "grpc")]
pub use iota_grpc_client as grpc_client;
#[cfg(feature = "grpc")]
pub use iota_grpc_types as grpc_types;
#[cfg(feature = "crypto")]
pub use iota_sdk_crypto as crypto;
#[cfg(feature = "graphql")]
pub use iota_sdk_graphql_client as graphql_client;
#[cfg(feature = "txn-builder")]
pub use iota_sdk_transaction_builder as transaction_builder;
#[cfg(feature = "types")]
pub use iota_sdk_types as types;
