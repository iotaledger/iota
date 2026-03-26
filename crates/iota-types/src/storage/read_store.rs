// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use move_core_types::{
    annotated_value::MoveTypeLayout,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};
use typed_store_error::TypedStoreError;

use super::{ObjectStore, error::Result};
use crate::{
    base_types::{EpochId, IotaAddress, MoveObjectType, ObjectID, ObjectType, SequenceNumber},
    committee::Committee,
    digests::{ChainIdentifier, CheckpointContentsDigest, CheckpointDigest, TransactionDigest},
    dynamic_field::DynamicFieldType,
    effects::{TransactionEffects, TransactionEvents},
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::{
        CheckpointContents, CheckpointSequenceNumber, FullCheckpointContents, VerifiedCheckpoint,
    },
    object::Object,
    storage::{get_transaction_input_objects, get_transaction_output_objects},
    transaction::VerifiedTransaction,
};

/// Represents a transaction combined with its effects and events for efficient
/// batch processing
#[derive(Clone, Debug)]
pub struct TransactionWithEffectsAndEvents {
    pub transaction: Arc<VerifiedTransaction>,
    pub effects: TransactionEffects,
    pub events: Option<TransactionEvents>,
}

pub trait ReadStore: ObjectStore {
    // Committee Getters
    //

    fn try_get_committee(&self, epoch: EpochId) -> Result<Option<Arc<Committee>>>;

    /// Non-fallible version of `try_get_committee`.
    fn get_committee(&self, epoch: EpochId) -> Option<Arc<Committee>> {
        self.try_get_committee(epoch)
            .expect("storage access failed")
    }

    // Checkpoint Getters
    //

    /// Get the latest available checkpoint. This is the latest executed
    /// checkpoint.
    ///
    /// All transactions, effects, objects and events are guaranteed to be
    /// available for the returned checkpoint.
    fn try_get_latest_checkpoint(&self) -> Result<VerifiedCheckpoint>;

    /// Non-fallible version of `try_get_latest_checkpoint`.
    fn get_latest_checkpoint(&self) -> VerifiedCheckpoint {
        self.try_get_latest_checkpoint()
            .expect("storage access failed")
    }

    /// Get the latest available checkpoint sequence number. This is the
    /// sequence number of the latest executed checkpoint.
    fn try_get_latest_checkpoint_sequence_number(&self) -> Result<CheckpointSequenceNumber> {
        let latest_checkpoint = self.try_get_latest_checkpoint()?;
        Ok(*latest_checkpoint.sequence_number())
    }

    /// Non-fallible version of `try_get_latest_checkpoint_sequence_number`.
    fn get_latest_checkpoint_sequence_number(&self) -> CheckpointSequenceNumber {
        self.try_get_latest_checkpoint_sequence_number()
            .expect("storage access failed")
    }

    /// Get the epoch of the latest checkpoint
    fn try_get_latest_epoch_id(&self) -> Result<EpochId> {
        let latest_checkpoint = self.try_get_latest_checkpoint()?;
        Ok(latest_checkpoint.epoch())
    }

    /// Non-fallible version of `try_get_latest_epoch_id`.
    fn get_latest_epoch_id(&self) -> EpochId {
        self.try_get_latest_epoch_id()
            .expect("storage access failed")
    }

    /// Get the highest verified checkpoint. This is the highest checkpoint
    /// summary that has been verified, generally by state-sync. Only the
    /// checkpoint header is guaranteed to be present in the store.
    fn try_get_highest_verified_checkpoint(&self) -> Result<VerifiedCheckpoint>;

    /// Non-fallible version of `try_get_highest_verified_checkpoint`.
    fn get_highest_verified_checkpoint(&self) -> VerifiedCheckpoint {
        self.try_get_highest_verified_checkpoint()
            .expect("storage access failed")
    }

    /// Get the highest synced checkpoint. This is the highest checkpoint that
    /// has been synced from state-synce. The checkpoint header, contents,
    /// transactions, and effects of this checkpoint are guaranteed to be
    /// present in the store
    fn try_get_highest_synced_checkpoint(&self) -> Result<VerifiedCheckpoint>;

