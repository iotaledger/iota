// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::num::NonZeroUsize;

use anyhow::Result;
use async_trait::async_trait;
use iota_types::{
    base_types::IotaAddress,
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

pub use bigtable::{
    client,
    worker::{KvWorker, Table},
};
pub use iota_bigtable::{BigTableClient, Cell, Row, proto};

use crate::bigtable::client::{TransactionSequenceNumber, TransactionsOrder};

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

    /// Fetches a list `(sequence number, digest)` pairs for transactions
    /// affecting the given address, ordered by [`TransactionsOrder`] and capped
    /// at `limit`.
    ///
    /// # Pagination
    /// `cursor` is **exclusive**.
    ///
    /// - [`TransactionsOrder::NewestFirst`]: returns entries with `tx_seq <
    ///   cursor`.
    /// - [`TransactionsOrder::OldestFirst`]: returns entries with `tx_seq >
    ///   cursor`.
    ///
    /// When `None`, the scan starts from the beginning of the requested
    /// [`TransactionsOrder`].
    async fn get_transaction_digests_by_address(
        &mut self,
        address: IotaAddress,
        cursor: impl Into<Option<TransactionSequenceNumber>> + Send,
        limit: impl TryInto<NonZeroUsize> + Send,
        order: TransactionsOrder,
    ) -> Result<Vec<(TransactionSequenceNumber, TransactionDigest)>, Self::Error>;

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
    async fn get_checkpoints_by_digest<I>(
        &mut self,
        digests: I,
    ) -> Result<Vec<Checkpoint>, Self::Error>
    where
        I: IntoIterator<Item = CheckpointDigest> + Send,
        I::IntoIter: Send;

    /// Fetches a list of checkpoint sequence numbers by their digests.
    ///
    /// Not found checkpoints are omitted from the output list.
    async fn get_checkpoint_sequence_numbers<I>(
        &mut self,
        digests: I,
    ) -> Result<Vec<CheckpointSequenceNumber>, Self::Error>
    where
        I: IntoIterator<Item = CheckpointDigest> + Send,
        I::IntoIter: Send;
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

    /// Persists a mapping of `(` [`IotaAddress`], `transaction_sequence_number`
    /// `)` to `TransactionDigest` for every affected address.
    ///
    /// An address is considered "affected" if it appears as the sender, a
    /// recipient, or the gas payer.
    async fn save_transactions_by_address<I>(&mut self, entries: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = (IotaAddress, u64, TransactionDigest)> + Send,
        I::IntoIter: Send;

    /// Persists a checkpoint to the store.
    async fn save_checkpoint(&mut self, checkpoint: &CheckpointData) -> Result<(), Self::Error>;

    /// Persists a checkpoint digest to its corresponding sequence number to the
    /// store.
    async fn save_checkpoint_by_digest(
        &mut self,
        checkpoint: &CheckpointData,
    ) -> Result<(), Self::Error>;
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
