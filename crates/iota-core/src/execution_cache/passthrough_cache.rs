// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::{FutureExt, future::BoxFuture};
use iota_common::sync::notify_read::NotifyRead;
use iota_storage::package_object_cache::PackageObjectCache;
use iota_types::{
    accumulator::Accumulator,
    base_types::{EpochId, ObjectID, ObjectRef, SequenceNumber, VerifiedExecutionData},
    digests::{TransactionDigest, TransactionEffectsDigest},
    effects::{TransactionEffects, TransactionEvents},
    error::{IotaError, IotaResult},
    executable_transaction::VerifiedExecutableTransaction,
    iota_system_state::{IotaSystemState, get_iota_system_state},
    message_envelope::Message,
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
    storage::{MarkerValue, ObjectKey, ObjectOrTombstone, ObjectStore, PackageObject},
    transaction::{VerifiedSignedTransaction, VerifiedTransaction},
};
use prometheus::Registry;
use tap::TapFallible;
use tracing::instrument;
use typed_store::Map;

use super::{
    Batch, CheckpointCache, ExecutionCacheCommit, ExecutionCacheMetrics, ExecutionCacheReconfigAPI,
    ExecutionCacheWrite, ObjectCacheRead, StateSyncAPI, TestingAPI, TransactionCacheRead,
    implement_passthrough_traits,
};
use crate::{
    authority::{
        AuthorityStore,
        authority_per_epoch_store::AuthorityPerEpochStore,
        authority_store::{ExecutionLockWriteGuard, IotaLockResult},
        epoch_start_configuration::{EpochFlag, EpochStartConfiguration},
    },
    state_accumulator::AccumulatorStore,
    transaction_outputs::TransactionOutputs,
};

pub struct PassthroughCache {
    store: Arc<AuthorityStore>,
    metrics: Arc<ExecutionCacheMetrics>,
    package_cache: Arc<PackageObjectCache>,
    executed_effects_digests_notify_read: NotifyRead<TransactionDigest, TransactionEffectsDigest>,
}

impl PassthroughCache {
    pub fn new(store: Arc<AuthorityStore>, metrics: Arc<ExecutionCacheMetrics>) -> Self {
        Self {
            store,
            metrics,
            package_cache: PackageObjectCache::new(),
            executed_effects_digests_notify_read: NotifyRead::new(),
        }
    }

    pub fn new_for_tests(store: Arc<AuthorityStore>, registry: &Registry) -> Self {
        let metrics = Arc::new(ExecutionCacheMetrics::new(registry));
        Self::new(store, metrics)
    }

    pub fn store_for_testing(&self) -> &Arc<AuthorityStore> {
        &self.store
    }

    fn revert_state_update_impl(&self, digest: &TransactionDigest) -> IotaResult {
        self.store.revert_state_update(digest)
    }

    fn clear_state_end_of_epoch_impl(&self, execution_guard: &ExecutionLockWriteGuard) {
        self.store
            .clear_object_per_epoch_marker_table(execution_guard)
            .tap_err(|e| {
                tracing::error!(?e, "Failed to clear object per-epoch marker table");
            })
            .ok();
    }

    fn bulk_insert_genesis_objects_impl(&self, objects: &[Object]) -> IotaResult {
        self.store.bulk_insert_genesis_objects(objects)
    }

    fn insert_genesis_object_impl(&self, object: Object) -> IotaResult {
        self.store.insert_genesis_object(object)
    }
}

impl ObjectCacheRead for PassthroughCache {
    #[instrument(level = "trace", skip_all, fields(package_id))]
    fn try_get_package_object(&self, package_id: &ObjectID) -> IotaResult<Option<PackageObject>> {
        self.package_cache
            .get_package_object(package_id, &*self.store)
    }

    fn force_reload_system_packages(&self, system_package_ids: &[ObjectID]) {
        self.package_cache
            .force_reload_system_packages(system_package_ids.iter().cloned(), self);
    }

    #[instrument(level = "trace", skip_all, fields(object_id = ?id))]
    fn try_get_object(&self, id: &ObjectID) -> IotaResult<Option<Object>> {
        self.store.try_get_object(id).map_err(Into::into)
    }