    /// Non-fallible version of `try_get_highest_synced_checkpoint`.
    fn get_highest_synced_checkpoint(&self) -> VerifiedCheckpoint {
        self.try_get_highest_synced_checkpoint()
            .expect("storage access failed")
    }

    /// Lowest available checkpoint for which transaction and checkpoint data
    /// can be requested.
    ///
    /// Specifically this is the lowest checkpoint for which the following data
    /// can be requested:
    ///  - checkpoints
    ///  - transactions
    ///  - effects
    ///  - events
    ///
    /// For object availability see `get_lowest_available_checkpoint_objects`.
    fn try_get_lowest_available_checkpoint(&self) -> Result<CheckpointSequenceNumber>;

    /// Non-fallible version of `try_get_lowest_available_checkpoint`.
    fn get_lowest_available_checkpoint(&self) -> CheckpointSequenceNumber {
        self.try_get_lowest_available_checkpoint()
            .expect("storage access failed")
    }

    fn try_get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> Result<Option<VerifiedCheckpoint>>;

    /// Non-fallible version of `try_get_checkpoint_by_digest`.
    fn get_checkpoint_by_digest(&self, digest: &CheckpointDigest) -> Option<VerifiedCheckpoint> {
        self.try_get_checkpoint_by_digest(digest)
            .expect("storage access failed")
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<VerifiedCheckpoint>>;

    /// Non-fallible version of `try_get_checkpoint_by_sequence_number`.
    fn get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Option<VerifiedCheckpoint> {
        self.try_get_checkpoint_by_sequence_number(sequence_number)
            .expect("storage access failed")
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<CheckpointContents>>;

    /// Non-fallible version of `try_get_checkpoint_contents_by_digest`.
    fn get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Option<CheckpointContents> {
        self.try_get_checkpoint_contents_by_digest(digest)
            .expect("storage access failed")
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<CheckpointContents>>;

    /// Non-fallible version of
    /// `try_get_checkpoint_contents_by_sequence_number`.
    fn get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Option<CheckpointContents> {
        self.try_get_checkpoint_contents_by_sequence_number(sequence_number)
            .expect("storage access failed")
    }

    // Transaction Getters
    //

    fn try_get_transaction(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<Arc<VerifiedTransaction>>>;

    /// Non-fallible version of `try_get_transaction`.
    fn get_transaction(&self, tx_digest: &TransactionDigest) -> Option<Arc<VerifiedTransaction>> {
        self.try_get_transaction(tx_digest)
            .expect("storage access failed")
    }

    fn try_multi_get_transactions(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<Arc<VerifiedTransaction>>>> {
        tx_digests
            .iter()
            .map(|digest| self.try_get_transaction(digest))
            .collect::<Result<Vec<_>, _>>()
    }

    /// Non-fallible version of `try_multi_get_transactions`.
    fn multi_get_transactions(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Vec<Option<Arc<VerifiedTransaction>>> {
        self.try_multi_get_transactions(tx_digests)
            .expect("storage access failed")
    }

    fn try_get_transaction_effects(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<TransactionEffects>>;

    /// Non-fallible version of `try_get_transaction_effects`.
    fn get_transaction_effects(&self, tx_digest: &TransactionDigest) -> Option<TransactionEffects> {
        self.try_get_transaction_effects(tx_digest)
            .expect("storage access failed")
    }

    fn try_multi_get_transaction_effects(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEffects>>> {
        tx_digests
            .iter()
            .map(|digest| self.try_get_transaction_effects(digest))
            .collect::<Result<Vec<_>, _>>()
    }

    /// Non-fallible version of `try_multi_get_transaction_effects`.
    fn multi_get_transaction_effects(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Vec<Option<TransactionEffects>> {
        self.try_multi_get_transaction_effects(tx_digests)
            .expect("storage access failed")
    }

    fn try_get_events(&self, digest: &TransactionDigest) -> Result<Option<TransactionEvents>>;

    /// Non-fallible version of `try_get_events`.
    fn get_events(&self, digest: &TransactionDigest) -> Option<TransactionEvents> {
        self.try_get_events(digest).expect("storage access failed")
    }

    fn try_multi_get_events(
        &self,
        digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEvents>>> {
        digests
            .iter()
            .map(|digest| self.try_get_events(digest))
            .collect::<Result<Vec<_>, _>>()
    }

    /// Non-fallible version of `try_multi_get_events`.
    fn multi_get_events(&self, digests: &[TransactionDigest]) -> Vec<Option<TransactionEvents>> {
        self.try_multi_get_events(digests)
            .expect("storage access failed")
    }

    // Extra Checkpoint fetching apis
    //

    /// Get a "full" checkpoint for purposes of state-sync
    /// "full" checkpoints include: header, contents, transactions, effects
    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<FullCheckpointContents>>;

    /// Non-fallible version of
    /// `try_get_full_checkpoint_contents_by_sequence_number`.
    fn get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Option<FullCheckpointContents> {
        self.try_get_full_checkpoint_contents_by_sequence_number(sequence_number)
            .expect("storage access failed")
    }

    /// Get a "full" checkpoint for purposes of state-sync
    /// "full" checkpoints include: header, contents, transactions, effects
    fn try_get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<FullCheckpointContents>>;

    /// Non-fallible version of `try_get_full_checkpoint_contents`.
    fn get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Option<FullCheckpointContents> {
        self.try_get_full_checkpoint_contents(digest)
            .expect("storage access failed")
    }

    fn try_multi_get_transactions_with_events_and_effects(
        &self,
        transaction_digests: Vec<TransactionDigest>,
    ) -> anyhow::Result<Vec<TransactionWithEffectsAndEvents>> {
        use crate::effects::TransactionEffectsAPI;

        // Batch read all transactions
        let transactions = self
            .try_multi_get_transactions(&transaction_digests)?
            .into_iter()
            .map(|maybe_transaction| {
                maybe_transaction.ok_or_else(|| anyhow::anyhow!("missing transaction"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // Batch read all effects
        let effects = self
            .try_multi_get_transaction_effects(&transaction_digests)?
            .into_iter()
            .map(|maybe_effects| maybe_effects.ok_or_else(|| anyhow::anyhow!("missing effects")))
            .collect::<anyhow::Result<Vec<_>>>()?;

        // Extract transaction digests for transactions that have events
        let event_tx_digests = transaction_digests
            .iter()
            .zip(effects.iter())
            .filter_map(|(tx_digest, fx)| fx.events_digest().map(|_| *tx_digest))
            .collect::<Vec<_>>();

        // Batch read all events
        let events = self
            .try_multi_get_events(&event_tx_digests)?
            .into_iter()
            .zip(event_tx_digests)
            .map(|(maybe_event, tx_digest)| {
                maybe_event
                    .ok_or_else(|| anyhow::anyhow!("missing event"))
                    .map(|event| (tx_digest, event))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        // Collect the final result
        let result = transactions
            .into_iter()
            .zip(effects)
            .map(|(transaction, effects)| {
                let events = effects
                    .events_digest()
                    .and_then(|_| events.get(effects.transaction_digest()).cloned());

                TransactionWithEffectsAndEvents {
                    transaction,
                    effects,
                    events,
                }
            })
            .collect();

        Ok(result)
    }

    fn get_checkpoint_transaction(
        &self,
        tx_with_events_and_effects: TransactionWithEffectsAndEvents,
    ) -> anyhow::Result<CheckpointTransaction> {
        let input_objects =
            get_transaction_input_objects(&self, &tx_with_events_and_effects.effects)?;
        let output_objects =
            get_transaction_output_objects(&self, &tx_with_events_and_effects.effects)?;

        let full_transaction = CheckpointTransaction {
            transaction: (*tx_with_events_and_effects.transaction).clone().into(),
            effects: tx_with_events_and_effects.effects,
            events: tx_with_events_and_effects.events,
            input_objects,
            output_objects,
        };

        Ok(full_transaction)
    }

    /// Stream checkpoint transactions individually to avoid large memory
    /// footprint. Returns a stream of individual CheckpointTransaction items
    /// along with metadata
    fn stream_checkpoint_transactions(
        &self,
        checkpoint_contents: CheckpointContents,
    ) -> std::pin::Pin<
        Box<dyn futures::Stream<Item = anyhow::Result<CheckpointTransaction>> + Send + '_>,
    >
    where
        Self: Sync,
    {
        Box::pin(async_stream::stream! {
            let transaction_digests = checkpoint_contents
                .iter()
                .map(|execution_digests| execution_digests.transaction)
                .collect::<Vec<_>>();

            let txs_with_events_and_effects = self
                .try_multi_get_transactions_with_events_and_effects(transaction_digests)?;

            for tx_with_events_and_effects in txs_with_events_and_effects {
                yield self.get_checkpoint_transaction(tx_with_events_and_effects);
            }
        })
    }

    // Fetch all checkpoint data
    // TODO fix return type to not be anyhow
    fn try_get_checkpoint_data(
        &self,
        checkpoint: VerifiedCheckpoint,
        checkpoint_contents: CheckpointContents,
    ) -> anyhow::Result<CheckpointData> {
        let transaction_digests = checkpoint_contents
            .iter()
            .map(|execution_digests| execution_digests.transaction)
            .collect::<Vec<_>>();

        let txs_with_events_and_effects =
            self.try_multi_get_transactions_with_events_and_effects(transaction_digests)?;

        let mut transactions = Vec::with_capacity(txs_with_events_and_effects.len());
        for tx_with_events_and_effects in txs_with_events_and_effects {
            transactions.push(self.get_checkpoint_transaction(tx_with_events_and_effects)?);
        }

        let checkpoint_data = CheckpointData {
            checkpoint_summary: checkpoint.into(),
            checkpoint_contents,
            transactions,
        };

        Ok(checkpoint_data)
    }

    /// Non-fallible version of `try_get_checkpoint_data`.
    fn get_checkpoint_data(
        &self,
        checkpoint: VerifiedCheckpoint,
        checkpoint_contents: CheckpointContents,
    ) -> CheckpointData {
        self.try_get_checkpoint_data(checkpoint, checkpoint_contents)
            .expect("storage access failed")
    }
}

impl<T: ReadStore + ?Sized> ReadStore for &T {
    fn try_get_committee(&self, epoch: EpochId) -> Result<Option<Arc<Committee>>> {
        (*self).try_get_committee(epoch)
    }

    fn try_get_latest_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (*self).try_get_latest_checkpoint()
    }

    fn try_get_latest_checkpoint_sequence_number(&self) -> Result<CheckpointSequenceNumber> {
        (*self).try_get_latest_checkpoint_sequence_number()
    }

    fn try_get_latest_epoch_id(&self) -> Result<EpochId> {
        (*self).try_get_latest_epoch_id()
    }

    fn try_get_highest_verified_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (*self).try_get_highest_verified_checkpoint()
    }

    fn try_get_highest_synced_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (*self).try_get_highest_synced_checkpoint()
    }

    fn try_get_lowest_available_checkpoint(&self) -> Result<CheckpointSequenceNumber> {
        (*self).try_get_lowest_available_checkpoint()
    }

    fn try_get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> Result<Option<VerifiedCheckpoint>> {
        (*self).try_get_checkpoint_by_digest(digest)
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<VerifiedCheckpoint>> {
        (*self).try_get_checkpoint_by_sequence_number(sequence_number)
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<CheckpointContents>> {
        (*self).try_get_checkpoint_contents_by_digest(digest)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<CheckpointContents>> {
        (*self).try_get_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_transaction(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<Arc<VerifiedTransaction>>> {
        (*self).try_get_transaction(tx_digest)
    }

    fn try_multi_get_transactions(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<Arc<VerifiedTransaction>>>> {
        (*self).try_multi_get_transactions(tx_digests)
    }

    fn try_get_transaction_effects(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<TransactionEffects>> {
        (*self).try_get_transaction_effects(tx_digest)
    }

    fn try_multi_get_transaction_effects(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEffects>>> {
        (*self).try_multi_get_transaction_effects(tx_digests)
    }

    fn try_get_events(&self, digest: &TransactionDigest) -> Result<Option<TransactionEvents>> {
        (*self).try_get_events(digest)
    }

    fn try_multi_get_events(
        &self,
        digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEvents>>> {
        (*self).try_multi_get_events(digests)
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<FullCheckpointContents>> {
        (*self).try_get_full_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<FullCheckpointContents>> {
        (*self).try_get_full_checkpoint_contents(digest)
    }

    fn try_get_checkpoint_data(
        &self,
        checkpoint: VerifiedCheckpoint,
        checkpoint_contents: CheckpointContents,
    ) -> anyhow::Result<CheckpointData> {
        (*self).try_get_checkpoint_data(checkpoint, checkpoint_contents)
    }
}

impl<T: ReadStore + ?Sized> ReadStore for Box<T> {
    fn try_get_committee(&self, epoch: EpochId) -> Result<Option<Arc<Committee>>> {
        (**self).try_get_committee(epoch)
    }

    fn try_get_latest_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (**self).try_get_latest_checkpoint()
    }

    fn try_get_latest_checkpoint_sequence_number(&self) -> Result<CheckpointSequenceNumber> {
        (**self).try_get_latest_checkpoint_sequence_number()
    }

    fn try_get_latest_epoch_id(&self) -> Result<EpochId> {
        (**self).try_get_latest_epoch_id()
    }

    fn try_get_highest_verified_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (**self).try_get_highest_verified_checkpoint()
    }

    fn try_get_highest_synced_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (**self).try_get_highest_synced_checkpoint()
    }

    fn try_get_lowest_available_checkpoint(&self) -> Result<CheckpointSequenceNumber> {
        (**self).try_get_lowest_available_checkpoint()
    }

    fn try_get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> Result<Option<VerifiedCheckpoint>> {
        (**self).try_get_checkpoint_by_digest(digest)
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<VerifiedCheckpoint>> {
        (**self).try_get_checkpoint_by_sequence_number(sequence_number)
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<CheckpointContents>> {
        (**self).try_get_checkpoint_contents_by_digest(digest)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<CheckpointContents>> {
        (**self).try_get_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_transaction(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<Arc<VerifiedTransaction>>> {
        (**self).try_get_transaction(tx_digest)
    }

    fn try_multi_get_transactions(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<Arc<VerifiedTransaction>>>> {
        (**self).try_multi_get_transactions(tx_digests)
    }

    fn try_get_transaction_effects(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<TransactionEffects>> {
        (**self).try_get_transaction_effects(tx_digest)
    }

    fn try_multi_get_transaction_effects(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEffects>>> {
        (**self).try_multi_get_transaction_effects(tx_digests)
    }

    fn try_get_events(&self, digest: &TransactionDigest) -> Result<Option<TransactionEvents>> {
        (**self).try_get_events(digest)
    }

    fn try_multi_get_events(
        &self,
        digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEvents>>> {
        (**self).try_multi_get_events(digests)
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<FullCheckpointContents>> {
        (**self).try_get_full_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<FullCheckpointContents>> {
        (**self).try_get_full_checkpoint_contents(digest)
    }

    fn try_get_checkpoint_data(
        &self,
        checkpoint: VerifiedCheckpoint,
        checkpoint_contents: CheckpointContents,
    ) -> anyhow::Result<CheckpointData> {
        (**self).try_get_checkpoint_data(checkpoint, checkpoint_contents)
    }
}

impl<T: ReadStore + ?Sized> ReadStore for Arc<T> {
    fn try_get_committee(&self, epoch: EpochId) -> Result<Option<Arc<Committee>>> {
        (**self).try_get_committee(epoch)
    }

    fn try_get_latest_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (**self).try_get_latest_checkpoint()
    }

    fn try_get_latest_checkpoint_sequence_number(&self) -> Result<CheckpointSequenceNumber> {
        (**self).try_get_latest_checkpoint_sequence_number()
    }

    fn try_get_latest_epoch_id(&self) -> Result<EpochId> {
        (**self).try_get_latest_epoch_id()
    }

    fn try_get_highest_verified_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (**self).try_get_highest_verified_checkpoint()
    }

    fn try_get_highest_synced_checkpoint(&self) -> Result<VerifiedCheckpoint> {
        (**self).try_get_highest_synced_checkpoint()
    }

    fn try_get_lowest_available_checkpoint(&self) -> Result<CheckpointSequenceNumber> {
        (**self).try_get_lowest_available_checkpoint()
    }

    fn try_get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> Result<Option<VerifiedCheckpoint>> {
        (**self).try_get_checkpoint_by_digest(digest)
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<VerifiedCheckpoint>> {
        (**self).try_get_checkpoint_by_sequence_number(sequence_number)
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<CheckpointContents>> {
        (**self).try_get_checkpoint_contents_by_digest(digest)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<CheckpointContents>> {
        (**self).try_get_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_transaction(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<Arc<VerifiedTransaction>>> {
        (**self).try_get_transaction(tx_digest)
    }

    fn try_multi_get_transactions(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<Arc<VerifiedTransaction>>>> {
        (**self).try_multi_get_transactions(tx_digests)
    }

    fn try_get_transaction_effects(
        &self,
        tx_digest: &TransactionDigest,
    ) -> Result<Option<TransactionEffects>> {
        (**self).try_get_transaction_effects(tx_digest)
    }

    fn try_multi_get_transaction_effects(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEffects>>> {
        (**self).try_multi_get_transaction_effects(tx_digests)
    }

    fn try_get_events(&self, digest: &TransactionDigest) -> Result<Option<TransactionEvents>> {
        (**self).try_get_events(digest)
    }

    fn try_multi_get_events(
        &self,
        digests: &[TransactionDigest],
    ) -> Result<Vec<Option<TransactionEvents>>> {
        (**self).try_multi_get_events(digests)
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<FullCheckpointContents>> {
        (**self).try_get_full_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<FullCheckpointContents>> {
        (**self).try_get_full_checkpoint_contents(digest)
    }

    fn try_get_checkpoint_data(
        &self,
        checkpoint: VerifiedCheckpoint,
        checkpoint_contents: CheckpointContents,
    ) -> anyhow::Result<CheckpointData> {
        (**self).try_get_checkpoint_data(checkpoint, checkpoint_contents)
    }
}

/// Trait used to provide functionality to the REST API service.
///
/// It extends both ObjectStore and ReadStore by adding functionality that may
/// require more detailed underlying databases or indexes to support.
pub trait RestStateReader: ObjectStore + ReadStore + Send + Sync {
    /// Lowest available checkpoint for which object data can be requested.
    ///
    /// Specifically this is the lowest checkpoint for which input/output object
    /// data will be available.
    fn get_lowest_available_checkpoint_objects(&self) -> Result<CheckpointSequenceNumber>;

    fn get_chain_identifier(&self) -> Result<ChainIdentifier>;

    fn get_epoch_last_checkpoint(&self, epoch_id: EpochId) -> Result<Option<VerifiedCheckpoint>>;

    // Get a handle to an instance of the RpcIndexes
    fn indexes(&self) -> Option<&dyn RestIndexes>;

    fn get_type_layout(&self, type_tag: &TypeTag) -> Result<Option<MoveTypeLayout>> {
        match type_tag {
            TypeTag::Bool => Ok(Some(MoveTypeLayout::Bool)),
            TypeTag::U8 => Ok(Some(MoveTypeLayout::U8)),
            TypeTag::U64 => Ok(Some(MoveTypeLayout::U64)),
            TypeTag::U128 => Ok(Some(MoveTypeLayout::U128)),
            TypeTag::Address => Ok(Some(MoveTypeLayout::Address)),
            TypeTag::Signer => Ok(Some(MoveTypeLayout::Signer)),
            TypeTag::Vector(type_tag) => Ok(self
                .get_type_layout(type_tag)?
                .map(|layout| MoveTypeLayout::Vector(Box::new(layout)))),
            TypeTag::Struct(struct_tag) => self.get_struct_layout(struct_tag),
            TypeTag::U16 => Ok(Some(MoveTypeLayout::U16)),
            TypeTag::U32 => Ok(Some(MoveTypeLayout::U32)),
            TypeTag::U256 => Ok(Some(MoveTypeLayout::U256)),
        }
    }

    fn get_struct_layout(&self, type_tag: &StructTag) -> Result<Option<MoveTypeLayout>>;
}

pub type DynamicFieldIteratorItem =
    Result<(DynamicFieldKey, DynamicFieldIndexInfo), TypedStoreError>;
pub trait RestIndexes: Send + Sync {
    // only used in "grpc-server"
    fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<EpochInfo>>;

    // used in both "grpc-server" and "rest-api"
    fn get_transaction_info(&self, digest: &TransactionDigest) -> Result<Option<TransactionInfo>>;

    /// Returns an iterator over objects owned by `owner`, optionally filtered
    /// by `object_type`.
    ///
    /// The `cursor` bound is **inclusive**: if `Some(id)` is provided, the
    /// iterator starts *at* that object ID. Callers that wish to paginate past
    /// a previously-seen cursor must `.skip(1)` on the returned iterator to
    /// avoid re-returning the cursor item.
    // only used in "rest-api"
    fn account_owned_objects_info_iter(
        &self,
        owner: IotaAddress,
        cursor: Option<ObjectID>,
        object_type: Option<StructTag>,
    ) -> Result<Box<dyn Iterator<Item = Result<AccountOwnedObjectInfo, TypedStoreError>> + '_>>;

    /// Returns an iterator over objects owned by `owner`, optionally filtered
    /// by `object_type`.
    ///
    /// Each item includes an [`OwnedObjectV2Cursor`] that can be stored in an
    /// opaque page token for seek-based cursor resumption.
    ///
    /// The `cursor` bound is **inclusive**: the iterator starts *at* the cursor
    /// position. Callers must `.skip(1)` to get exclusive semantics.
    // only used in "grpc-server"
    fn account_owned_objects_info_iter_v2(
        &self,
        owner: IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        object_type: Option<StructTag>,
    ) -> Result<Box<dyn Iterator<Item = OwnedObjectV2IteratorItem> + '_>>;

    /// Returns an iterator over the dynamic fields of `parent`.
    ///
    /// The `cursor` bound is **inclusive**: if `Some(id)` is provided, the
    /// iterator starts *at* that object ID. Callers that wish to paginate past
    /// a previously-seen cursor must `.skip(1)` on the returned iterator to
    /// avoid re-returning the cursor item.
    // used in both "grpc-server" and "rest-api"
    fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> Result<Box<dyn Iterator<Item = DynamicFieldIteratorItem> + '_>>;

    // only used in "rest-api"
    fn get_coin_info(&self, coin_type: &StructTag) -> Result<Option<CoinInfo>>;

    /// Returns unified coin info from the `coin_v2` table (merges
    /// `coin` + `regulated_coin`).
    // only used in "grpc-server"
    fn get_coin_v2_info(&self, coin_type: &StructTag) -> Result<Option<CoinInfoV2>>;

    /// Returns an iterator over the versions of a package identified by
    /// `original_package_id`.
    // only used in "grpc-server"
    fn package_versions_iter(
        &self,
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> Result<Box<dyn Iterator<Item = PackageVersionIteratorItem> + '_>>;

    /// Returns `true` once the `owner_v2` backfill has completed.
    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_owner_v2_index_ready(&self) -> bool {
        true
    }

    /// Returns `true` once the `coin_v2` backfill has completed.
    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_coin_v2_index_ready(&self) -> bool {
        true
    }

    /// Returns `true` once the `package_version` backfill has completed and the
    /// index is ready to serve queries.  Defaults to `true` so that
    /// implementations without a backfill concept (e.g. simulacrum) are
    /// unaffected.
    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_package_version_index_ready(&self) -> bool {
        true
    }
}

pub type PackageVersionIteratorItem =
    Result<(PackageVersionKey, PackageVersionInfo), TypedStoreError>;

#[derive(Clone)]
pub struct AccountOwnedObjectInfo {
    pub owner: IotaAddress,
    pub object_id: ObjectID,
    pub version: SequenceNumber,
    pub type_: MoveObjectType,
}

/// Opaque cursor for seeking in the `owner_v2` index.
///
/// Mirrors the non-owner components of the v2 composite key so that
/// pagination can resume with a direct RocksDB seek instead of scanning
/// from the start.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnedObjectV2Cursor {
    pub object_type_identifier: u64,
    pub object_type_params: u64,
    pub inverted_balance: Option<u64>,
    pub object_id: ObjectID,
}

pub type OwnedObjectV2IteratorItem =
    Result<(AccountOwnedObjectInfo, OwnedObjectV2Cursor), TypedStoreError>;

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct DynamicFieldKey {
    pub parent: ObjectID,
    pub field_id: ObjectID,
}

impl DynamicFieldKey {
    pub fn new<P: Into<ObjectID>>(parent: P, field_id: ObjectID) -> Self {
        Self {
            parent: parent.into(),
            field_id,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct DynamicFieldIndexInfo {
    // field_id of this dynamic field is a part of the Key
    pub dynamic_field_type: DynamicFieldType,
    pub name_type: TypeTag,
    pub name_value: Vec<u8>,
    // TODO do we want to also store the type of the value? We can get this for free for
    // DynamicFields, but for DynamicObjects it would require a lookup in the DB on init, or
    // scanning the transaction's output objects for the coorisponding Object to retrieve its type
    // information.
    //
    // pub value_type: TypeTag,
    /// ObjectId of the child object when `dynamic_field_type ==
    /// DynamicFieldType::DynamicObject`
    pub dynamic_object_id: Option<ObjectID>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CoinInfo {
    pub coin_metadata_object_id: Option<ObjectID>,
    pub treasury_object_id: Option<ObjectID>,
}

/// Extended coin info from the `coin_v2` table — merges `coin` +
/// `regulated_coin` into a single lookup.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct CoinInfoV2 {
    pub coin_metadata_object_id: Option<ObjectID>,
    pub treasury_object_id: Option<ObjectID>,
    pub regulated_coin_metadata_object_id: Option<ObjectID>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PackageVersionKey {
    pub original_package_id: ObjectID,
    pub version: u64,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct PackageVersionInfo {
    pub storage_id: ObjectID,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct TransactionInfo {
    pub checkpoint: u64,
    pub object_types: HashMap<ObjectID, ObjectType>,
}

impl TransactionInfo {
    pub fn new(
        input_objects: &[Object],
        output_objects: &[Object],
        checkpoint: u64,
    ) -> TransactionInfo {
        let object_types = input_objects
            .iter()
            .chain(output_objects)
            .map(|object| (object.id(), ObjectType::from(object)))
            .collect();

        TransactionInfo {
            checkpoint,
            object_types,
        }
    }
}

/// Epoch information structure for indexing.
///
/// Contains metadata about an epoch including timing, checkpoints, protocol
/// version, and a snapshot of the system state at the start of the epoch.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct EpochInfo {
    pub epoch: u64,
    pub protocol_version: u64,
    pub start_timestamp_ms: u64,
    pub end_timestamp_ms: Option<u64>,
    pub start_checkpoint: u64,
    pub end_checkpoint: Option<u64>,
    pub reference_gas_price: u64,
    /// System State as of the start of the epoch
    pub system_state: crate::iota_system_state::IotaSystemState,
}
