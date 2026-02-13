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
    CHECKPOINT_READ_MASK, EPOCH_READ_MASK, EXECUTION_READ_MASK, Error, OBJECTS_READ_MASK, Result,
    SERVICE_INFO_READ_MASK, TRANSACTIONS_READ_MASK,
};
pub(crate) use common::{
    ProtoResult, TryFromProtoError, build_proto_transaction, field_mask_with_default,
};
// Re-export proto types as the primary API
pub use iota_grpc_types::v0::{
    checkpoint::Checkpoint,
    epoch::Epoch,
    event::Event,
    ledger_service::GetServiceInfoResponse,
    object::{Object, Objects},
    transaction::{ExecutedTransaction, Transaction, TransactionEffects, TransactionEvents},
};

/// Response for a checkpoint query.
///
/// Contains checkpoint summary, signature, contents, transactions, and events.
/// Fields are proto types that can be lazily converted to SDK types using their
/// conversion methods (e.g., `response.summary()?`, `response.effects()?`).
#[derive(Debug, Clone)]
pub struct CheckpointResponse {
    /// The checkpoint sequence number.
    pub sequence_number: CheckpointSequenceNumber,
    /// Proto checkpoint summary. Use `summary.summary()` to convert to SDK
    /// type.
    pub summary: Option<iota_grpc_types::v0::checkpoint::CheckpointSummary>,
    /// Proto validator signature. Use TryInto or
    /// ValidatorAggregatedSignature::try_from to convert.
    pub signature: Option<iota_grpc_types::v0::signatures::ValidatorAggregatedSignature>,
    /// Proto checkpoint contents. Use `contents.contents()` to convert to
    /// SDK type.
    pub contents: Option<iota_grpc_types::v0::checkpoint::CheckpointContents>,
    /// Proto executed transactions. Use methods like `tx.effects()?`,
    /// `tx.transaction()?`, etc.
    pub transactions: Vec<ExecutedTransaction>,
    /// Proto events. Use `event.events()` to convert to SDK type.
    pub events: Vec<Event>,
}

impl CheckpointResponse {
    pub fn sequence_number(&self) -> CheckpointSequenceNumber {
        self.sequence_number
    }

    pub fn summary(&self) -> Result<iota_sdk_types::checkpoint::CheckpointSummary> {
        self.summary
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("summary"))?
            .try_into()
            .map_err(Into::into)
    }

    pub fn signature(&self) -> Result<iota_sdk_types::ValidatorAggregatedSignature> {
        self.signature
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("signature"))?
            .try_into()
            .map_err(Into::into)
    }

    pub fn contents(&self) -> Result<iota_sdk_types::checkpoint::CheckpointContents> {
        self.contents
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("contents"))?
            .try_into()
            .map_err(Into::into)
    }

    pub fn transactions(&self) -> Result<Vec<&ExecutedTransaction>> {
        Ok(self.transactions.iter().collect())
    }

    pub fn events(&self) -> Result<Vec<iota_sdk_types::Event>> {
        self.events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                e.try_into().map_err(|e: TryFromProtoError| {
                    e.nested_at(iota_grpc_types::v0::event::Events::EVENTS_FIELD.name, i)
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn checkpoint_data(&self) -> Result<iota_sdk_types::checkpoint::CheckpointData> {
        Ok(iota_sdk_types::checkpoint::CheckpointData {
            checkpoint_contents: self.contents()?,
            checkpoint_summary: iota_sdk_types::SignedCheckpointSummary {
                checkpoint: self.summary()?,
                signature: self.signature()?,
            },
            transactions: self
                .transactions()?
                .into_iter()
                .map(TryInto::try_into)
                .collect::<std::result::Result<Vec<_>, _>>()?,
        })
    }
}