    #[instrument(level = "trace", skip_all, fields(object_id, version))]
    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        Ok(self.store.try_get_object_by_key(object_id, version)?)
    }

    #[instrument(level = "trace", skip_all)]
    fn try_multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Result<Vec<Option<Object>>, IotaError> {
        Ok(self.store.try_multi_get_objects_by_key(object_keys)?)
    }

    #[instrument(level = "trace", skip_all, fields(object_id, version))]
    fn try_object_exists_by_key(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<bool> {
        self.store.object_exists_by_key(object_id, version)
    }

    #[instrument(level = "trace", skip_all)]
    fn try_multi_object_exists_by_key(&self, object_keys: &[ObjectKey]) -> IotaResult<Vec<bool>> {
        self.store.multi_object_exists_by_key(object_keys)
    }

    #[instrument(level = "trace", skip_all, fields(object_id))]
    fn try_get_latest_object_ref_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<ObjectRef>> {
        self.store.get_latest_object_ref_or_tombstone(object_id)
    }

    #[instrument(level = "trace", skip_all, fields(object_id))]
    fn try_get_latest_object_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> Result<Option<(ObjectKey, ObjectOrTombstone)>, IotaError> {
        self.store.get_latest_object_or_tombstone(object_id)
    }

    #[instrument(level = "trace", skip_all, fields(object_id, version_bound))]
    fn try_find_object_lt_or_eq_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        self.store.find_object_lt_or_eq_version(object_id, version)
    }

    fn try_get_lock(
        &self,
        obj_ref: ObjectRef,
        epoch_store: &AuthorityPerEpochStore,
    ) -> IotaLockResult {
        self.store.get_lock(obj_ref, epoch_store)
    }

    fn _try_get_live_objref(&self, object_id: ObjectID) -> IotaResult<ObjectRef> {
        self.store.get_latest_live_version_for_object_id(object_id)
    }

    fn try_check_owned_objects_are_live(&self, owned_object_refs: &[ObjectRef]) -> IotaResult {
        self.store.check_owned_objects_are_live(owned_object_refs)
    }

    fn try_get_iota_system_state_object_unsafe(&self) -> IotaResult<IotaSystemState> {
        get_iota_system_state(self)
    }

    fn try_get_marker_value(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<MarkerValue>> {
        self.store.get_marker_value(object_id, &version, epoch_id)
    }

    fn try_get_latest_marker(
        &self,
        object_id: &ObjectID,
        epoch_id: EpochId,
    ) -> IotaResult<Option<(SequenceNumber, MarkerValue)>> {
        self.store.get_latest_marker(object_id, epoch_id)
    }

    fn try_get_highest_pruned_checkpoint(&self) -> IotaResult<Option<CheckpointSequenceNumber>> {
        self.store
            .perpetual_tables
            .get_highest_pruned_checkpoint()
            .map_err(IotaError::from)
    }
}

