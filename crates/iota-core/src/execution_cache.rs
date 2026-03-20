// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, path::Path, sync::Arc};

use futures::{FutureExt, future::BoxFuture};
use iota_common::fatal;
use iota_config::{ExecutionCacheConfig, ExecutionCacheType};
use iota_types::{
    base_types::{EpochId, ObjectID, ObjectRef, SequenceNumber, VerifiedExecutionData},
    digests::{TransactionDigest, TransactionEffectsDigest},
    effects::{TransactionEffects, TransactionEvents},
    error::{IotaError, IotaResult, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    iota_system_state::IotaSystemState,
    messages_checkpoint::CheckpointSequenceNumber,
    object::{Object, Owner},
    storage::{
        BackingPackageStore, BackingStore, ChildObjectResolver, InputKey, MarkerValue, ObjectKey,
        ObjectOrTombstone, ObjectStore, PackageObject,
        error::{Error as StorageError, Result as StorageResult},
    },
    transaction::{VerifiedSignedTransaction, VerifiedTransaction},
};
use prometheus::Registry;
use tracing::instrument;
use typed_store::rocks::DBBatch;

use crate::{
    authority::{
        AuthorityStore,
        authority_per_epoch_store::AuthorityPerEpochStore,
        authority_store::{ExecutionLockWriteGuard, IotaLockResult, ObjectLockStatus},
        backpressure::BackpressureManager,
        epoch_start_configuration::{EpochFlag, EpochStartConfiguration},
    },
    state_accumulator::AccumulatorStore,
    transaction_outputs::TransactionOutputs,
};

pub(crate) mod cache_types;
pub mod metrics;
mod object_locks;
pub mod passthrough_cache;
pub mod proxy_cache;
pub mod writeback_cache;

use metrics::ExecutionCacheMetrics;
pub use passthrough_cache::PassthroughCache;
pub use proxy_cache::ProxyCache;
pub use writeback_cache::WritebackCache;

// If you have Arc<ExecutionCache>, you cannot return a reference to it as
// an &Arc<dyn ExecutionCacheRead> (for example), because the trait object is a
// fat pointer. So, in order to be able to return &Arc<dyn T>, we create all the
// converted trait objects (aka fat pointers) up front and return references to
// them.
#[derive(Clone)]
pub struct ExecutionCacheTraitPointers {
    pub object_cache_reader: Arc<dyn ObjectCacheRead>,
    pub transaction_cache_reader: Arc<dyn TransactionCacheRead>,
    pub cache_writer: Arc<dyn ExecutionCacheWrite>,
    pub backing_store: Arc<dyn BackingStore + Send + Sync>,
    pub backing_package_store: Arc<dyn BackingPackageStore + Send + Sync>,
    pub object_store: Arc<dyn ObjectStore + Send + Sync>,
    pub reconfig_api: Arc<dyn ExecutionCacheReconfigAPI>,
    pub accumulator_store: Arc<dyn AccumulatorStore>,
    pub checkpoint_cache: Arc<dyn CheckpointCache>,
    pub state_sync_store: Arc<dyn StateSyncAPI>,
    pub cache_commit: Arc<dyn ExecutionCacheCommit>,
    pub testing_api: Arc<dyn TestingAPI>,
}

impl ExecutionCacheTraitPointers {
    pub fn new<T>(cache: Arc<T>) -> Self
    where
        T: ObjectCacheRead
            + TransactionCacheRead
            + ExecutionCacheWrite
            + BackingStore
            + BackingPackageStore
            + ObjectStore
            + ExecutionCacheReconfigAPI
            + AccumulatorStore
            + CheckpointCache
            + StateSyncAPI
            + ExecutionCacheCommit
            + TestingAPI
            + 'static,
    {
        Self {
            object_cache_reader: cache.clone(),
            transaction_cache_reader: cache.clone(),
            cache_writer: cache.clone(),
            backing_store: cache.clone(),
            backing_package_store: cache.clone(),
            object_store: cache.clone(),
            reconfig_api: cache.clone(),
            accumulator_store: cache.clone(),
            checkpoint_cache: cache.clone(),
            state_sync_store: cache.clone(),
            cache_commit: cache.clone(),
            testing_api: cache,
        }
    }
}

pub fn build_execution_cache(
    cache_config: &ExecutionCacheConfig,
    epoch_start_config: &EpochStartConfiguration,
    prometheus_registry: &Registry,
    store: &Arc<AuthorityStore>,
    backpressure_manager: Arc<BackpressureManager>,
) -> ExecutionCacheTraitPointers {
    let execution_cache_metrics = Arc::new(ExecutionCacheMetrics::new(prometheus_registry));

    ExecutionCacheTraitPointers::new(
        ProxyCache::new(
            cache_config,
            epoch_start_config,
            store.clone(),
            execution_cache_metrics,
            backpressure_manager,
        )
        .into(),
    )
}

/// Should only be used for iota-tool or tests. Nodes must use
/// build_execution_cache which uses the epoch_start_config to prevent cache
/// impl from switching except at epoch boundaries.
pub fn build_execution_cache_from_env(
    prometheus_registry: &Registry,
    store: &Arc<AuthorityStore>,
) -> ExecutionCacheTraitPointers {
    let execution_cache_metrics = Arc::new(ExecutionCacheMetrics::new(prometheus_registry));

    // Load cache type from env
    let cache_type = ExecutionCacheType::default().cache_type();
    let config = ExecutionCacheConfig::default();
    match cache_type {
        ExecutionCacheType::PassthroughCache => ExecutionCacheTraitPointers::new(
            PassthroughCache::new(store.clone(), execution_cache_metrics).into(),
        ),
        ExecutionCacheType::WritebackCache => ExecutionCacheTraitPointers::new(
            WritebackCache::new(
                &config.writeback_cache,
                store.clone(),
                execution_cache_metrics,
                BackpressureManager::new_for_tests(),
            )
            .into(),
        ),
    }
}

pub type Batch = (Vec<Arc<TransactionOutputs>>, DBBatch);

pub trait ExecutionCacheCommit: Send + Sync {
    /// Build a DBBatch containing the given transaction outputs.
    fn build_db_batch(&self, epoch: EpochId, digests: &[TransactionDigest]) -> Batch;

    /// Durably commit the outputs of the given transactions to the database.
    /// Will be called by CheckpointExecutor to ensure that transaction outputs
    /// are written durably before marking a checkpoint as finalized.
    fn try_commit_transaction_outputs(
        &self,
        epoch: EpochId,
        batch: Batch,
        digests: &[TransactionDigest],
    ) -> IotaResult;

    /// Non-fallible version of `try_commit_transaction_outputs`.
    fn commit_transaction_outputs(
        &self,
        epoch: EpochId,
        batch: Batch,
        digests: &[TransactionDigest],
    ) {
        self.try_commit_transaction_outputs(epoch, batch, digests)
            .expect("storage access failed");
    }

    /// Durably commit a transaction to the database. Used to store any
    /// transactions that cannot be reconstructed at start-up by consensus
    /// replay. Currently the only case of this is RandomnessStateUpdate.
    fn try_persist_transaction(&self, tx: &VerifiedExecutableTransaction) -> IotaResult;

    /// Non-fallible version of `try_persist_transactions`.
    fn persist_transaction(&self, tx: &VerifiedExecutableTransaction) {
        self.try_persist_transaction(tx)
            .expect("storage access failed")
    }

    // Number of pending uncommitted transactions
    fn approximate_pending_transaction_count(&self) -> u64;
}

pub trait ObjectCacheRead: Send + Sync {
    fn try_get_package_object(&self, id: &ObjectID) -> IotaResult<Option<PackageObject>>;

    /// Non-fallible version of `try_get_package_object`.
    fn get_package_object(&self, id: &ObjectID) -> Option<PackageObject> {
        self.try_get_package_object(id)
            .expect("storage access failed")
    }

    fn force_reload_system_packages(&self, system_package_ids: &[ObjectID]);

    fn try_get_object(&self, id: &ObjectID) -> IotaResult<Option<Object>>;

    /// Non-fallible version of `try_get_object`.
    fn get_object(&self, id: &ObjectID) -> Option<Object> {
        self.try_get_object(id).expect("storage access failed")
    }

    fn try_get_objects(&self, objects: &[ObjectID]) -> IotaResult<Vec<Option<Object>>> {
        let mut ret = Vec::with_capacity(objects.len());
        for object_id in objects {
            ret.push(self.try_get_object(object_id)?);
        }
        Ok(ret)
    }

    /// Non-fallible version of `try_get_objects`.
    fn get_objects(&self, objects: &[ObjectID]) -> Vec<Option<Object>> {
        self.try_get_objects(objects)
            .expect("storage access failed")
    }

    fn try_get_latest_object_ref_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<ObjectRef>>;

    /// Non-fallible version of `try_get_latest_object_ref_or_tombstone`.
    fn get_latest_object_ref_or_tombstone(&self, object_id: ObjectID) -> Option<ObjectRef> {
        self.try_get_latest_object_ref_or_tombstone(object_id)
            .expect("storage access failed")
    }

    fn try_get_latest_object_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<(ObjectKey, ObjectOrTombstone)>>;

    /// Non-fallible version of `try_get_latest_object_or_tombstone`.
    fn get_latest_object_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> Option<(ObjectKey, ObjectOrTombstone)> {
        self.try_get_latest_object_or_tombstone(object_id)
            .expect("storage access failed")
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>>;

    /// Non-fallible version of `try_get_object_by_key`.
    fn get_object_by_key(&self, object_id: &ObjectID, version: SequenceNumber) -> Option<Object> {
        self.try_get_object_by_key(object_id, version)
            .expect("storage access failed")
    }

    fn try_multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> IotaResult<Vec<Option<Object>>>;

    /// Non-fallible version of `try_multi_get_objects_by_key`.
    fn multi_get_objects_by_key(&self, object_keys: &[ObjectKey]) -> Vec<Option<Object>> {
        self.try_multi_get_objects_by_key(object_keys)
            .expect("storage access failed")
    }

    fn try_object_exists_by_key(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<bool>;

    /// Non-fallible version of `try_object_exists_by_key`.
    fn object_exists_by_key(&self, object_id: &ObjectID, version: SequenceNumber) -> bool {
        self.try_object_exists_by_key(object_id, version)
            .expect("storage access failed")
    }

    fn try_multi_object_exists_by_key(&self, object_keys: &[ObjectKey]) -> IotaResult<Vec<bool>>;

    /// Non-fallible version of `try_multi_object_exists_by_key`.
    fn multi_object_exists_by_key(&self, object_keys: &[ObjectKey]) -> Vec<bool> {
        self.try_multi_object_exists_by_key(object_keys)
            .expect("storage access failed")
    }

    /// Load a list of objects from the store by object reference.
    /// If they exist in the store, they are returned directly.
    /// If any object missing, we try to figure out the best error to return.
    /// If the object we are asking is currently locked at a future version, we
    /// know this transaction is out-of-date and we return a
    /// ObjectVersionUnavailableForConsumption, which indicates this is not
    /// retriable. Otherwise, we return a ObjectNotFound error, which
    /// indicates this is retriable.
    fn try_multi_get_objects_with_more_accurate_error_return(
        &self,
        object_refs: &[ObjectRef],
    ) -> Result<Vec<Object>, IotaError> {
        let objects = self.try_multi_get_objects_by_key(
            &object_refs.iter().map(ObjectKey::from).collect::<Vec<_>>(),
        )?;
        let mut result = Vec::new();
        for (object_opt, object_ref) in objects.into_iter().zip(object_refs) {
            match object_opt {
                None => {
                    let live_objref = self._try_get_live_objref(object_ref.0)?;
                    let error = if live_objref.1 >= object_ref.1 {
                        UserInputError::ObjectVersionUnavailableForConsumption {
                            provided_obj_ref: *object_ref,
                            current_version: live_objref.1,
                        }
                    } else {
                        UserInputError::ObjectNotFound {
                            object_id: object_ref.0,
                            version: Some(object_ref.1),
                        }
                    };
                    return Err(IotaError::UserInput { error });
                }
                Some(object) => {
                    result.push(object);
                }
            }
        }
        assert_eq!(result.len(), object_refs.len());
        Ok(result)
    }

    /// Non-fallible version of
    /// `try_multi_get_objects_with_more_accurate_error_return`.
    fn multi_get_objects_with_more_accurate_error_return(
        &self,
        object_refs: &[ObjectRef],
    ) -> Vec<Object> {
        self.try_multi_get_objects_with_more_accurate_error_return(object_refs)
            .expect("storage access failed")
    }

    /// Used by transaction manager to determine if input objects are ready.
    /// Distinct from multi_get_object_by_key because it also consults
    /// markers to handle the case where an object will never become available
    /// (e.g. because it has been received by some other transaction
    /// already).
    fn try_multi_input_objects_available(
        &self,
        keys: &[InputKey],
        receiving_objects: HashSet<InputKey>,
        epoch: EpochId,
    ) -> Result<Vec<bool>, IotaError> {
        let (keys_with_version, keys_without_version): (Vec<_>, Vec<_>) = keys
            .iter()
            .enumerate()
            .partition(|(_, key)| key.version().is_some());

        let mut versioned_results = vec![];
        for ((idx, input_key), has_key) in keys_with_version.iter().zip(
            self.try_multi_object_exists_by_key(
                &keys_with_version
                    .iter()
                    .map(|(_, k)| ObjectKey(k.id(), k.version().unwrap()))
                    .collect::<Vec<_>>(),
            )?
            .into_iter(),
        ) {
            assert!(
                input_key.version().is_none() || input_key.version().unwrap().is_valid(),
                "Shared objects in cancelled transaction should always be available immediately, 
                 but it appears that transaction manager is waiting for {input_key:?} to become available"
            );
            // If the key exists at the specified version, then the object is available.
            if has_key {
                versioned_results.push((*idx, true))
            } else if receiving_objects.contains(input_key) {
                // There could be a more recent version of this object, and the object at the
                // specified version could have already been pruned. In such a case `has_key`
                // will be false, but since this is a receiving object we should
                // mark it as available if we can determine that an object with
                // a version greater than or equal to the specified version
                // exists or was deleted. We will then let mark it as available
                // to let the transaction through so it can fail at execution.
                let is_available = self
                    .try_get_object(&input_key.id())?
                    .map(|obj| obj.version() >= input_key.version().unwrap())
                    .unwrap_or(false)
                    || self.try_have_deleted_owned_object_at_version_or_after(
                        &input_key.id(),
                        input_key.version().unwrap(),
                        epoch,
                    )?;
                versioned_results.push((*idx, is_available));
            } else if self
                .try_get_deleted_shared_object_previous_tx_digest(
                    &input_key.id(),
                    input_key.version().unwrap(),
                    epoch,
                )?
                .is_some()
            {
                // If the object is an already deleted shared object, mark it as available if
                // the version for that object is in the shared deleted marker
                // table.
                versioned_results.push((*idx, true));
            } else {
                versioned_results.push((*idx, false));
            }
        }

        let unversioned_results = keys_without_version.into_iter().map(|(idx, key)| {
            (
                idx,
                match self
                    .try_get_latest_object_ref_or_tombstone(key.id())
                    .expect("read cannot fail")
                {
                    None => false,
                    Some(entry) => entry.2.is_alive(),
                },
            )
        });

        let mut results = versioned_results
            .into_iter()
            .chain(unversioned_results)
            .collect::<Vec<_>>();
        results.sort_by_key(|(idx, _)| *idx);
        Ok(results.into_iter().map(|(_, result)| result).collect())
    }

    /// Non-fallible version of `try_multi_input_objects_available`.
    fn multi_input_objects_available(
        &self,
        keys: &[InputKey],
        receiving_objects: HashSet<InputKey>,
        epoch: EpochId,
    ) -> Vec<bool> {
        self.try_multi_input_objects_available(keys, receiving_objects, epoch)
            .expect("storage access failed")
    }

    /// Return the object with version less then or eq to the provided seq
    /// number. This is used by indexer to find the correct version of
    /// dynamic field child object. We do not store the version of the child
    /// object, but because of lamport timestamp, we know the child must
    /// have version number less then or eq to the parent.
    fn try_find_object_lt_or_eq_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>>;

    /// Non-fallible version of `try_find_object_lt_or_eq_version`.
    fn find_object_lt_or_eq_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> Option<Object> {
        self.try_find_object_lt_or_eq_version(object_id, version)
            .expect("storage access failed")
    }

    fn try_get_lock(
        &self,
        obj_ref: ObjectRef,
        epoch_store: &AuthorityPerEpochStore,
    ) -> IotaLockResult;

    /// Non-fallible version of `try_get_lock`.
    fn get_lock(
        &self,
        obj_ref: ObjectRef,
        epoch_store: &AuthorityPerEpochStore,
    ) -> ObjectLockStatus {
        self.try_get_lock(obj_ref, epoch_store)
            .expect("storage access failed")
    }

    // This method is considered "private" - only used by
    // multi_get_objects_with_more_accurate_error_return
    fn _try_get_live_objref(&self, object_id: ObjectID) -> IotaResult<ObjectRef>;

    // Check that the given set of objects are live at the given version. This is
    // used as a safety check before execution, and could potentially be deleted
    // or changed to a debug_assert
    fn try_check_owned_objects_are_live(&self, owned_object_refs: &[ObjectRef]) -> IotaResult;

    /// Non-fallible version of `try_check_owned_objects_are_live`.
    fn check_owned_objects_are_live(&self, owned_object_refs: &[ObjectRef]) {
        self.try_check_owned_objects_are_live(owned_object_refs)
            .expect("storage access failed")
    }

    fn try_get_iota_system_state_object_unsafe(&self) -> IotaResult<IotaSystemState>;

    /// Non-fallible version of `try_get_iota_system_state_object_unsafe`.
    fn get_iota_system_state_object_unsafe(&self) -> IotaSystemState {
        self.try_get_iota_system_state_object_unsafe()
            .expect("storage access failed")
    }

    // Marker methods

    /// Get the marker at a specific version
    fn try_get_marker_value(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<MarkerValue>>;

    /// Non-fallible version of `try_get_marker_value`.
    fn get_marker_value(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> Option<MarkerValue> {
        self.try_get_marker_value(object_id, version, epoch_id)
            .expect("storage access failed")
    }

    /// Get the latest marker for a given object.
    fn try_get_latest_marker(
        &self,
        object_id: &ObjectID,
        epoch_id: EpochId,
    ) -> IotaResult<Option<(SequenceNumber, MarkerValue)>>;

    /// Non-fallible version of `try_get_latest_marker`.
    fn get_latest_marker(
        &self,
        object_id: &ObjectID,
        epoch_id: EpochId,
    ) -> Option<(SequenceNumber, MarkerValue)> {
        self.try_get_latest_marker(object_id, epoch_id)
            .expect("storage access failed")
    }

    /// If the shared object was deleted, return deletion info for the current
    /// live version
    fn try_get_last_shared_object_deletion_info(
        &self,
        object_id: &ObjectID,
        epoch_id: EpochId,
    ) -> IotaResult<Option<(SequenceNumber, TransactionDigest)>> {
        match self.try_get_latest_marker(object_id, epoch_id)? {
            Some((version, MarkerValue::SharedDeleted(digest))) => Ok(Some((version, digest))),
            _ => Ok(None),
        }
    }

    /// Non-fallible version of `try_get_last_shared_object_deletion_info`.
    fn get_last_shared_object_deletion_info(
        &self,
        object_id: &ObjectID,
        epoch_id: EpochId,
    ) -> Option<(SequenceNumber, TransactionDigest)> {
        self.try_get_last_shared_object_deletion_info(object_id, epoch_id)
            .expect("storage access failed")
    }

    /// If the shared object was deleted, return deletion info for the specified
    /// version.
    fn try_get_deleted_shared_object_previous_tx_digest(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<TransactionDigest>> {
        match self.try_get_marker_value(object_id, version, epoch_id)? {
            Some(MarkerValue::SharedDeleted(digest)) => Ok(Some(digest)),
            _ => Ok(None),
        }
    }

    /// Non-fallible version of
    /// `try_get_deleted_shared_object_previous_tx_digest`.
    fn get_deleted_shared_object_previous_tx_digest(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> Option<TransactionDigest> {
        self.try_get_deleted_shared_object_previous_tx_digest(object_id, version, epoch_id)
            .expect("storage access failed")
    }

    fn try_have_received_object_at_version(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<bool> {
        match self.try_get_marker_value(object_id, version, epoch_id)? {
            Some(MarkerValue::Received) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Non-fallible version of `try_have_received_object_at_version`.
    fn have_received_object_at_version(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> bool {
        self.try_have_received_object_at_version(object_id, version, epoch_id)
            .expect("storage access failed")
    }

    fn try_have_deleted_owned_object_at_version_or_after(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<bool> {
        match self.try_get_latest_marker(object_id, epoch_id)? {
            Some((marker_version, MarkerValue::OwnedDeleted)) if marker_version >= version => {
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Non-fallible version of
    /// `try_have_deleted_owned_object_at_version_or_after`.
    fn have_deleted_owned_object_at_version_or_after(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> bool {
        self.try_have_deleted_owned_object_at_version_or_after(object_id, version, epoch_id)
            .expect("storage access failed")
    }

    /// Return the watermark for the highest checkpoint for which we've pruned
    /// objects.
    fn try_get_highest_pruned_checkpoint(&self) -> IotaResult<Option<CheckpointSequenceNumber>>;

    /// Non-fallible version of `try_get_highest_pruned_checkpoint`.
    fn get_highest_pruned_checkpoint(&self) -> Option<CheckpointSequenceNumber> {
        self.try_get_highest_pruned_checkpoint()
            .expect("storage access failed")
    }
}

pub trait TransactionCacheRead: Send + Sync {
    fn try_multi_get_transaction_blocks(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<Arc<VerifiedTransaction>>>>;

    /// Non-fallible version of `try_multi_get_transaction_blocks`.
    fn multi_get_transaction_blocks(
        &self,
        digests: &[TransactionDigest],
    ) -> Vec<Option<Arc<VerifiedTransaction>>> {
        self.try_multi_get_transaction_blocks(digests)
            .expect("storage access failed")
    }

    fn try_get_transaction_block(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<Arc<VerifiedTransaction>>> {
        self.try_multi_get_transaction_blocks(&[*digest])
            .map(|mut blocks| {
                blocks
                    .pop()
                    .expect("multi-get must return correct number of items")
            })
    }

    /// Non-fallible version of `try_get_transaction_block`.
    fn get_transaction_block(
        &self,
        digest: &TransactionDigest,
    ) -> Option<Arc<VerifiedTransaction>> {
        self.try_get_transaction_block(digest)
            .expect("storage access failed")
    }

    #[instrument(level = "trace", skip_all)]
    fn try_get_transactions_and_serialized_sizes(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<(VerifiedTransaction, usize)>>> {
        let txns = self.try_multi_get_transaction_blocks(digests)?;
        txns.into_iter()
            .map(|txn| {
                txn.map(|txn| {
                    // Note: if the transaction is read from the db, we are wasting some
                    // effort relative to reading the raw bytes from the db instead of
                    // calling serialized_size. However, transactions should usually be
                    // fetched from cache.
                    match txn.serialized_size() {
                        Ok(size) => Ok(((*txn).clone(), size)),
                        Err(e) => Err(e),
                    }
                })
                .transpose()
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// Non-fallible version of `try_get_transactions_and_serialized_sizes`.
    fn get_transactions_and_serialized_sizes(
        &self,
        digests: &[TransactionDigest],
    ) -> Vec<Option<(VerifiedTransaction, usize)>> {
        self.try_get_transactions_and_serialized_sizes(digests)
            .expect("storage access failed")
    }

    fn try_multi_get_executed_effects_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffectsDigest>>>;

    /// Non-fallible version of `try_multi_get_executed_effects_digests`.
    fn multi_get_executed_effects_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> Vec<Option<TransactionEffectsDigest>> {
        self.try_multi_get_executed_effects_digests(digests)
            .expect("storage access failed")
    }

    fn try_is_tx_already_executed(&self, digest: &TransactionDigest) -> IotaResult<bool> {
        self.try_multi_get_executed_effects_digests(&[*digest])
            .map(|mut digests| {
                digests
                    .pop()
                    .expect("multi-get must return correct number of items")
                    .is_some()
            })
    }

    /// Non-fallible version of `try_is_tx_already_executed`.
    fn is_tx_already_executed(&self, digest: &TransactionDigest) -> bool {
        self.try_is_tx_already_executed(digest)
            .expect("storage access failed")
    }

    fn try_multi_get_executed_effects(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>> {
        let effects_digests = self.try_multi_get_executed_effects_digests(digests)?;
        assert_eq!(effects_digests.len(), digests.len());

        let mut results = vec![None; digests.len()];
        let mut fetch_digests = Vec::with_capacity(digests.len());
        let mut fetch_indices = Vec::with_capacity(digests.len());

        for (i, digest) in effects_digests.into_iter().enumerate() {
            if let Some(digest) = digest {
                fetch_digests.push(digest);
                fetch_indices.push(i);
            }
        }

        let effects = self.try_multi_get_effects(&fetch_digests)?;
        for (i, effects) in fetch_indices.into_iter().zip(effects.into_iter()) {
            results[i] = effects;
        }

        Ok(results)
    }

    /// Non-fallible version of `try_multi_get_executed_effects`.
    fn multi_get_executed_effects(
        &self,
        digests: &[TransactionDigest],
    ) -> Vec<Option<TransactionEffects>> {
        self.try_multi_get_executed_effects(digests)
            .expect("storage access failed")
    }

    fn try_get_executed_effects(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TransactionEffects>> {
        self.try_multi_get_executed_effects(&[*digest])
            .map(|mut effects| {
                effects
                    .pop()
                    .expect("multi-get must return correct number of items")
            })
    }

    /// Non-fallible version of `try_get_executed_effects`.
    fn get_executed_effects(&self, digest: &TransactionDigest) -> Option<TransactionEffects> {
        self.try_get_executed_effects(digest)
            .expect("storage access failed")
    }

    fn try_multi_get_effects(
        &self,
        digests: &[TransactionEffectsDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>>;

    /// Non-fallible version of `try_multi_get_effects`.
    fn multi_get_effects(
        &self,
        digests: &[TransactionEffectsDigest],
    ) -> Vec<Option<TransactionEffects>> {
        self.try_multi_get_effects(digests)
            .expect("storage access failed")
    }

    fn try_get_effects(
        &self,
        digest: &TransactionEffectsDigest,
    ) -> IotaResult<Option<TransactionEffects>> {
        self.try_multi_get_effects(&[*digest]).map(|mut effects| {
            effects
                .pop()
                .expect("multi-get must return correct number of items")
        })
    }

    /// Non-fallible version of `try_get_effects`.
    fn get_effects(&self, digest: &TransactionEffectsDigest) -> Option<TransactionEffects> {
        self.try_get_effects(digest).expect("storage access failed")
    }

    fn try_multi_get_events(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>>;

    /// Non-fallible version of `try_multi_get_events`.
    fn multi_get_events(&self, digests: &[TransactionDigest]) -> Vec<Option<TransactionEvents>> {
        self.try_multi_get_events(digests)
            .expect("storage access failed")
    }

    fn try_get_events(&self, digest: &TransactionDigest) -> IotaResult<Option<TransactionEvents>> {
        self.try_multi_get_events(&[*digest]).map(|mut events| {
            events
                .pop()
                .expect("multi-get must return correct number of items")
        })
    }

    /// Non-fallible version of `try_get_events`.
    fn get_events(&self, digest: &TransactionDigest) -> Option<TransactionEvents> {
        self.try_get_events(digest).expect("storage access failed")
    }

    fn try_notify_read_executed_effects_digests<'a>(
        &'a self,
        digests: &'a [TransactionDigest],
    ) -> BoxFuture<'a, IotaResult<Vec<TransactionEffectsDigest>>>;

    /// Non-fallible version of `try_notify_read_executed_effects_digests`.
    fn notify_read_executed_effects_digests<'a>(
        &'a self,
        digests: &'a [TransactionDigest],
    ) -> BoxFuture<'a, Vec<TransactionEffectsDigest>> {
        Box::pin(async move {
            self.try_notify_read_executed_effects_digests(digests)
                .await
                .expect("storage access failed")
        })
    }

    /// Wait until the effects of the given transactions are available and
    /// return them. WARNING: If calling this on a transaction that could be
    /// reverted, you must be sure that this function cannot be called
    /// during reconfiguration. The best way to do this is to wrap your
    /// future in EpochStore::within_alive_epoch. Holding an
    /// ExecutionLockReadGuard would also prevent reconfig from happening while
    /// waiting, but this is very dangerous, as it could prevent
    /// reconfiguration from ever occurring!
    fn try_notify_read_executed_effects<'a>(
        &'a self,
        digests: &'a [TransactionDigest],
    ) -> BoxFuture<'a, IotaResult<Vec<TransactionEffects>>> {
        async move {
            let digests = self
                .try_notify_read_executed_effects_digests(digests)
                .await?;
            // once digests are available, effects must be present as well
            self.try_multi_get_effects(&digests).map(|effects| {
                effects
                    .into_iter()
                    .map(|e| e.unwrap_or_else(|| fatal!("digests must exist")))
                    .collect()
            })
        }
        .boxed()
    }

    /// Non-fallible version of `try_notify_read_executed_effects`.
    fn notify_read_executed_effects<'a>(
        &'a self,
        digests: &'a [TransactionDigest],
    ) -> BoxFuture<'a, Vec<TransactionEffects>> {
        Box::pin(async move {
            self.try_notify_read_executed_effects(digests)
                .await
                .expect("storage access failed")
        })
    }
}

pub trait ExecutionCacheWrite: Send + Sync {
    /// Write the output of a transaction.
    ///
    /// Because of the child object consistency rule (readers that observe
    /// parents must observe all children of that parent, up to the parent's
    /// version bound), implementations of this method must not write any
    /// top-level (address-owned or shared) objects before they have written all
    /// of the object-owned objects (i.e. child objects) in the `objects` list.
    ///
    /// In the future, we may modify this method to expose finer-grained
    /// information about parent/child relationships. (This may be
    /// especially necessary for distributed object storage, but is unlikely
    /// to be an issue before we tackle that problem).
    ///
    /// This function may evict the mutable input objects (and successfully
    /// received objects) of transaction from the cache, since they cannot
    /// be read by any other transaction.
    ///
    /// Any write performed by this method immediately notifies any waiter that
    /// has previously called notify_read_objects_for_execution or
    /// notify_read_objects_for_signing for the object in question.
    fn try_write_transaction_outputs(
        &self,
        epoch_id: EpochId,
        tx_outputs: Arc<TransactionOutputs>,
    ) -> IotaResult;

    /// Non-fallible version of `try_write_transaction_outputs`.
    fn write_transaction_outputs(&self, epoch_id: EpochId, tx_outputs: Arc<TransactionOutputs>) {
        self.try_write_transaction_outputs(epoch_id, tx_outputs)
            .expect("storage access failed")
    }

    /// Attempt to acquire object locks for all of the owned input locks.
    fn try_acquire_transaction_locks(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        owned_input_objects: &[ObjectRef],
        transaction: VerifiedSignedTransaction,
    ) -> IotaResult;

    /// Non-fallible version of `try_acquire_transaction_locks`.
    fn acquire_transaction_locks(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        owned_input_objects: &[ObjectRef],
        transaction: VerifiedSignedTransaction,
    ) {
        self.try_acquire_transaction_locks(epoch_store, owned_input_objects, transaction)
            .expect("storage access failed")
    }
}

pub trait CheckpointCache: Send + Sync {
    // TODO: In addition to the methods below, this will eventually
    // include access to the CheckpointStore.

    // Note, the methods below were deemed deprecated before.
    // Currently, they are only used to implement `get_transaction_block`
    // for JSON RPC `ReadApi`.

    fn try_get_transaction_perpetual_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<(EpochId, CheckpointSequenceNumber)>>;

    /// Non-fallible version of `try_get_transaction_perpetual_checkpoint`.
    fn get_transaction_perpetual_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> Option<(EpochId, CheckpointSequenceNumber)> {
        self.try_get_transaction_perpetual_checkpoint(digest)
            .expect("storage access failed")
    }

    fn try_multi_get_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<(EpochId, CheckpointSequenceNumber)>>>;

    /// Non-fallible version of
    /// `try_multi_get_transactions_perpetual_checkpoints`.
    fn multi_get_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
    ) -> Vec<Option<(EpochId, CheckpointSequenceNumber)>> {
        self.try_multi_get_transactions_perpetual_checkpoints(digests)
            .expect("storage access failed")
    }

    fn try_insert_finalized_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
        epoch: EpochId,
        sequence: CheckpointSequenceNumber,
    ) -> IotaResult;

    /// Non-fallible version of
    /// `try_insert_finalized_transactions_perpetual_checkpoints`.
    fn insert_finalized_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
        epoch: EpochId,
        sequence: CheckpointSequenceNumber,
    ) {
        self.try_insert_finalized_transactions_perpetual_checkpoints(digests, epoch, sequence)
            .expect("storage access failed")
    }
}

pub trait ExecutionCacheReconfigAPI: Send + Sync {
    fn try_insert_genesis_object(&self, object: Object) -> IotaResult;

    /// Non-fallible version of `try_insert_genesis_object`.
    fn insert_genesis_object(&self, object: Object) {
        self.try_insert_genesis_object(object)
            .expect("storage access failed")
    }

    fn try_bulk_insert_genesis_objects(&self, objects: &[Object]) -> IotaResult;

    /// Non-fallible version of `try_bulk_insert_genesis_objects`.
    fn bulk_insert_genesis_objects(&self, objects: &[Object]) {
        self.try_bulk_insert_genesis_objects(objects)
            .expect("storage access failed")
    }

    fn try_revert_state_update(&self, digest: &TransactionDigest) -> IotaResult;

    /// Non-fallible version of `try_revert_state_update`.
    fn revert_state_update(&self, digest: &TransactionDigest) {
        self.try_revert_state_update(digest)
            .expect("storage access failed")
    }

    fn try_set_epoch_start_configuration(
        &self,
        epoch_start_config: &EpochStartConfiguration,
    ) -> IotaResult;

    /// Non-fallible version of `try_set_epoch_start_configuration`.
    fn set_epoch_start_configuration(&self, epoch_start_config: &EpochStartConfiguration) {
        self.try_set_epoch_start_configuration(epoch_start_config)
            .expect("storage access failed")
    }

    fn update_epoch_flags_metrics(&self, old: &[EpochFlag], new: &[EpochFlag]);

    fn clear_state_end_of_epoch(&self, execution_guard: &ExecutionLockWriteGuard<'_>);

    fn try_expensive_check_iota_conservation(
        &self,
        old_epoch_store: &AuthorityPerEpochStore,
        epoch_supply_change: Option<i64>,
    ) -> IotaResult;

    /// Non-fallible version of `try_expensive_check_iota_conservation`.
    fn expensive_check_iota_conservation(
        &self,
        old_epoch_store: &AuthorityPerEpochStore,
        epoch_supply_change: Option<i64>,
    ) {
        self.try_expensive_check_iota_conservation(old_epoch_store, epoch_supply_change)
            .expect("storage access failed")
    }

    fn try_checkpoint_db(&self, path: &Path) -> IotaResult;

    /// Non-fallible version of `try_checkpoint_db`.
    fn checkpoint_db(&self, path: &Path) {
        self.try_checkpoint_db(path).expect("storage access failed")
    }

    /// Reconfigure the cache itself.
    /// TODO: this is only needed for ProxyCache to switch between cache impls.
    /// It can be removed once WritebackCache is the sole cache impl.
    fn reconfigure_cache<'a>(
        &'a self,
        epoch_start_config: &'a EpochStartConfiguration,
    ) -> BoxFuture<'a, ()>;
}

// StateSyncAPI is for writing any data that was not the result of transaction
// execution, but that arrived via state sync. The fact that it came via state
// sync implies that it is certified output, and can be immediately persisted to
// the store.
pub trait StateSyncAPI: Send + Sync {
    fn try_insert_transaction_and_effects(
        &self,
        transaction: &VerifiedTransaction,
        transaction_effects: &TransactionEffects,
    ) -> IotaResult;

    /// Non-fallible version of `try_insert_transaction_and_effects`.
    fn insert_transaction_and_effects(
        &self,
        transaction: &VerifiedTransaction,
        transaction_effects: &TransactionEffects,
    ) {
        self.try_insert_transaction_and_effects(transaction, transaction_effects)
            .expect("storage access failed")
    }

    fn try_multi_insert_transaction_and_effects(
        &self,
        transactions_and_effects: &[VerifiedExecutionData],
    ) -> IotaResult;

    /// Non-fallible version of `try_multi_insert_transaction_and_effects`.
    fn multi_insert_transaction_and_effects(
        &self,
        transactions_and_effects: &[VerifiedExecutionData],
    ) {
        self.try_multi_insert_transaction_and_effects(transactions_and_effects)
            .expect("storage access failed");
    }
}

pub trait TestingAPI: Send + Sync {
    fn database_for_testing(&self) -> Arc<AuthorityStore>;
}

macro_rules! implement_storage_traits {
    ($implementor: ident) => {
        impl ObjectStore for $implementor {
            fn try_get_object(&self, object_id: &ObjectID) -> StorageResult<Option<Object>> {
                ObjectCacheRead::try_get_object(self, object_id).map_err(StorageError::custom)
            }

            fn try_get_object_by_key(
                &self,
                object_id: &ObjectID,
                version: iota_types::base_types::VersionNumber,
            ) -> StorageResult<Option<Object>> {
                ObjectCacheRead::try_get_object_by_key(self, object_id, version)
                    .map_err(StorageError::custom)
            }
        }

        impl ChildObjectResolver for $implementor {
            fn read_child_object(
                &self,
                parent: &ObjectID,
                child: &ObjectID,
                child_version_upper_bound: SequenceNumber,
            ) -> IotaResult<Option<Object>> {
                let Some(child_object) =
                    self.try_find_object_lt_or_eq_version(*child, child_version_upper_bound)?
                else {
                    return Ok(None);
                };

                let parent = *parent;
                if child_object.owner != Owner::ObjectOwner(parent.into()) {
                    return Err(IotaError::InvalidChildObjectAccess {
                        object: *child,
                        given_parent: parent,
                        actual_owner: child_object.owner,
                    });
                }
                Ok(Some(child_object))
            }

            fn get_object_received_at_version(
                &self,
                owner: &ObjectID,
                receiving_object_id: &ObjectID,
                receive_object_at_version: SequenceNumber,
                epoch_id: EpochId,
            ) -> IotaResult<Option<Object>> {
                let Some(recv_object) = ObjectCacheRead::try_get_object_by_key(
                    self,
                    receiving_object_id,
                    receive_object_at_version,
                )?
                else {
                    return Ok(None);
                };

                // Check for:
                // * Invalid access -- treat as the object does not exist. Or;
                // * If we've already received the object at the version -- then treat it as
                //   though it doesn't exist.
                // These two cases must remain indisguishable to the caller otherwise we risk
                // forks in transaction replay due to possible reordering of
                // transactions during replay.
                if recv_object.owner != Owner::AddressOwner((*owner).into())
                    || self.try_have_received_object_at_version(
                        receiving_object_id,
                        receive_object_at_version,
                        epoch_id,
                    )?
                {
                    return Ok(None);
                }

                Ok(Some(recv_object))
            }
        }

        impl BackingPackageStore for $implementor {
            fn get_package_object(
                &self,
                package_id: &ObjectID,
            ) -> IotaResult<Option<PackageObject>> {
                ObjectCacheRead::try_get_package_object(self, package_id)
            }
        }
    };
}

// Implement traits for a cache implementation that always go directly to the
// store.
macro_rules! implement_passthrough_traits {
    ($implementor: ident) => {
        impl CheckpointCache for $implementor {
            fn try_get_transaction_perpetual_checkpoint(
                &self,
                digest: &TransactionDigest,
            ) -> IotaResult<Option<(EpochId, CheckpointSequenceNumber)>> {
                self.store.get_transaction_perpetual_checkpoint(digest)
            }

            fn try_multi_get_transactions_perpetual_checkpoints(
                &self,
                digests: &[TransactionDigest],
            ) -> IotaResult<Vec<Option<(EpochId, CheckpointSequenceNumber)>>> {
                self.store
                    .multi_get_transactions_perpetual_checkpoints(digests)
            }

            fn try_insert_finalized_transactions_perpetual_checkpoints(
                &self,
                digests: &[TransactionDigest],
                epoch: EpochId,
                sequence: CheckpointSequenceNumber,
            ) -> IotaResult {
                self.store
                    .insert_finalized_transactions_perpetual_checkpoints(digests, epoch, sequence)
            }
        }

        impl ExecutionCacheReconfigAPI for $implementor {
            fn try_insert_genesis_object(&self, object: Object) -> IotaResult {
                self.insert_genesis_object_impl(object)
            }

            fn try_bulk_insert_genesis_objects(&self, objects: &[Object]) -> IotaResult {
                self.bulk_insert_genesis_objects_impl(objects)
            }

            fn try_revert_state_update(&self, digest: &TransactionDigest) -> IotaResult {
                self.revert_state_update_impl(digest)
            }

            fn try_set_epoch_start_configuration(
                &self,
                epoch_start_config: &EpochStartConfiguration,
            ) -> IotaResult {
                self.store.set_epoch_start_configuration(epoch_start_config)
            }

            fn update_epoch_flags_metrics(&self, old: &[EpochFlag], new: &[EpochFlag]) {
                self.store.update_epoch_flags_metrics(old, new)
            }

            fn clear_state_end_of_epoch(&self, execution_guard: &ExecutionLockWriteGuard<'_>) {
                self.clear_state_end_of_epoch_impl(execution_guard)
            }

            fn try_expensive_check_iota_conservation(
                &self,
                old_epoch_store: &AuthorityPerEpochStore,
                epoch_supply_change: Option<i64>,
            ) -> IotaResult {
                self.store.expensive_check_iota_conservation(
                    self,
                    old_epoch_store,
                    epoch_supply_change,
                )
            }

            fn try_checkpoint_db(&self, path: &std::path::Path) -> IotaResult {
                self.store.perpetual_tables.checkpoint_db(path)
            }

            fn reconfigure_cache<'a>(
                &'a self,
                _: &'a EpochStartConfiguration,
            ) -> BoxFuture<'a, ()> {
                // Since we now use WritebackCache directly at startup (if the epoch flag is
                // set), this can be called at reconfiguration time. It is a no-op.
                // TODO: remove this once we completely remove ProxyCache.
                std::future::ready(()).boxed()
            }
        }

        impl TestingAPI for $implementor {
            fn database_for_testing(&self) -> Arc<AuthorityStore> {
                self.store.clone()
            }
        }
    };
}

use implement_passthrough_traits;

implement_storage_traits!(PassthroughCache);
implement_storage_traits!(WritebackCache);
implement_storage_traits!(ProxyCache);

pub trait ExecutionCacheAPI:
    ObjectCacheRead
    + ExecutionCacheWrite
    + ExecutionCacheCommit
    + ExecutionCacheReconfigAPI
    + CheckpointCache
    + StateSyncAPI
{
}
