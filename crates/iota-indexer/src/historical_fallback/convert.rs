// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Provides helper functions and types for converting historical fallback
//! types into Indexer-compatible types for the JSON-RPC API.

use std::sync::Arc;

use iota_json_rpc_types::{IotaEvent, IotaTransactionKind};
use iota_package_resolver::{PackageStore, Resolver};
use iota_types::{
    digests::TransactionDigest,
    dynamic_field::DynamicFieldType,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::Object,
    transaction::{Transaction, TransactionDataAPI},
};
use prometheus::Registry;

use crate::{
    errors::IndexerResult,
    ingestion::{common::prepare::extract_df_kind, primary::prepare::InMemTxChanges},
    metrics::IndexerMetrics,
    models::{
        checkpoints::StoredCheckpoint,
        objects::StoredObject,
        transactions::{StoredTransaction, tx_events_to_iota_tx_events},
    },
    types::{IndexedCheckpoint, IndexedObject, IndexedTransaction},
};

/// Represents the data needed to fetch from historical fallback storage in
/// order to convert it into compatible types the indexer can use for JSON RPC
/// API.
#[derive(Debug, Clone)]
pub struct HistoricalFallbackObject(Object);

impl HistoricalFallbackObject {
    pub fn new(object: Object) -> Self {
        Self(object)
    }

    /// Returns the inner [`Object`].
    pub fn into_inner(self) -> Object {
        self.0
    }

    /// Inspect the inner [`Object`] and determine whether it represents a
    /// Dynamic Field or a Dynamic Object Field based on its type.
    fn df_kind(&self) -> Option<DynamicFieldType> {
        extract_df_kind(&self.0)
    }
}

impl From<HistoricalFallbackObject> for StoredObject {
    fn from(object: HistoricalFallbackObject) -> Self {
        let df_kind = object.df_kind();
        // StoredObject::from implementation does not require a checkpoint sequence
        // number, in this regard it is safe to hardcode the checkpoint sequence number
        // to 0.
        let indexed = IndexedObject::from_object(0, object.into_inner(), df_kind);
        StoredObject::from(indexed)
    }
}

/// Represents the data needed to fetch from historical fallback storage in
/// order to convert it into compatible types the indexer can use for JSON RPC
/// API.
#[derive(Debug, Clone)]
pub struct HistoricalFallbackCheckpoint((CertifiedCheckpointSummary, CheckpointContents));

impl HistoricalFallbackCheckpoint {
    pub fn new(
        checkpoint_summary: CertifiedCheckpointSummary,
        checkpoint_contents: CheckpointContents,
    ) -> Self {
        Self((checkpoint_summary, checkpoint_contents))
    }

    /// Returns the inner [`CertifiedCheckpointSummary`] and
    /// [`CheckpointContents`].
    pub fn into_inner(self) -> (CertifiedCheckpointSummary, CheckpointContents) {
        self.0
    }
}

impl From<HistoricalFallbackCheckpoint> for StoredCheckpoint {
    fn from(checkpoint: HistoricalFallbackCheckpoint) -> Self {
        let (checkpoint_summary, checkpoint_contents) = checkpoint.into_inner();
        // StoredCheckpoint::from implementation does not use the `successful_tx_num`
        // param in IndexedCheckpoint::from_iota_checkpoint, in this regard it is safe
        // to hardcode to 0.
        let indexed =
            IndexedCheckpoint::from_iota_checkpoint(&checkpoint_summary, &checkpoint_contents, 0);
        StoredCheckpoint::from(&indexed)
    }
}

/// Represents the data needed to fetch from historical fallback storage in
/// order to convert it into compatible types the indexer can use for JSON RPC
/// API.
#[derive(Debug, Clone)]
pub struct HistoricalFallbackEvents {
    events: TransactionEvents,
    /// Checkpoint timestamp.
    timestamp: u64,
}

impl HistoricalFallbackEvents {
    #[expect(dead_code)]
    pub fn new(events: TransactionEvents, checkpoint_summary: CertifiedCheckpointSummary) -> Self {
        Self {
            events,
            timestamp: checkpoint_summary.timestamp_ms,
        }
    }

    /// Converts the raw [`Event`]s into JSON RPC compatible [`IotaEvent`]s.
    #[expect(dead_code)]
    pub(crate) async fn into_iota_events(
        self,
        package_resolver: Arc<Resolver<impl PackageStore>>,
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

/// Represents the data needed to fetch from historical fallback storage in
/// order to convert it into compatible types the indexer can
/// use for JSON RPC API.
#[derive(Debug, Clone)]
pub struct HistoricalFallbackTransaction {
    transaction: Transaction,
    effects: TransactionEffects,
    events: Option<TransactionEvents>,
    /// Objects state before the transaction was executed.
    input_objects: Vec<Object>,
    /// Objects that were mutated, created or unwrapped by this transaction
    /// after its execution.
    output_objects: Vec<Object>,
    /// Checkpoint sequence number the transaction is part of.
    checkpoint_sequence_number: CheckpointSequenceNumber,
    /// Checkpoint timestamp.
    timestamp: u64,
}

impl HistoricalFallbackTransaction {
    #[expect(dead_code)]
    pub fn new(
        transaction: Transaction,
        effects: TransactionEffects,
        events: impl Into<Option<TransactionEvents>>,
        input_objects: Vec<Object>,
        output_objects: Vec<Object>,
        checkpoint_summary: CertifiedCheckpointSummary,
    ) -> Self {
        Self {
            transaction,
            effects,
            events: events.into(),
            input_objects,
            output_objects,
            checkpoint_sequence_number: checkpoint_summary.sequence_number,
            timestamp: checkpoint_summary.timestamp_ms,
        }
    }

    /// Converts the historical fallback transaction into a
    /// [`StoredTransaction`].
    #[expect(dead_code)]
    async fn into_stored_transaction(self) -> IndexerResult<StoredTransaction> {
        let tx_digest = self.transaction.digest();
        let tx_data = self.transaction.transaction_data();

        let events = self
            .events
            .as_ref()
            .map(|events| events.data.clone())
            .unwrap_or_default();

        let transaction_kind = IotaTransactionKind::from(tx_data.kind());

        let objects = self
            .input_objects
            .iter()
            .chain(self.output_objects.iter())
            .collect::<Vec<_>>();

        let (balance_change, object_changes) =
            InMemTxChanges::new(&objects, IndexerMetrics::new(&Registry::new()))
                .get_changes(tx_data, &self.effects, tx_digest)
                .await?;

        let indexed_tx = IndexedTransaction {
            // StoredTransaction::from implementation does not use the `tx_sequence_number`, in this
            // regard it is safe to hardcode to 0.
            tx_sequence_number: 0,
            tx_digest: *tx_digest,
            checkpoint_sequence_number: self.checkpoint_sequence_number,
            timestamp_ms: self.timestamp,
            sender_signed_data: self.transaction.data().clone(),
            successful_tx_num: if self.effects.status().is_ok() {
                tx_data.kind().tx_count() as u64
            } else {
                0
            },
            effects: self.effects,
            object_changes,
            balance_change,
            events,
            transaction_kind,
        };

        Ok(StoredTransaction::from(&indexed_tx))
    }
}
