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

/// An item from a checkpoint data stream.
///
/// When `filter_checkpoints` is enabled, the server may skip checkpoints that
/// don't match the provided filters. In that case, `Progress` items are sent
/// periodically to indicate liveness and the current scan position. When
/// `filter_checkpoints` is disabled, only `Checkpoint` items are produced.
///
/// For liveness detection with `filter_checkpoints`, wrap `stream.next()` in
/// `tokio::time::timeout()` — if neither a `Checkpoint` nor a `Progress`
/// arrives within your chosen duration plus some buffer for connection latency,
/// the connection is likely dead.
#[derive(Debug, Clone)]
pub enum CheckpointStreamItem {
    /// A complete checkpoint with its transactions and events.
    Checkpoint(Box<CheckpointResponse>),
    /// A progress indicator sent during filtered scanning.
    /// Contains the sequence number of the latest scanned checkpoint.
    Progress {
        latest_scanned_sequence_number: CheckpointSequenceNumber,
    },
}

impl CheckpointStreamItem {
    /// Returns the contained checkpoint, or `None` if this is a progress
    /// message.
    pub fn into_checkpoint(self) -> Option<CheckpointResponse> {
        match self {
            Self::Checkpoint(c) => Some(*c),
            Self::Progress { .. } => None,
        }
    }

    /// Returns the progress sequence number, or `None` if this is a
    /// checkpoint.
    pub fn into_progress(self) -> Option<CheckpointSequenceNumber> {
        match self {
            Self::Checkpoint(_) => None,
            Self::Progress {
                latest_scanned_sequence_number,
            } => Some(latest_scanned_sequence_number),
        }
    }

    /// Returns `true` if this is a checkpoint item.
    pub fn is_checkpoint(&self) -> bool {
        matches!(self, Self::Checkpoint(_))
    }

    /// Returns `true` if this is a progress item.
    pub fn is_progress(&self) -> bool {
        matches!(self, Self::Progress { .. })
    }
}

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
