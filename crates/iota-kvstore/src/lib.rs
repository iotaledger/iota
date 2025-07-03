// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod bigtable;
use anyhow::Result;
use async_trait::async_trait;
pub use bigtable::{
    client::BigTableClient, progress_store::BigTableProgressStore, worker::KvWorker,
};
use iota_types::{
    base_types::ObjectID,
    digests::{CheckpointDigest, TransactionDigest},
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber, CheckpointSummary,
    },
    object::Object,
    storage::ObjectKey,
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};

/// Read key-value data from a persistent store, such as objects, transactions,
/// and checkpoints.
#[async_trait]
pub trait KeyValueStoreReader {
    /// Fetches a list of objects by their keys.
    async fn get_objects(&mut self, objects: &[ObjectKey]) -> Result<Vec<Object>>;

    /// Fetches a list of transactions by their digests.
    async fn get_transactions(
        &mut self,
        transactions: &[TransactionDigest],
    ) -> Result<Vec<TransactionData>>;

    /// Fetches a list of checkpoints by their sequence numbers.
    async fn get_checkpoints(
        &mut self,
        sequence_numbers: &[CheckpointSequenceNumber],
    ) -> Result<Vec<Checkpoint>>;

    /// Fetches a checkpoint by its digest.
    async fn get_checkpoint_by_digest(
        &mut self,
        digest: CheckpointDigest,
    ) -> Result<Option<Checkpoint>>;

    /// Fetches the sequence number of the latest checkpoint.
    async fn get_latest_checkpoint(&mut self) -> Result<CheckpointSequenceNumber>;

    /// Fetches the summary of the latest checkpoint, if available.
    async fn get_latest_checkpoint_summary(&mut self) -> Result<Option<CheckpointSummary>>;

    /// Fetches the latest version of an object by its ID.
    async fn get_latest_object(&mut self, object_id: &ObjectID) -> Result<Option<Object>>;
}

/// Writing key-value data to a persistent store, such as objects, transactions,
/// and checkpoints.
#[async_trait]
pub trait KeyValueStoreWriter {
    /// Persists a list of objects to the store.
    async fn save_objects(&mut self, objects: &[&Object]) -> Result<()>;

    /// Persists a list of transactions to the store.
    async fn save_transactions(&mut self, transactions: &[TransactionData]) -> Result<()>;

    /// Persists a checkpoint to the store.
    async fn save_checkpoint(&mut self, checkpoint: &CheckpointData) -> Result<()>;

    /// Persists the watermark to the store.
    async fn save_watermark(&mut self, watermark: CheckpointSequenceNumber) -> Result<()>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub summary: CertifiedCheckpointSummary,
    pub contents: CheckpointContents,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionData {
    pub transaction: Transaction,
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    pub checkpoint_number: CheckpointSequenceNumber,
}

impl TransactionData {
    pub fn new(
        checkpoint_transaction: &CheckpointTransaction,
        checkpoint_sequence_number: CheckpointSequenceNumber,
    ) -> Self {
        Self {
            transaction: checkpoint_transaction.transaction.clone(),
            effects: checkpoint_transaction.effects.clone(),
            events: checkpoint_transaction.events.clone(),
            checkpoint_number: checkpoint_sequence_number,
        }
    }
}
