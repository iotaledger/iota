// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversion utilities for historical fallback data.
//!
//! This module provides wrapper types that enable conversion from raw data
//! fetched from historical fallback storage into the `Stored*` or JSON-RPC
//! compatible types used by the Indexer's JSON-RPC API layer.

use std::sync::Arc;

use iota_json_rpc_types::IotaEvent;
use iota_package_resolver::{PackageStore, Resolver};
use iota_types::{
    digests::TransactionDigest,
    effects::TransactionEvents,
    full_checkpoint_content::CheckpointTransaction,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointContentsExt,
        CheckpointSequenceNumber,
    },
    object::Object,
};
use prometheus_filtered::Registry;

use crate::{
    errors::{IndexerError, IndexerResult},
    ingestion::{common::prepare::extract_df_kind, primary::prepare::PrimaryWorker},
    metrics::IndexerMetrics,
    models::{
        checkpoints::StoredCheckpoint,
        events::StoredEvent,
        objects::StoredObject,
        transactions::{StoredTransaction, tx_events_to_iota_tx_events},
    },
    types::{IndexedCheckpoint, IndexedEvent, IndexedObject},
};

/// Alias for an [`Object`] fetched from historical fallback storage.
///
/// Contains all data needed to reconstruct a [`StoredObject`].
pub(crate) type HistoricalFallbackObject = Object;

/// Alias for [`CertifiedCheckpointSummary`] with its [`CheckpointContents`]
/// data fetched from historical fallback storage.
///
/// Contains all data needed to reconstruct a [`StoredCheckpoint`].
pub(crate) type HistoricalFallbackCheckpoint = (CertifiedCheckpointSummary, CheckpointContents);

impl From<HistoricalFallbackObject> for StoredObject {
    fn from(object: HistoricalFallbackObject) -> Self {
        let df_kind = extract_df_kind(&object);
        let indexed = IndexedObject::from_object(None, object, df_kind);
        StoredObject::from(indexed)
    }
}

impl From<HistoricalFallbackCheckpoint> for StoredCheckpoint {
    fn from(checkpoint: HistoricalFallbackCheckpoint) -> Self {
        let (checkpoint_summary, checkpoint_contents) = checkpoint;
        // StoredCheckpoint::from implementation does not use the `successful_tx_num`
        // param in IndexedCheckpoint::from_iota_checkpoint, in this regard it is safe
        // to hardcode to 0.
        let indexed =
            IndexedCheckpoint::from_iota_checkpoint(&checkpoint_summary, &checkpoint_contents, 0);
        StoredCheckpoint::from(&indexed)
    }
}

/// Wrapper for [`TransactionEvents`] and additional data fetched from
/// historical fallback storage.
///
/// Contains all data needed to reconstruct [`IotaEvent`]s or [`StoredEvent`]s.
#[derive(Debug, Clone)]
pub struct HistoricalFallbackEvents {
    /// Events emitted during transaction execution.
    events: TransactionEvents,
    /// Digest of the transaction that emitted the events.
    tx_digest: TransactionDigest,
    /// Sequence number of the transaction that emitted the events.
    tx_sequence_number: u64,
    /// Sequence number of the checkpoint the transaction is part of.
    checkpoint_sequence_number: CheckpointSequenceNumber,
    /// Checkpoint timestamp.
    timestamp: u64,
}

impl HistoricalFallbackEvents {
    /// Creates the wrapper from the events of the `tx_digest` transaction and
    /// the checkpoint the transaction is part of.
    ///
    /// # Errors
    ///
    /// Returns [`IndexerError::HistoricalFallbackStorageError`] when
    /// transaction is not part of the provided checkpoint.
    pub fn new(
        events: TransactionEvents,
        tx_digest: TransactionDigest,
        checkpoint_summary: &CertifiedCheckpointSummary,
        checkpoint_contents: &CheckpointContents,
    ) -> IndexerResult<Self> {
        let Some(tx_sequence_number) = checkpoint_contents
            .enumerate_transactions(checkpoint_summary)
            .find(|(_, execution_digest)| execution_digest.transaction == tx_digest)
            .map(|(seq, _)| seq)
        else {
            return Err(IndexerError::HistoricalFallbackStorageError(format!(
                "cannot find transaction sequence number to transaction: {tx_digest}"
            )));
        };

        Ok(Self {
            events,
            tx_digest,
            tx_sequence_number,
            checkpoint_sequence_number: checkpoint_summary.sequence_number,
            timestamp: checkpoint_summary.timestamp_ms,
        })
    }

    /// Converts the raw [`TransactionEvents`] into JSON RPC compatible
    /// [`IotaEvent`]s.
    pub(crate) async fn into_iota_events(
        self,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
    ) -> IndexerResult<Vec<IotaEvent>> {
        tx_events_to_iota_tx_events(
            self.events,
            package_resolver,
            self.tx_digest,
            Some(self.timestamp),
        )
        .await
        .map(|tx_block_event| tx_block_event.data)
    }

    /// Converts the raw [`TransactionEvents`] into [`StoredEvent`]s.
    pub(crate) fn into_stored_events(self) -> Vec<StoredEvent> {
        self.events
            .iter()
            .enumerate()
            .map(|(idx, event)| {
                StoredEvent::from(IndexedEvent::from_event(
                    self.tx_sequence_number,
                    idx as u64,
                    self.checkpoint_sequence_number,
                    self.tx_digest,
                    event,
                    self.timestamp,
                ))
            })
            .collect()
    }
}

/// Wrapper for a complete transaction fetched from historical fallback storage.
///
/// Contains all data needed to reconstruct a [`StoredTransaction`].
#[derive(Debug, Clone)]
pub struct HistoricalFallbackTransaction {
    /// Checkpointed transaction data.
    checkpoint_transaction: CheckpointTransaction,
    /// Checkpoint sequence number the transaction is part of.
    historical_checkpoint: HistoricalFallbackCheckpoint,
}

impl HistoricalFallbackTransaction {
    pub fn new(
        checkpoint_transaction: CheckpointTransaction,
        historical_checkpoint: HistoricalFallbackCheckpoint,
    ) -> Self {
        Self {
            checkpoint_transaction,
            historical_checkpoint,
        }
    }

    /// Converts the historical fallback transaction into a
    /// [`StoredTransaction`].
    pub(crate) async fn into_stored_transaction(self) -> IndexerResult<StoredTransaction> {
        let tx_digest = self.checkpoint_transaction.transaction.digest();
        let (summary, contents) = self.historical_checkpoint;

        let Some(tx_sequence_number) = contents
            .enumerate_transactions(&summary)
            .find(|(_seq, execution_digest)| &execution_digest.transaction == tx_digest)
            .map(|(seq, _execution_digest)| seq)
        else {
            return Err(IndexerError::HistoricalFallbackStorageError(format!(
                "cannot find transaction sequence number to transaction: {tx_digest}"
            )));
        };

        let indexed_tx = PrimaryWorker::index_transaction(
            &self.checkpoint_transaction,
            tx_sequence_number,
            summary.sequence_number,
            summary.timestamp_ms,
            &IndexerMetrics::new(&Registry::new()),
        )
        .await?;

        Ok(StoredTransaction::from(&indexed_tx))
    }
}
