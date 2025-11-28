// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::{CheckpointDigest, TransactionDigest},
    effects::{TransactionEffects, TransactionEvents},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::Object,
    transaction::Transaction,
};

use crate::errors::IndexerResult;

#[expect(dead_code)]
pub(crate) trait KeyValueStoreClient {
    async fn multi_get_transactions(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<Transaction>>>;

    async fn multi_get_effects(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<TransactionEffects>>>;

    async fn multi_get_transactions_perpetual_checkpoints(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<CheckpointSequenceNumber>>>;

    async fn multi_get_events_by_tx_digests(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<TransactionEvents>>>;

    async fn multi_get_checkpoints_summaries_by_sequence_numbers(
        &self,
        checkpoint_sequence_numbers: &[CheckpointSequenceNumber],
    ) -> IndexerResult<Vec<Option<CertifiedCheckpointSummary>>>;

    async fn multi_get_checkpoints_contents(
        &self,
        checkpoint_sequence_numbers: &[CheckpointSequenceNumber],
    ) -> IndexerResult<Vec<Option<CheckpointContents>>>;

    async fn multi_get_checkpoints_summaries_by_digests(
        &self,
        checkpoint_digests: &[CheckpointDigest],
    ) -> IndexerResult<Vec<Option<CertifiedCheckpointSummary>>>;

    async fn multi_get_objects(
        &self,
        object_refs: &[(ObjectID, SequenceNumber)],
    ) -> IndexerResult<Vec<Option<Object>>>;
}
