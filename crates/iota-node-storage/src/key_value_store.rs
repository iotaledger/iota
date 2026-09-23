// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_sdk_types::{
    CheckpointContents, CheckpointDigest, ObjectId, TransactionDigest, TransactionEffects,
    TransactionEvents, Version,
};
use iota_types::{
    error::IotaResult,
    messages_checkpoint::{CertifiedCheckpointSummary, CheckpointSequenceNumber},
    object::Object,
    storage::ObjectKey,
    transaction::TransactionEnvelope,
};

pub type KVStoreTransactionData = (
    Vec<Option<TransactionEnvelope>>,
    Vec<Option<TransactionEffects>>,
);

pub type KVStoreCheckpointData = (
    Vec<Option<CertifiedCheckpointSummary>>,
    Vec<Option<CheckpointContents>>,
    Vec<Option<CertifiedCheckpointSummary>>,
);

/// Immutable key/value store trait for reading transactions, effects, events,
/// checkpoints and objects, mostly with batched `multi_get*` methods.
#[async_trait]
pub trait TransactionKeyValueStoreTrait {
    /// Generic multi_get, allows implementors to get heterogenous values with a
    /// single round trip.
    async fn multi_get(
        &self,
        transaction_keys: &[TransactionDigest],
        effects_keys: &[TransactionDigest],
    ) -> IotaResult<KVStoreTransactionData>;

    /// Generic multi_get to allow implementors to get heterogenous values with
    /// a single round trip.
    async fn multi_get_checkpoints(
        &self,
        checkpoint_summaries: &[CheckpointSequenceNumber],
        checkpoint_contents: &[CheckpointSequenceNumber],
        checkpoint_summaries_by_digest: &[CheckpointDigest],
    ) -> IotaResult<KVStoreCheckpointData>;

    async fn get_transaction_perpetual_checkpoint(
        &self,
        digest: TransactionDigest,
    ) -> IotaResult<Option<CheckpointSequenceNumber>>;

    async fn get_object(&self, object_id: ObjectId, version: Version)
    -> IotaResult<Option<Object>>;

    async fn multi_get_objects(&self, object_keys: &[ObjectKey])
    -> IotaResult<Vec<Option<Object>>>;

    async fn multi_get_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<CheckpointSequenceNumber>>>;

    async fn multi_get_events_by_tx_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>>;
}