impl TransactionCacheRead for PassthroughCache {
    #[instrument(level = "trace", skip_all)]
    fn try_multi_get_transaction_blocks(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<Arc<VerifiedTransaction>>>> {
        Ok(self
            .store
            .multi_get_transaction_blocks(digests)?
            .into_iter()
            .map(|o| o.map(Arc::new))
            .collect())
    }

    #[instrument(level = "trace", skip_all)]
    fn try_multi_get_executed_effects_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffectsDigest>>> {
        self.store
            .multi_get_executed_effects_digests(digests)
            .map_err(IotaError::from)
    }

    #[instrument(level = "trace", skip_all)]
    fn try_multi_get_effects(
        &self,
        digests: &[TransactionEffectsDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>> {
        Ok(self.store.perpetual_tables.effects.multi_get(digests)?)
    }

    #[instrument(level = "trace", skip_all)]
    fn try_notify_read_executed_effects_digests<'a>(
        &'a self,
        digests: &'a [TransactionDigest],
    ) -> BoxFuture<'a, IotaResult<Vec<TransactionEffectsDigest>>> {
        self.executed_effects_digests_notify_read
            .read(digests, |digests| {
                self.try_multi_get_executed_effects_digests(digests)
            })
            .boxed()
    }

    #[instrument(level = "trace", skip_all)]
    fn try_multi_get_events(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>> {
        self.store.multi_get_events(tx_digests)
    }
}

impl ExecutionCacheWrite for PassthroughCache {
    #[instrument(level = "debug", skip_all)]
    fn try_write_transaction_outputs(
        &self,
        epoch_id: EpochId,
        tx_outputs: Arc<TransactionOutputs>,
    ) -> IotaResult {
        let tx_digest = *tx_outputs.transaction.digest();
        let effects_digest = tx_outputs.effects.digest();

        // NOTE: We just check here that live markers exist, not that they are locked to
        // a specific TX. Why?
        // 1. Live markers existence prevents re-execution of old certs when objects
        //    have been upgraded
        // 2. Not all validators lock, just 2f+1, so transaction should proceed
        //    regardless (But the live markers should exist which means previous
        //    transactions finished)
        // 3. Equivocation possible (different TX) but as long as 2f+1 approves current
        //    TX its fine
        // 4. Live markers may have existed when we started processing this tx, but
        //    could have since been deleted by a concurrent tx that finished first. In
        //    that case, check if the tx effects exist.
        self.store
            .check_owned_objects_are_live(&tx_outputs.live_object_markers_to_delete)?;

        let batch = self.store.build_db_batch(epoch_id, &[tx_outputs])?;
        batch.write()?;

        self.executed_effects_digests_notify_read
            .notify(&tx_digest, &effects_digest);

        self.metrics
            .pending_notify_read
            .set(self.executed_effects_digests_notify_read.num_pending() as i64);

        Ok(())
    }

    fn try_acquire_transaction_locks<'a>(
        &'a self,
        epoch_store: &'a AuthorityPerEpochStore,
        owned_input_objects: &'a [ObjectRef],
        transaction: VerifiedSignedTransaction,
    ) -> IotaResult {
        self.store
            .acquire_transaction_locks(epoch_store, owned_input_objects, transaction)
    }
}

impl AccumulatorStore for PassthroughCache {
    fn get_root_state_accumulator_for_epoch(
        &self,
        epoch: EpochId,
    ) -> IotaResult<Option<(CheckpointSequenceNumber, Accumulator)>> {
        self.store.get_root_state_accumulator_for_epoch(epoch)
    }

    fn get_root_state_accumulator_for_highest_epoch(
        &self,
    ) -> IotaResult<Option<(EpochId, (CheckpointSequenceNumber, Accumulator))>> {
        self.store.get_root_state_accumulator_for_highest_epoch()
    }

    fn insert_state_accumulator_for_epoch(
        &self,
        epoch: EpochId,
        checkpoint_seq_num: &CheckpointSequenceNumber,
        acc: &Accumulator,
    ) -> IotaResult {
        self.store
            .insert_state_accumulator_for_epoch(epoch, checkpoint_seq_num, acc)
    }

    fn iter_live_object_set(
        &self,
    ) -> Box<dyn Iterator<Item = crate::authority::authority_store_tables::LiveObject> + '_> {
        self.store.iter_live_object_set()
    }
}

impl ExecutionCacheCommit for PassthroughCache {
    fn build_db_batch(&self, _epoch: EpochId, _digests: &[TransactionDigest]) -> Batch {
        // Nothing needs to be done since they were already committed in
        // write_transaction_outputs
        (vec![], self.store.perpetual_tables.transactions.batch())
    }

    fn try_commit_transaction_outputs(
        &self,
        _epoch: EpochId,
        _batch: Batch,
        _digests: &[TransactionDigest],
    ) -> IotaResult {
        // Nothing needs to be done since they were already committed in
        // write_transaction_outputs
        Ok(())
    }

    fn try_persist_transaction(&self, _tx: &VerifiedExecutableTransaction) -> IotaResult {
        // Nothing needs to be done since they were already committed in
        // write_transaction_outputs
        Ok(())
    }

    fn approximate_pending_transaction_count(&self) -> u64 {
        0
    }
}

impl StateSyncAPI for PassthroughCache {
    fn try_insert_transaction_and_effects(
        &self,
        transaction: &VerifiedTransaction,
        transaction_effects: &TransactionEffects,
    ) -> IotaResult {
        self.store
            .insert_transaction_and_effects(transaction, transaction_effects)
            .map_err(IotaError::from)
    }

    fn try_multi_insert_transaction_and_effects(
        &self,
        transactions_and_effects: &[VerifiedExecutionData],
    ) -> IotaResult {
        self.store
            .multi_insert_transaction_and_effects(transactions_and_effects.iter())
            .map_err(IotaError::from)
    }
}

implement_passthrough_traits!(PassthroughCache);
