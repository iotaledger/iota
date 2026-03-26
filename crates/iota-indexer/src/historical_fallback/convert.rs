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
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointContents},
    object::Object,
};
use prometheus::Registry;

use crate::{
    errors::{IndexerError, IndexerResult},
    ingestion::{common::prepare::extract_df_kind, primary::prepare::PrimaryWorker},
    metrics::IndexerMetrics,
    models::{
        checkpoints::StoredCheckpoint,
        objects::StoredObject,
        transactions::{StoredTransaction, tx_events_to_iota_tx_events},
    },
    types::{IndexedCheckpoint, IndexedObject},
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
        // StoredObject::from implementation does not require a checkpoint sequence
        // number, in this regard it is safe to hardcode the checkpoint sequence number
        // to 0.
        let indexed = IndexedObject::from_object(0, object, df_kind);
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
/// Contains all data needed to reconstruct [`IotaEvent`]s.
#[derive(Debug, Clone)]
pub struct HistoricalFallbackEvents {
    /// Events emitted during transaction execution.
    events: TransactionEvents,
    /// Checkpoint timestamp.
    timestamp: u64,
}

impl HistoricalFallbackEvents {
    pub fn new(events: TransactionEvents, checkpoint_summary: CertifiedCheckpointSummary) -> Self {
        Self {
            events,
            timestamp: checkpoint_summary.timestamp_ms,
        }
    }

    /// Converts the raw [`TransactionEvents`] into JSON RPC compatible
    /// [`IotaEvent`]s.
    pub(crate) async fn into_iota_events(
        self,
        package_resolver: &Arc<Resolver<impl PackageStore>>,
        tx_digest: TransactionDigest,
    ) -> IndexerResult<Vec<IotaEvent>> {
        tx_events_to_iota_tx_events(
            self.events,
            package_resolver,
            tx_digest,
            Some(self.timestamp),
        )
        .await
        .map(|tx_block_event| tx_block_event.data)
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
