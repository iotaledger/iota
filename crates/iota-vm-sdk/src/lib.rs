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
//! All are off by default.
//!
//! - `grpc` — a `GrpcStore` that resolves objects on demand from a node via
//!   gRPC, caching them in an [`InMemoryStore`].
//! - `graphql` — a `GraphQLStore` over GraphQL.
//! - `tracing` — compile the Move VM gas profiler and instruction tracer into
//!   the engine so [`DebugConfig::with_profiling`] /
//!   [`DebugConfig::with_tracing`] capture output.

mod debug;
mod error;
mod executor;
mod store;

#[cfg(any(feature = "grpc", feature = "graphql"))]
mod caching;

#[cfg(feature = "grpc")]
pub mod grpc;

#[cfg(feature = "graphql")]
pub mod graphql;

pub use debug::{DebugArtifacts, DebugConfig, ProfileOutput, ProfileSink};
pub use error::{ExecutionError, SignatureError, StoreError, ValidationError, VmError, VmSdkError};
pub use executor::{
    ChainContext, CommandResult, DecodedEvent, ExecuteOptions, ExecutionMode, ExecutionResult,
    LocalVm, SignatureStatus,
};
// Upstream types re-exported in the public API.
pub use iota_config::transaction_deny_config::{
    TransactionDenyConfig, TransactionDenyConfigBuilder,
};
pub use iota_protocol_config::{Chain, ProtocolVersion};
pub use iota_sdk_types::{Address, MoveAuthenticator, ObjectId, StructTag, TypeTag, Version};
pub use iota_types::{
    effects::{TransactionEffects, TransactionEvents},
    object::Object,
    signature::UserSignature,
    transaction::{SenderSignedData, TransactionData},
};
pub use store::{InMemoryStore, Store};
