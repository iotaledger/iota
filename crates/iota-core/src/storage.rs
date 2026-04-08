// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_types::{
    base_types::{IotaAddress, ObjectID, TransactionDigest},
    committee::{Committee, EpochId},
    effects::{TransactionEffects, TransactionEvents},
    error::IotaError,
    messages_checkpoint::{
        CheckpointContentsDigest, CheckpointDigest, CheckpointSequenceNumber, EndOfEpochData,
        FullCheckpointContents, VerifiedCheckpoint, VerifiedCheckpointContents,
    },
    object::Object,
    storage::{
        AccountOwnedObjectInfo, CoinInfo, CoinInfoV2, DynamicFieldIndexInfo, DynamicFieldKey,
        ObjectKey, ObjectStore, OwnedObjectV2Cursor, OwnedObjectV2IteratorItem, ReadStore,
        RestIndexes, RestStateReader, TransactionInfo, WriteStore,
        error::{Error as StorageError, Result},
    },
    transaction::VerifiedTransaction,
};
use move_core_types::language_storage::StructTag;
use parking_lot::Mutex;
use tap::Pipe;
use tracing::instrument;
use typed_store::TypedStoreError;

use crate::{
    authority::AuthorityState,
    checkpoints::CheckpointStore,
    epoch::committee_store::CommitteeStore,
    execution_cache::ExecutionCacheTraitPointers,
    rest_index::{CoinIndexInfo, OwnerIndexInfo, OwnerIndexKey, OwnerV2TypeFilter, RestIndexStore},
};

#[derive(Clone)]
pub struct RocksDbStore {
    cache_traits: ExecutionCacheTraitPointers,

    committee_store: Arc<CommitteeStore>,
    checkpoint_store: Arc<CheckpointStore>,
    // in memory checkpoint watermark sequence numbers
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

    fn try_get_highest_synced_checkpoint(&self) -> Result<VerifiedCheckpoint, StorageError> {
        self.checkpoint_store
            .get_highest_synced_checkpoint()
            .map(|maybe_checkpoint| {
                maybe_checkpoint
                    .expect("storage should have been initialized with genesis checkpoint")
            })
            .map_err(Into::into)
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
        self.checkpoint_store
            .get_full_checkpoint_contents_by_sequence_number(sequence_number)
            .map_err(Into::into)
    }

