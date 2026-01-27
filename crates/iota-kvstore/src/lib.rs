// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use async_trait::async_trait;
use iota_types::{
    digests::{CheckpointDigest, TransactionDigest},
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::Object,
    storage::ObjectKey,
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};

/// BigTable Key Value store implementation.
mod bigtable;

pub use bigtable::{client, worker::KvWorker};
pub use iota_bigtable::{BigTableClient, Cell, Row, proto};

/// Read key-value data from a persistent store, such as objects, transactions,
/// and checkpoints.
#[async_trait]
pub trait KeyValueStoreReader {
    type Error;

    /// Fetches a list of objects by their keys.
    ///
    /// Not found objects are omitted from the output list.
    async fn get_objects(&mut self, objects: &[ObjectKey]) -> Result<Vec<Object>, Self::Error>;

    /// Fetches a list of transactions by their digests.
    ///
    /// Not found transactions are omitted from the output list.
    async fn get_transactions(
        &mut self,
        transactions: &[TransactionDigest],
    ) -> Result<Vec<TransactionData>, Self::Error>;

    /// Fetches a list of checkpoints by their sequence numbers.
    ///
    /// Not found checkpoints are omitted from the output list.
    async fn get_checkpoints(
        &mut self,
        sequence_numbers: &[CheckpointSequenceNumber],
    ) -> Result<Vec<Checkpoint>, Self::Error>;

    /// Fetches a list of checkpoints by their digests.
    ///
    /// Not found checkpoints are omitted from the output list.
    async fn get_checkpoints_by_digest(
        &mut self,
        digests: &[CheckpointDigest],
    ) -> Result<Vec<Checkpoint>, Self::Error>;
}

/// Writing key-value data to a persistent store, such as objects, transactions,
/// and checkpoints.
#[async_trait]
pub trait KeyValueStoreWriter {
    type Error;

    /// Persists a list of objects to the store.
    async fn save_objects(&mut self, objects: &[&Object]) -> Result<(), Self::Error>;

    /// Persists a list of transactions to the store.
    async fn save_transactions(
        &mut self,
        transactions: &[TransactionData],
    ) -> Result<(), Self::Error>;

    /// Persists a checkpoint to the store.
    async fn save_checkpoint(&mut self, checkpoint: &CheckpointData) -> Result<(), Self::Error>;
}

/// Represents all stored Key-Value data associated to a checkpoint containing
/// both the summary and the full contents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub summary: CertifiedCheckpointSummary,
    pub contents: CheckpointContents,
}

/// Represents all stored Key-Value data associated with a transaction,
/// including its effects, events, and the checkpoint number it belongs to.
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
