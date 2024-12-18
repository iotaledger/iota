// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
mod bigtable;
use anyhow::Result;
use async_trait::async_trait;
pub use bigtable::client::BigTableClient;
pub use bigtable::worker::KvWorker;
use iota_types::base_types::ObjectID;
use iota_types::crypto::AuthorityStrongQuorumSignInfo;
use iota_types::digests::{CheckpointDigest, TransactionDigest};
use iota_types::effects::{TransactionEffects, TransactionEvents};
use iota_types::full_checkpoint_content::CheckpointData;
use iota_types::messages_checkpoint::{
    CheckpointContents, CheckpointSequenceNumber, CheckpointSummary,
};
use iota_types::object::Object;
use iota_types::storage::ObjectKey;
use iota_types::transaction::Transaction;

#[async_trait]
pub trait KeyValueStoreReader {
    async fn get_objects(&mut self, objects: &[ObjectKey]) -> Result<Vec<Object>>;
    async fn get_transactions(
        &mut self,
        transactions: &[TransactionDigest],
    ) -> Result<Vec<TransactionData>>;
    async fn get_checkpoints(
        &mut self,
        sequence_numbers: &[CheckpointSequenceNumber],
    ) -> Result<Vec<Checkpoint>>;
    async fn get_checkpoint_by_digest(
        &mut self,
        digest: CheckpointDigest,
    ) -> Result<Option<Checkpoint>>;
    async fn get_latest_checkpoint(&mut self) -> Result<CheckpointSequenceNumber>;
    async fn get_latest_object(&mut self, object_id: &ObjectID) -> Result<Option<Object>>;
}

#[async_trait]
pub trait KeyValueStoreWriter {
    async fn save_objects(&mut self, objects: &[&Object]) -> Result<()>;
    async fn save_transactions(&mut self, transactions: &[TransactionData]) -> Result<()>;
    async fn save_checkpoint(&mut self, checkpoint: &CheckpointData) -> Result<()>;
}

#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub summary: CheckpointSummary,
    pub contents: CheckpointContents,
    pub signatures: AuthorityStrongQuorumSignInfo,
}

#[derive(Clone, Debug)]
pub struct TransactionData {
    pub transaction: Transaction,
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
    pub checkpoint_number: CheckpointSequenceNumber,
    pub timestamp: u64,
}
