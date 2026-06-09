// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Local IOTA Move VM SDK.
//!
//! A three-part surface for running and inspecting transactions against the
//! same Move execution engine a full node uses, with no network connection:
//!
//! 1. **Store** — an object store ([`Store`] trait, [`InMemoryStore`]).
//! 2. **Execute** — the [`LocalVm`] executor running in one of three
//!    [`ExecutionMode`]s.
//! 3. **Inspect** — the [`ExecutionResult`] / [`DecodedEvent`] outputs.
//!
//! # Features
//!
//! Both are off by default; enable one to pull in its networked store.
//!
//! - `grpc` — a [`GrpcStore`](grpc::GrpcStore) that resolves objects on demand
//!   from a node via gRPC, caching them in an [`InMemoryStore`].
//! - `graphql` — a [`GraphqlStore`](graphql::GraphqlStore) over GraphQL.
//! - `wasm-bindgen` — a JS-facing surface for the browser.
//!
//! The networked stores and their heavy dependencies are additionally
//! target-gated to `cfg(not(target_arch = "wasm32"))`, so the wasm bundle never
//! sees tonic / reqwest / the gRPC client.

mod debug;
mod error;
mod executor;
mod store;

#[cfg(all(any(feature = "grpc", feature = "graphql"), not(target_arch = "wasm32")))]
mod caching;

#[cfg(all(feature = "grpc", not(target_arch = "wasm32")))]
pub mod grpc;

#[cfg(all(feature = "graphql", not(target_arch = "wasm32")))]
pub mod graphql;

#[cfg(all(feature = "wasm-bindgen", target_arch = "wasm32"))]
mod wasm;

#[cfg(all(feature = "wasm-bindgen", target_arch = "wasm32"))]
mod wasm_store;

// --- SDK surface ---------------------------------------------------------
pub use debug::{DebugArtifacts, DebugConfig, ProfileOutput, ProfileSink};
pub use error::{ExecutionError, SignatureError, StoreError, ValidationError, VmError, VmSdkError};
pub use executor::{
    ChainContext, CommandResult, DecodedEvent, ExecuteOptions, ExecutionMode, ExecutionResult,
    LocalVm, SignatureStatus,
};
// --- Re-exports of upstream types in the public API ----------------------
pub use iota_protocol_config::{Chain, ProtocolVersion};
pub use iota_sdk_types::{Address, ObjectId, StructTag, TypeTag, Version};
pub use iota_types::{
    effects::{TransactionEffects, TransactionEvents},
    move_authenticator::MoveAuthenticator,
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData},
};
pub use store::{InMemoryStore, Store};
