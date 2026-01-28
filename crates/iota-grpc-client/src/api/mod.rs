// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for gRPC client operations.
//!
//! This module provides wrappers around the raw gRPC service clients.
//! Proto types are exposed directly with lazy conversion methods, allowing
//! users to convert only what they need to SDK types.

use iota_sdk_types::CheckpointSequenceNumber;

mod common;
pub mod execution;
pub mod ledger;

pub use common::{
    CHECKPOINT_DATA_READ_MASK, EXECUTION_READ_MASK, Error, OBJECTS_READ_MASK, Result,
    TRANSACTIONS_READ_MASK,
};
pub(crate) use common::{
    ProtoResult, TryFromProtoError, build_proto_transaction, field_mask_with_default,
};
// Re-export proto types as the primary API
pub use iota_grpc_types::v0::{
    checkpoint::Checkpoint,
    event::Event,
    object::{Object, Objects},
    transaction::{ExecutedTransaction, Transaction, TransactionEffects, TransactionEvents},
};

/// Response for a full checkpoint data query.
///
/// Contains checkpoint summary, optional contents, transactions, and events.
/// Fields are proto types that can be lazily converted to SDK types using their
/// conversion methods (e.g., `summary.sdk_summary()?`, `tx.sdk_effects()?`).
#[derive(Debug, Clone)]
pub struct CheckpointResponse {
    /// The checkpoint sequence number.
    pub sequence_number: CheckpointSequenceNumber,
    /// Proto checkpoint summary. Use `summary.deserialize()` to convert to SDK
    /// type.
    pub summary: Option<iota_grpc_types::v0::checkpoint::CheckpointSummary>,
    /// Proto validator signature. Use TryInto or
    /// ValidatorAggregatedSignature::try_from to convert.
    pub signature: Option<iota_grpc_types::v0::signatures::ValidatorAggregatedSignature>,
    /// Proto checkpoint contents. Use `contents.deserialize()` to convert to
    /// SDK type.
    pub contents: Option<iota_grpc_types::v0::checkpoint::CheckpointContents>,
    /// Proto executed transactions. Use methods like `tx.sdk_effects()?`,
    /// `tx.sdk_transaction()?`, etc.
    pub transactions: Vec<Box<ExecutedTransaction>>,
    /// Proto events. Use `event.deserialize()` to convert to SDK type.
    pub events: Vec<Event>,
}
