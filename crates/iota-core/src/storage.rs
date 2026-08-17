// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_node_storage::{GrpcIndexes, GrpcStateReader};
use iota_sdk_types::{
    CheckpointContentsDigest, CheckpointDigest, StructTag, TransactionDigest, TransactionEffects,
    TransactionEvents,
    checkpoint::{CheckpointContents, EndOfEpochData},
};
use iota_types::{
    committee::{Committee, EpochId},
    error::IotaError,
    messages_checkpoint::{
        CheckpointContentsExt, CheckpointSequenceNumber, FullCheckpointContents,
        VerifiedCheckpoint, VerifiedCheckpointContents,
    },
    object::Object,
    storage::{
        ObjectKey, ObjectStore, ReadStore, WriteStore,
        error::{Error as StorageError, Result},
    },
    transaction::VerifiedTransaction,
};
use parking_lot::Mutex;
use tracing::instrument;

use crate::{
    authority::AuthorityState,
    checkpoints::CheckpointStore,
    epoch::committee_store::CommitteeStore,
    execution_cache::ExecutionCacheTraitPointers,
    rpc_indexes::{RpcIndexesStore, schema::IndexGroup},
};

#[derive(Clone)]
pub struct RocksDbStore {
    cache_traits: ExecutionCacheTraitPointers,

    committee_store: Arc<CommitteeStore>,
    checkpoint_store: Arc<CheckpointStore>,
    // Lower bounds on the watermark rows, not mirrors of them: a row already
    // ahead leaves its cache behind, which costs a repeated call and nothing
    // else. Held so that the read and the write of a row happen under one
    // lock.
    highest_verified_checkpoint: Arc<Mutex<Option<u64>>>,
    highest_synced_checkpoint: Arc<Mutex<Option<u64>>>,
}