    fn try_get_full_checkpoint_contents(
        &self,
        digest: &CheckpointContentsDigest,
    ) -> Result<Option<FullCheckpointContents>, StorageError> {
        // First look to see if we saved the complete contents already.
        if let Some(seq_num) = self
            .checkpoint_store
            .get_sequence_number_by_contents_digest(digest)
            .map_err(iota_types::storage::error::Error::custom)?
        {
            let contents = self
                .checkpoint_store
                .get_full_checkpoint_contents_by_sequence_number(seq_num)
                .map_err(iota_types::storage::error::Error::custom)?;
            if contents.is_some() {
                return Ok(contents);
            }
        }

        // Otherwise gather it from the individual components.
        // Note we can't insert the constructed contents into `full_checkpoint_content`,
        // because it needs to be inserted along with
        // `checkpoint_sequence_by_contents_digest` and `checkpoint_content`.
        // However at this point it's likely we don't know the corresponding
        // sequence number yet.
        self.checkpoint_store
            .get_checkpoint_contents(digest)
            .map_err(iota_types::storage::error::Error::custom)?
            .map(|contents| {
                let mut transactions = Vec::with_capacity(contents.size());
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
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        self.checkpoint_store
            .get_checkpoint_contents(digest)
            .map_err(iota_types::storage::error::Error::custom)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        match self.try_get_checkpoint_by_sequence_number(sequence_number) {
            Ok(Some(checkpoint)) => {
                self.try_get_checkpoint_contents_by_digest(&checkpoint.content_digest)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl ObjectStore for RocksDbStore {
    fn try_get_object(
        &self,
        object_id: &iota_types::base_types::ObjectID,
    ) -> iota_types::storage::error::Result<Option<Object>> {
        self.cache_traits.object_store.try_get_object(object_id)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &iota_types::base_types::ObjectID,
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
            let next_committee = next_epoch_committee.iter().cloned().collect();
            let committee =
                Committee::new(checkpoint.epoch().checked_add(1).unwrap(), next_committee);
            self.try_insert_committee(committee)?;
        }

        self.checkpoint_store
            .insert_verified_checkpoint(checkpoint)
            .map_err(Into::into)
    }

    fn try_update_highest_synced_checkpoint(
        &self,
        checkpoint: &VerifiedCheckpoint,
    ) -> Result<(), iota_types::storage::error::Error> {
        let mut locked = self.highest_synced_checkpoint.lock();
        if locked.is_some() && locked.unwrap() >= checkpoint.sequence_number {
            return Ok(());
        }
        self.checkpoint_store
            .update_highest_synced_checkpoint(checkpoint)
            .map_err(iota_types::storage::error::Error::custom)?;
        *locked = Some(checkpoint.sequence_number);
        Ok(())
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
        *locked = Some(checkpoint.sequence_number);
        Ok(())
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
}

pub struct RestReadStore {
    state: Arc<AuthorityState>,
    rocks: RocksDbStore,
}

impl RestReadStore {
    pub fn new(state: Arc<AuthorityState>, rocks: RocksDbStore) -> Self {
        Self { state, rocks }
    }

    fn index(&self) -> iota_types::storage::error::Result<&RestIndexStore> {
        self.state.rest_index.as_deref().ok_or_else(|| {
            iota_types::storage::error::Error::custom("rest index store is disabled")
        })
    }
}

impl ObjectStore for RestReadStore {
    fn try_get_object(
        &self,
        object_id: &iota_types::base_types::ObjectID,
    ) -> iota_types::storage::error::Result<Option<Object>> {
        self.rocks.try_get_object(object_id)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &iota_types::base_types::ObjectID,
        version: iota_types::base_types::VersionNumber,
    ) -> iota_types::storage::error::Result<Option<Object>> {
        self.rocks.try_get_object_by_key(object_id, version)
    }
}

impl ReadStore for RestReadStore {
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

    fn try_get_highest_synced_checkpoint(
        &self,
    ) -> iota_types::storage::error::Result<VerifiedCheckpoint> {
        self.rocks.try_get_highest_synced_checkpoint()
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
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
        self.rocks.try_get_checkpoint_contents_by_digest(digest)
    }

    fn try_get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> iota_types::storage::error::Result<
        Option<iota_types::messages_checkpoint::CheckpointContents>,
    > {
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

impl RestStateReader for RestReadStore {
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

    fn indexes(&self) -> Option<&dyn RestIndexes> {
        self.index().ok().map(|index| index as _)
    }

    fn get_struct_layout(
        &self,
        struct_tag: &move_core_types::language_storage::StructTag,
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

impl RestIndexes for RestIndexStore {
    // only used in "grpc-server"
    fn get_epoch_info(&self, epoch: EpochId) -> Result<Option<iota_types::storage::EpochInfo>> {
        self.get_epoch_info(epoch).map_err(StorageError::custom)
    }

    // used in both "grpc-server" and "rest-api"
    fn get_transaction_info(
        &self,
        digest: &TransactionDigest,
    ) -> iota_types::storage::error::Result<Option<TransactionInfo>> {
        self.get_transaction_info(digest)
            .map_err(StorageError::custom)
    }

    /// **Performance note:** When `object_type` is `Some`, the filter is
    /// applied as a post-filter on the iterator — it scans **all** objects
    /// owned by `owner` (starting from `cursor`) and checks each one's type.
    /// This is O(N) in the total number of owned objects, not O(result-set).
    // only used in "rest-api"
    fn account_owned_objects_info_iter(
        &self,
        owner: IotaAddress,
        cursor: Option<ObjectID>,
        object_type: Option<StructTag>,
    ) -> Result<Box<dyn Iterator<Item = Result<AccountOwnedObjectInfo, TypedStoreError>> + '_>>
    {
        let iter = self
            .owner_iter(owner, cursor)?
            .map(|result| {
                result.map(
                    |(OwnerIndexKey { owner, object_id }, OwnerIndexInfo { version, type_ })| {
                        AccountOwnedObjectInfo {
                            owner,
                            object_id,
                            version,
                            type_,
                        }
                    },
                )
            })
            .filter(move |result| match (&object_type, result) {
                (None, _) => true,
                (_, Err(_)) => true,
                (Some(filter), Ok(info)) => {
                    let obj_type: StructTag = info.type_.clone().into();
                    if filter.type_params.is_empty() {
                        obj_type.address == filter.address
                            && obj_type.module == filter.module
                            && obj_type.name == filter.name
                    } else {
                        obj_type == *filter
                    }
                }
            });

        Ok(Box::new(iter) as _)
    }

    /// Uses the `owner_v2` table which supports hash-based type narrowing.
    /// When `object_type` is `Some`, the iterator only scans the hash
    /// bucket for that type rather than all owned objects.
    // only used in "grpc-server"
    fn account_owned_objects_info_iter_v2(
        &self,
        owner: IotaAddress,
        cursor: Option<&OwnedObjectV2Cursor>,
        object_type: Option<StructTag>,
    ) -> Result<Box<dyn Iterator<Item = OwnedObjectV2IteratorItem> + '_>> {
        let type_filter = OwnerV2TypeFilter::from_struct_tag(object_type.as_ref());
        let iter = self
            .owner_v2_iter(owner, cursor, type_filter)?
            .map(|result| {
                result.map(|(key, info)| {
                    let cursor = OwnedObjectV2Cursor {
                        object_type_identifier: key.object_type_identifier,
                        object_type_params: key.object_type_params,
                        inverted_balance: key.inverted_balance,
                        object_id: key.object_id,
                    };
                    let owned = AccountOwnedObjectInfo {
                        owner: key.owner,
                        object_id: key.object_id,
                        version: info.version,
                        type_: info.object_type.into(),
                    };
                    (owned, cursor)
                })
            });

        Ok(Box::new(iter) as _)
    }

    // used in both "grpc-server" and "rest-api"
    fn dynamic_field_iter(
        &self,
        parent: ObjectID,
        cursor: Option<ObjectID>,
    ) -> iota_types::storage::error::Result<
        Box<
            dyn Iterator<Item = Result<(DynamicFieldKey, DynamicFieldIndexInfo), TypedStoreError>>
                + '_,
        >,
    > {
        let iter = self.dynamic_field_iter(parent, cursor)?;
        Ok(Box::new(iter) as _)
    }

    // only used in "rest-api"
    fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> iota_types::storage::error::Result<Option<CoinInfo>> {
        self.get_coin_info(coin_type)?
            .map(
                |CoinIndexInfo {
                     coin_metadata_object_id,
                     treasury_object_id,
                 }| CoinInfo {
                    coin_metadata_object_id,
                    treasury_object_id,
                },
            )
            .pipe(Ok)
    }

    // only used in "grpc-server"
    fn get_coin_v2_info(
        &self,
        coin_type: &StructTag,
    ) -> iota_types::storage::error::Result<Option<CoinInfoV2>> {
        self.get_coin_v2_info(coin_type)?
            .map(CoinInfoV2::from)
            .pipe(Ok)
    }

    // only used in "grpc-server"
    fn package_versions_iter(
        &self,
        original_package_id: ObjectID,
        cursor: Option<u64>,
    ) -> iota_types::storage::error::Result<
        Box<dyn Iterator<Item = iota_types::storage::PackageVersionIteratorItem> + '_>,
    > {
        let iter = self.package_versions_iter(original_package_id, cursor)?;
        Ok(Box::new(iter) as _)
    }

    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_owner_v2_index_ready(&self) -> bool {
        self.is_owner_v2_index_ready()
    }

    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_coin_v2_index_ready(&self) -> bool {
        self.is_coin_v2_index_ready()
    }

    // only used in "grpc-server"
    // TODO(remove): https://github.com/iotaledger/iota/issues/10955
    fn is_package_version_index_ready(&self) -> bool {
        self.is_package_version_index_ready()
    }
}
