// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Local IOTA Move VM SDK.
//!
//! A four-part surface for running and inspecting transactions against the same
//! Move execution engine a full node uses, with no network connection:
//!
//! 1. **Decode** — static, VM-free helpers ([`decode_transaction`],
//!    [`decode_move_authenticator`], [`auth_function_field_id`],
//!    [`derive_field_id`]).
//! 2. **Store** — an object store ([`Store`] trait, [`InMemoryStore`]).
//! 3. **Execute** — the [`LocalVm`] executor running in one of three
//!    [`ExecutionMode`]s.
//! 4. **Introspect** — the [`ExecutionResult`] / [`DecodedEvent`] outputs.
//!
//! # Features
//!
//! - `grpc` — a [`GrpcStore`](grpc::GrpcStore) that pre-fetches objects from a
//!   node via gRPC into an [`InMemoryStore`].
//! - `graphql` — a [`GraphqlStore`](graphql::GraphqlStore) over GraphQL.
//!
//! The networked stores and their heavy dependencies are feature-gated off by
//! default.

mod debug;
mod decode;
mod error;
mod executor;
mod store;

#[cfg(feature = "grpc")]
pub mod grpc;

#[cfg(feature = "graphql")]
pub mod graphql;

// --- SDK surface ---------------------------------------------------------

pub use debug::{DebugArtifacts, DebugConfig, ProfileOutput, ProfileSink};
pub use decode::{
    DecodedTransaction, auth_function_field_id, decode_move_authenticator, decode_transaction,
    derive_field_id,
};
pub use error::{
    DecodeError, ExecutionError, SignatureError, ValidationError, VmError, VmSdkError,
};
pub use executor::{
    ChainContext, DecodedEvent, ExecuteOptions, ExecutionMode, ExecutionResult, GasEstimate,
    LocalVm, SignatureStatus,
};
// --- Re-exports of upstream types in the public API ----------------------
pub use iota_protocol_config::{Chain, ProtocolVersion};
pub use iota_sdk_types::{Address as IotaAddress, ObjectId, TypeTag, Version};
pub use iota_types::{
    effects::{TransactionEffects, TransactionEvents},
    move_authenticator::MoveAuthenticator,
    object::Object,
    signature::GenericSignature,
    transaction::{SenderSignedData, TransactionData},
};
pub use store::{InMemoryStore, Store};