impl RocksDbStore {
    pub fn new(
        cache_traits: ExecutionCacheTraitPointers,
        committee_store: Arc<CommitteeStore>,
        checkpoint_store: Arc<CheckpointStore>,
    ) -> Self {
        Self {
            cache_traits,
            committee_store,
            checkpoint_store,
            highest_verified_checkpoint: Arc::new(Mutex::new(None)),
            highest_synced_checkpoint: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_objects(&self, object_keys: &[ObjectKey]) -> Result<Vec<Option<Object>>, IotaError> {
        self.cache_traits
            .object_cache_reader
            .try_multi_get_objects_by_key(object_keys)
    }

    pub fn get_last_executed_checkpoint(&self) -> Result<Option<VerifiedCheckpoint>, IotaError> {
        Ok(self.checkpoint_store.get_highest_executed_checkpoint()?)
    }

    /// Marks a consecutive run of checkpoints as synced, writing the watermark
    /// once for the last one but notifying waiters of every checkpoint.
    fn update_highest_synced_checkpoints(
        &self,
        checkpoints: &[VerifiedCheckpoint],
    ) -> Result<(), iota_types::storage::error::Error> {
        let Some(last) = checkpoints.last() else {
            return Ok(());
        };
        let mut locked = self.highest_synced_checkpoint.lock();
        if locked.is_some_and(|seq| seq >= last.sequence_number) {
            return Ok(());
        }
        self.checkpoint_store
            .multi_update_highest_synced_checkpoint(checkpoints)
            .map_err(iota_types::storage::error::Error::custom)?;
        *locked = locked.max(Some(last.sequence_number));
        Ok(())
    }
}

impl ReadStore for RocksDbStore {
    fn try_get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> Result<Option<VerifiedCheckpoint>, StorageError> {
        self.checkpoint_store
            .get_checkpoint_by_digest(digest)
            .map_err(Into::into)
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<VerifiedCheckpoint>, StorageError> {
        self.checkpoint_store
            .get_checkpoint_by_sequence_number(sequence_number)
            .map_err(Into::into)
    }

    fn try_get_highest_verified_checkpoint(&self) -> Result<VerifiedCheckpoint, StorageError> {
        self.checkpoint_store
            .get_highest_verified_checkpoint()
            .map(|maybe_checkpoint| {
                maybe_checkpoint
                    .expect("storage should have been initialized with genesis checkpoint")
            })
            .map_err(Into::into)
    }

    fn try_get_highest_verified_checkpoint_seq_number(
        &self,
    ) -> Result<CheckpointSequenceNumber, StorageError> {
        Ok(self
            .checkpoint_store
            .get_highest_verified_checkpoint_seq_number()?
            .expect("storage should have been initialized with genesis checkpoint"))
    }

    fn try_get_highest_synced_checkpoint(&self) -> Result<VerifiedCheckpoint, StorageError> {
        self.checkpoint_store
            .get_highest_synced_checkpoint()
            .map(|maybe_checkpoint| {
                maybe_checkpoint
                    .expect("storage should have been initialized with genesis checkpoint")
            })
            .map_err(Into::into)
    }

    fn try_get_highest_synced_checkpoint_seq_number(
        &self,
    ) -> Result<CheckpointSequenceNumber, StorageError> {
        Ok(self
            .checkpoint_store
            .get_highest_synced_checkpoint_seq_number()?
            .expect("storage should have been initialized with genesis checkpoint"))
    }

    fn try_get_lowest_available_checkpoint(
        &self,
    ) -> Result<CheckpointSequenceNumber, StorageError> {
        if let Some(highest_pruned_cp) = self
            .checkpoint_store
            .get_highest_pruned_checkpoint_seq_number()
            .map_err(Into::<StorageError>::into)?
        {
            Ok(highest_pruned_cp + 1)
        } else {
            Ok(0)
        }
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Result<Option<FullCheckpointContents>, StorageError> {
        Ok(self
            .checkpoint_store
            .get_full_checkpoint_contents_by_sequence_number(sequence_number)
            .map(|contents| contents.as_ref().clone()))
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<FullCheckpointContents>, StorageError> {
        // First look to see if the in-memory cache still holds the complete
        // contents.
        if let Some(contents) = self
            .checkpoint_store
            .get_full_checkpoint_contents_by_digest(digest)
        {
            return Ok(Some(contents.as_ref().clone()));
        }

        // Otherwise gather it from the individual components.
        self.checkpoint_store
            .get_checkpoint_contents(digest)
            .map_err(iota_types::storage::error::Error::custom)?
            .map(|contents| {
                let mut transactions = Vec::with_capacity(contents.len());
                for tx in contents.iter() {
                    if let (Some(t), Some(e)) = (
                        self.try_get_transaction(&tx.transaction)?,
                        self.cache_traits
                            .transaction_cache_reader
                            .try_get_effects(&tx.effects)
                            .map_err(iota_types::storage::error::Error::custom)?,
                    ) {
                        transactions.push(iota_types::base_types::ExecutionData::new(
                            (*t).clone().into_inner(),
                            e,
                        ))
                    } else {
                        return Result::<
                            Option<FullCheckpointContents>,
                            iota_types::storage::error::Error,
                        >::Ok(None);
                    }
                }
                Ok(Some(
                    FullCheckpointContents::from_contents_and_execution_data(
                        contents,
                        transactions.into_iter(),
                    ),
                ))
            })
            .transpose()
            .map(|contents| contents.flatten())
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn try_get_committee(
        &self,
        epoch: EpochId,
    ) -> Result<Option<Arc<Committee>>, iota_types::storage::error::Error> {
        Ok(self.committee_store.get_committee(&epoch).unwrap())
    }

    fn try_get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<Arc<VerifiedTransaction>>, StorageError> {
        self.cache_traits
            .transaction_cache_reader
            .try_get_transaction_block(digest)
            .map_err(StorageError::custom)
    }

    fn try_get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionEffects>, StorageError> {
        self.cache_traits
            .transaction_cache_reader
            .try_get_executed_effects(digest)
            .map_err(StorageError::custom)
    }

    fn try_get_events(
        &self,
        digest: &TransactionDigest,
    ) -> Result<Option<TransactionEvents>, StorageError> {
        self.cache_traits
            .transaction_cache_reader
            .try_get_events(digest)
            .map_err(StorageError::custom)
    }

    fn try_get_latest_checkpoint(&self) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.checkpoint_store
            .get_highest_executed_checkpoint()
            .map_err(iota_types::storage::error::Error::custom)?
            .ok_or_else(|| {
                iota_types::storage::error::Error::missing("unable to get latest checkpoint")
            })
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        self.checkpoint_store
            .get_checkpoint_contents(digest)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        match self.try_get_checkpoint_by_sequence_number(sequence_number) {
            Ok(Some(checkpoint)) => {
                self.try_get_checkpoint_contents_by_digest(&checkpoint.contents_digest)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl ObjectStore for RocksDbStore {
    fn try_get_object(
        &self,
        object_id: &iota_sdk_types::ObjectId,
    ) -> iota_types::storage::error::Result<Option<Object>> {
        self.cache_traits.object_store.try_get_object(object_id)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &iota_sdk_types::ObjectId,
        version: iota_types::base_types::VersionNumber,
    ) -> iota_types::storage::error::Result<Option<Object>> {
        self.cache_traits
            .object_store
            .try_get_object_by_key(object_id, version)
    }
}

impl WriteStore for RocksDbStore {
    #[instrument(level = "trace", skip_all)]
    fn try_insert_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> Result<(), iota_types::storage::error::Error> {
        if let Some(EndOfEpochData {
            next_epoch_committee,
            ..
        }) = checkpoint.end_of_epoch_data.as_ref()
        {
            let committee = Committee::from_committee_members(
                checkpoint.epoch().checked_add(1).unwrap(),
                next_epoch_committee,
            );
            self.try_insert_committee(committee)?;
        }

        self.checkpoint_store
            .insert_certified_checkpoint(checkpoint)
            .map_err(iota_types::storage::error::Error::custom)?;
        // Not `insert_verified_checkpoint`, which would write the watermark
        // straight to the store: taking the same lock the archive path takes
        // is what stops two writers leaving the row behind where one of them
        // had already seen it.
        self.try_update_highest_verified_checkpoint(checkpoint)
    }

    fn try_update_highest_verified_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> Result<(), iota_types::storage::error::Error> {
        let mut locked = self.highest_verified_checkpoint.lock();
        if locked.is_some() && locked.unwrap() >= checkpoint.sequence_number {
            return Ok(());
        }
        self.checkpoint_store
            .update_highest_verified_checkpoint(checkpoint)
            .map_err(iota_types::storage::error::Error::custom)?;
        *locked = locked.max(Some(checkpoint.sequence_number));
        Ok(())
    }

    fn try_update_highest_synced_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> Result<(), iota_types::storage::error::Error> {
        self.update_highest_synced_checkpoints(std::slice::from_ref(checkpoint))
    }

    fn try_insert_checkpoint_contents(
        &self,
        checkpoint: &VerifiedCheckpoint,
        contents: VerifiedCheckpointContents,
    ) -> Result<(), iota_types::storage::error::Error> {
        self.cache_traits
            .state_sync_store
            .try_multi_insert_transaction_and_effects(contents.transactions())
            .map_err(iota_types::storage::error::Error::custom)?;
        self.checkpoint_store
            .insert_verified_checkpoint_contents(checkpoint, contents)
            .map_err(Into::into)
    }

    fn try_insert_committee(
        &self,
        new_committee: Committee,
    ) -> Result<(), iota_types::storage::error::Error> {
        self.committee_store
            .insert_new_committee(&new_committee)
            .unwrap();
        Ok(())
    }

    fn try_get_highest_executed_checkpoint_seq_number(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>, iota_types::storage::error::Error> {
        self.checkpoint_store
            .get_highest_executed_checkpoint_seq_number()
            .map_err(Into::into)
    }

    async fn wait_for_executed_checkpoint(&self, sequence_number: CheckpointSequenceNumber) {
        self.checkpoint_store
            .notify_read_executed_checkpoint(sequence_number)
            .await;
    }

    fn try_insert_synced_checkpoints(
        &self,
        checkpoints: Vec<(VerifiedCheckpoint, VerifiedCheckpointContents)>,
    ) -> Result<(), iota_types::storage::error::Error> {
        let summaries: Vec<VerifiedCheckpoint> = checkpoints
            .iter()
            .map(|(checkpoint, _)| checkpoint.clone())
            .collect();
        let Some(last) = summaries.last() else {
            return Ok(());
        };

        for checkpoint in &summaries {
            if let Some(EndOfEpochData {
                next_epoch_committee,
                ..
            }) = checkpoint.end_of_epoch_data.as_ref()
            {
                let committee = Committee::from_committee_members(
                    checkpoint.epoch().checked_add(1).unwrap(),
                    next_epoch_committee,
                );
                self.try_insert_committee(committee)?;
            }
        }

        self.checkpoint_store
            .multi_insert_certified_checkpoints(&summaries)?;
        self.try_update_highest_verified_checkpoint(last)?;

        // Transactions and effects must be durable before their contents
        // rows (see `CheckpointStore::cache_full_checkpoint_contents`).
        for (_, contents) in &checkpoints {
            self.cache_traits
                .state_sync_store
                .try_multi_insert_transaction_and_effects(contents.transactions())
                .map_err(iota_types::storage::error::Error::custom)?;
        }
        self.checkpoint_store
            .multi_insert_verified_checkpoint_contents(checkpoints)?;

        self.update_highest_synced_checkpoints(&summaries)
    }
}

pub struct GrpcReadStore {
    state: Arc<AuthorityState>,
    rocks: RocksDbStore,
}

impl GrpcReadStore {
    pub fn new(state: Arc<AuthorityState>, rocks: RocksDbStore) -> Self {
        Self { state, rocks }
    }

    /// The index store when this node maintains the gRPC group's tables.
    fn grpc_indexes_store(&self) -> iota_types::storage::error::Result<&RpcIndexesStore> {
        self.state
            .rpc_indexes_store
            .as_deref()
            .filter(|store| store.serves(IndexGroup::Grpc))
            .ok_or_else(|| {
                iota_types::storage::error::Error::custom("gRPC index store is disabled")
            })
    }
}

impl ObjectStore for GrpcReadStore {
    fn try_get_object(
        &self,
        object_id: &iota_sdk_types::ObjectId,
    ) -> iota_types::storage::error::Result<Option<Object>> {
        self.rocks.try_get_object(object_id)
    }

    /// Unlike [`RocksDbStore`], this reads through to the historic buckets
    /// after a live miss. Everything served from here is a gRPC response,
    /// and responses read past versions by exact key: a transaction's input
    /// pre-images, a checkpoint's transaction objects, an explicitly
    /// requested past version. Those versions leave the live table when
    /// their checkpoint commits.
    ///
    /// `RocksDbStore` keeps no fallback because state sync holds it too, and
    /// there a miss is a bug rather than a relocated version.
    fn try_get_object_by_key(
        &self,
        object_id: &iota_sdk_types::ObjectId,
        version: iota_types::base_types::VersionNumber,
    ) -> iota_types::storage::error::Result<Option<Object>> {
        self.state
            .get_object_with_historic_fallback(&ObjectKey(*object_id, version))
            .map_err(StorageError::custom)
    }
}

impl ReadStore for GrpcReadStore {
    fn try_get_committee(
        &self,
        epoch: EpochId,
    ) -> iota_types::storage::error::Result<Option<Arc<Committee>>> {
        self.rocks.try_get_committee(epoch)
    }

    fn try_get_latest_checkpoint(&self) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.rocks.try_get_latest_checkpoint()
    }

    fn try_get_highest_verified_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.rocks.try_get_highest_verified_checkpoint()
    }

    fn try_get_highest_verified_checkpoint_seq_number(
        &self,
    ) -> iota_types::storage::error::Result<CheckpointSequenceNumber> {
        self.rocks.try_get_highest_verified_checkpoint_seq_number()
    }

    fn try_get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.rocks.try_get_highest_synced_checkpoint()
    }

    fn try_get_highest_synced_checkpoint_seq_number(
        &self,
    ) -> iota_types::storage::error::Result<CheckpointSequenceNumber> {
        self.rocks.try_get_highest_synced_checkpoint_seq_number()
    }

    fn try_get_lowest_available_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<CheckpointSequenceNumber> {
        self.rocks.try_get_lowest_available_checkpoint()
    }

    fn try_get_checkpoint_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        self.rocks.try_get_checkpoint_by_digest(digest)
    }

    fn try_get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        self.rocks
            .try_get_checkpoint_by_sequence_number(sequence_number)
    }

    fn try_get_checkpoint_contents_by_digest(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        self.rocks.try_get_checkpoint_contents_by_digest(digest)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<CheckpointContents>> {
        self.rocks
            .try_get_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<Arc<VerifiedTransaction>>> {
        self.rocks.try_get_transaction(digest)
    }

    fn try_get_transaction_effects(
        &self,
        digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionEffects>> {
        self.rocks.try_get_transaction_effects(digest)
    }

    fn try_get_events(
        &self,
        digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionEvents>> {
        self.rocks.try_get_events(digest)
    }

    fn try_get_full_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<Option<FullCheckpointContents>> {
        self.rocks
            .try_get_full_checkpoint_contents_by_sequence_number(sequence_number)
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> iota_types::storage::error::Result<Option<FullCheckpointContents>> {
        self.rocks.try_get_full_checkpoint_contents(digest)
    }
}

impl GrpcStateReader for GrpcReadStore {
    fn get_lowest_available_checkpoint_objects(
        &self,
    ) -> iota_types::storage::error::Result<CheckpointSequenceNumber> {
        Ok(self
            .state
            .get_object_cache_reader()
            .try_get_highest_pruned_checkpoint()
            .map_err(StorageError::custom)?
            .map(|cp| cp + 1)
            .unwrap_or(0))
    }

    fn get_chain_identifier(&self) -> Result<iota_types::digests::ChainIdentifier> {
        Ok(self.state.get_chain_identifier())
    }

    fn get_epoch_last_checkpoint(
        &self,
        epoch_id: EpochId,
    ) -> iota_types::storage::error::Result<Option<VerifiedCheckpoint>> {
        self.rocks
            .checkpoint_store
            .get_epoch_last_checkpoint(epoch_id)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn get_epoch_info(
        &self,
        epoch: EpochId,
    ) -> iota_types::storage::error::Result<Option<iota_types::storage::EpochInfoV2>> {
        self.rocks
            .checkpoint_store
            .get_epoch_info(epoch)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn grpc_indexes(&self) -> Option<&dyn GrpcIndexes> {
        self.grpc_indexes_store().ok().map(|index| index as _)
    }

    fn get_transaction_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<CheckpointSequenceNumber>> {
        Ok(self
            .state
            .get_checkpoint_cache()
            .try_get_transaction_perpetual_checkpoint(digest)
            .map_err(iota_types::storage::error::Error::custom)?
            .map(|(_epoch, checkpoint)| checkpoint))
    }

    fn get_struct_layout(
        &self,
        struct_tag: &StructTag,
    ) -> Result<Option<move_core_types::annotated_value::MoveTypeLayout>> {
        self.state
            .load_epoch_store_one_call_per_task()
            .executor()
            // TODO(cache) - must read through cache
            .type_layout_resolver(Box::new(self.state.get_backing_package_store().as_ref()))
            .get_annotated_layout(struct_tag)
            .map(|layout| layout.into_layout())
            .map(Some)
            .map_err(StorageError::custom)
    }
}
