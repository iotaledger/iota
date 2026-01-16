// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for gRPC client operations.
//!
//! This module provides ergonomic wrappers around the raw gRPC service clients,
//! allowing users to work with SDK types directly without dealing with proto
//! types.

use iota_sdk_types::{
    CheckpointSequenceNumber, Digest, Object, Transaction, TransactionEffects, TransactionEvents,
    UserSignature,
};

mod common;
pub mod execution;
pub mod ledger;

pub use common::{
    EXECUTION_READ_MASK, Error, OBJECTS_READ_MASK, Result, TRANSACTIONS_READ_MASK,
    TransactionExecutionResponse,
};
pub(crate) use common::{
    ProtoResult, TryFromProtoError, build_proto_transaction, convert_object,
    extract_effects_and_events, extract_execution_response, field_mask_with_default,
};

/// Response for a transaction query.
///
/// Contains the transaction data, signatures, effects, and optional events.
#[derive(Debug, Clone)]
pub struct TransactionResponse {
    /// Transaction digest.
    pub digest: Digest,
    /// The transaction data.
    pub transaction: Transaction,
    /// User signatures on the transaction.
    pub signatures: Vec<UserSignature>,
    /// The effects of executing this transaction.
    pub effects: TransactionEffects,
    /// Events emitted during execution (if available).
    pub events: Option<TransactionEvents>,
    /// Checkpoint that includes this transaction (if finalized).
    pub checkpoint: Option<CheckpointSequenceNumber>,
    /// Timestamp in milliseconds when the transaction was executed (if
    /// available).
    pub timestamp_ms: Option<u64>,
}

/// Response for transaction simulation.
///
/// Contains the simulated effects, events, and objects.
///
/// Unlike [`TransactionExecutionResponse`], simulation always returns input and
/// output objects (as `Vec<Object>` rather than `Option<Vec<Object>>`) because
/// previewing object changes is the primary purpose of simulation.
#[derive(Debug, Clone)]
pub struct TransactionSimulationResponse {
    /// The simulated effects.
    pub effects: TransactionEffects,
    /// Events that would be emitted.
    pub events: Option<TransactionEvents>,
    /// Input objects used by the transaction (always populated for simulation).
    pub input_objects: Vec<Object>,
    /// Output objects that would be created/modified (always populated for
    /// simulation).
    pub output_objects: Vec<Object>,
}

impl From<TransactionExecutionResponse> for TransactionSimulationResponse {
    fn from(data: TransactionExecutionResponse) -> Self {
        Self {
            effects: data.effects,
            events: data.events,
            input_objects: data.input_objects.unwrap_or_default(),
            output_objects: data.output_objects.unwrap_or_default(),
        }
    }
}
