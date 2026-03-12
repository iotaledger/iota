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
mod metadata;

pub use common::{Error, Result, RpcStatus};
pub(crate) use common::{
    ProtoResult, TryFromProtoError, build_proto_transaction, field_mask_with_default,
};
pub use iota_grpc_types::read_masks::{
    EXECUTE_TRANSACTION_READ_MASK, GET_CHECKPOINT_READ_MASK, GET_EPOCH_READ_MASK,
    GET_OBJECTS_READ_MASK, GET_SERVICE_INFO_READ_MASK, GET_TRANSACTIONS_READ_MASK,
    SIMULATE_TRANSACTION_READ_MASK,
};
pub use metadata::MetadataEnvelope;

/// Response for a checkpoint query.
///
/// Contains checkpoint summary, signature, contents, transactions, and events.
/// Fields are proto types that can be accessed directly or converted to SDK
/// types using their conversion methods (e.g.,
/// `response.summary()?.summary()?`, `response.contents()?.contents()?`).
#[derive(Debug, Clone)]
pub struct CheckpointResponse {
    /// The checkpoint sequence number.
    pub sequence_number: CheckpointSequenceNumber,
    /// Proto checkpoint summary. Use `response.summary()?.summary()` to convert
    /// to SDK type.
    pub summary: Option<iota_grpc_types::v0::checkpoint::CheckpointSummary>,
    /// Proto validator signature. Use `response.signature()?.signature()` to
    /// convert to SDK type.
    pub signature: Option<iota_grpc_types::v0::signatures::ValidatorAggregatedSignature>,
    /// Proto checkpoint contents. Use `response.contents()?.contents()` to
    /// convert to SDK type.
    pub contents: Option<iota_grpc_types::v0::checkpoint::CheckpointContents>,
    /// Proto executed transactions. Use methods like `tx.effects()?`,
    /// `tx.transaction()?`, etc.
    pub executed_transactions: Vec<iota_grpc_types::v0::transaction::ExecutedTransaction>,
    /// Proto events. Use `event.try_into()` or `event.events()` to convert to
    /// SDK types.
    pub events: Vec<iota_grpc_types::v0::event::Event>,
}

impl CheckpointResponse {
    pub fn sequence_number(&self) -> CheckpointSequenceNumber {
        self.sequence_number
    }

    pub fn summary(&self) -> Result<&iota_grpc_types::v0::checkpoint::CheckpointSummary> {
        self.summary
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("summary").into())
    }

    pub fn signature(
        &self,
    ) -> Result<&iota_grpc_types::v0::signatures::ValidatorAggregatedSignature> {
        self.signature
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("signature").into())
    }

    pub fn contents(&self) -> Result<&iota_grpc_types::v0::checkpoint::CheckpointContents> {
        self.contents
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing("contents").into())
    }

    pub fn executed_transactions(
        &self,
    ) -> &Vec<iota_grpc_types::v0::transaction::ExecutedTransaction> {
        &self.executed_transactions
    }

    pub fn events(&self) -> &Vec<iota_grpc_types::v0::event::Event> {
        &self.events
    }

    pub fn checkpoint_data(&self) -> Result<iota_sdk_types::checkpoint::CheckpointData> {
        Ok(iota_sdk_types::checkpoint::CheckpointData {
            checkpoint_contents: self.contents()?.contents()?,
            checkpoint_summary: iota_sdk_types::SignedCheckpointSummary {
                checkpoint: self.summary()?.summary()?,
                signature: self.signature()?.signature()?,
            },
            transactions: self
                .executed_transactions()
                .iter()
                .map(TryInto::try_into)
                .collect::<std::result::Result<Vec<_>, _>>()?,
        })
    }
}
