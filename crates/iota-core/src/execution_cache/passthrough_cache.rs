// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

<<<<<<< HEAD
use either::Either;
use futures::{
    future::{join_all, BoxFuture},
    FutureExt,
};
use iota_common::sync::notify_read::NotifyRead;
use iota_config::node::AuthorityStorePruningConfig;
=======
use futures::{future::BoxFuture, FutureExt};
use iota_common::sync::notify_read::NotifyRead;
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
use iota_protocol_config::ProtocolVersion;
use iota_storage::package_object_cache::PackageObjectCache;
use iota_types::{
    accumulator::Accumulator,
    base_types::{EpochId, ObjectID, ObjectRef, SequenceNumber, VerifiedExecutionData},
<<<<<<< HEAD
=======
    bridge::{get_bridge, Bridge},
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
    digests::{TransactionDigest, TransactionEffectsDigest, TransactionEventsDigest},
    effects::{TransactionEffects, TransactionEvents},
    error::{IotaError, IotaResult},
    iota_system_state::{get_iota_system_state, IotaSystemState},
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
    implement_passthrough_traits, CheckpointCache, ExecutionCacheCommit, ExecutionCacheMetrics,
    ExecutionCacheReconfigAPI, ExecutionCacheWrite, ObjectCacheRead, StateSyncAPI, TestingAPI,
    TransactionCacheRead,
};
use crate::{
    authority::{
        authority_per_epoch_store::AuthorityPerEpochStore,
        authority_store::{ExecutionLockWriteGuard, IotaLockResult},
        epoch_start_configuration::{EpochFlag, EpochStartConfiguration},
        AuthorityStore,
    },
    state_accumulator::AccumulatorStore,
    transaction_outputs::TransactionOutputs,
};
use crate::{
    authority::{
        authority_per_epoch_store::AuthorityPerEpochStore,
        authority_store::{ExecutionLockWriteGuard, IotaLockResult},
        authority_store_pruner::{
            AuthorityStorePruner, AuthorityStorePruningMetrics, EPOCH_DURATION_MS_FOR_TESTING,
        },
        epoch_start_configuration::{EpochFlag, EpochStartConfiguration},
        AuthorityStore,
    },
    checkpoints::CheckpointStore,
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

<<<<<<< HEAD
    pub async fn prune_objects_and_compact_for_testing(
        &self,
        checkpoint_store: &Arc<CheckpointStore>,
    ) {
        let pruning_config = AuthorityStorePruningConfig {
            num_epochs_to_retain: 0,
            ..Default::default()
        };
        let _ = AuthorityStorePruner::prune_objects_for_eligible_epochs(
            &self.store.perpetual_tables,
            checkpoint_store,
            &self.store.objects_lock_table,
            pruning_config,
            AuthorityStorePruningMetrics::new_for_test(),
            usize::MAX,
            EPOCH_DURATION_MS_FOR_TESTING,
        )
        .await;
        let _ = AuthorityStorePruner::compact(&self.store.perpetual_tables);
    }

=======
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
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
}

<<<<<<< HEAD
impl ExecutionCacheRead for PassthroughCache {
=======
impl ObjectCacheRead for PassthroughCache {
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
    fn get_package_object(&self, package_id: &ObjectID) -> IotaResult<Option<PackageObject>> {
        self.package_cache
            .get_package_object(package_id, &*self.store)
    }

    fn force_reload_system_packages(&self, system_package_ids: &[ObjectID]) {
        self.package_cache
            .force_reload_system_packages(system_package_ids.iter().cloned(), self);
    }

    fn get_object(&self, id: &ObjectID) -> IotaResult<Option<Object>> {
        self.store.get_object(id).map_err(Into::into)
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        Ok(self.store.get_object_by_key(object_id, version)?)
    }

    fn multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Result<Vec<Option<Object>>, IotaError> {
        Ok(self.store.multi_get_objects_by_key(object_keys)?)
    }

    fn object_exists_by_key(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<bool> {
        self.store.object_exists_by_key(object_id, version)
    }

    fn multi_object_exists_by_key(&self, object_keys: &[ObjectKey]) -> IotaResult<Vec<bool>> {
        self.store.multi_object_exists_by_key(object_keys)
    }

    fn get_latest_object_ref_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> IotaResult<Option<ObjectRef>> {
        self.store.get_latest_object_ref_or_tombstone(object_id)
    }

    fn get_latest_object_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> Result<Option<(ObjectKey, ObjectOrTombstone)>, IotaError> {
        self.store.get_latest_object_or_tombstone(object_id)
    }

    fn find_object_lt_or_eq_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        self.store.find_object_lt_or_eq_version(object_id, version)
    }

    fn get_lock(&self, obj_ref: ObjectRef, epoch_store: &AuthorityPerEpochStore) -> IotaLockResult {
        self.store.get_lock(obj_ref, epoch_store)
    }

<<<<<<< HEAD
    fn _get_latest_lock_for_object_id(&self, object_id: ObjectID) -> IotaResult<ObjectRef> {
        self.store.get_latest_live_version_for_object_id(object_id)
    }

    fn check_owned_object_locks_exist(&self, owned_object_refs: &[ObjectRef]) -> IotaResult {
        self.store.check_owned_object_locks_exist(owned_object_refs)
=======
    fn _get_live_objref(&self, object_id: ObjectID) -> IotaResult<ObjectRef> {
        self.store.get_latest_live_version_for_object_id(object_id)
    }

    fn check_owned_objects_are_live(&self, owned_object_refs: &[ObjectRef]) -> IotaResult {
        self.store.check_owned_objects_are_live(owned_object_refs)
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
    }

    fn get_iota_system_state_object_unsafe(&self) -> IotaResult<IotaSystemState> {
        get_iota_system_state(self)
    }

    fn get_bridge_object_unsafe(&self) -> IotaResult<Bridge> {
        get_bridge(self)
    }

    fn get_marker_value(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<MarkerValue>> {
        self.store.get_marker_value(object_id, &version, epoch_id)
    }

    fn get_latest_marker(
        &self,
        object_id: &ObjectID,
        epoch_id: EpochId,
    ) -> IotaResult<Option<(SequenceNumber, MarkerValue)>> {
        self.store.get_latest_marker(object_id, epoch_id)
    }

    fn get_highest_pruned_checkpoint(&self) -> IotaResult<CheckpointSequenceNumber> {
        self.store.perpetual_tables.get_highest_pruned_checkpoint()
    }
}

impl TransactionCacheRead for PassthroughCache {
    fn multi_get_transaction_blocks(
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

    fn multi_get_executed_effects_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEffectsDigest>>> {
        self.store.multi_get_executed_effects_digests(digests)
    }

    fn multi_get_effects(
        &self,
        digests: &[TransactionEffectsDigest],
    ) -> IotaResult<Vec<Option<TransactionEffects>>> {
        Ok(self.store.perpetual_tables.effects.multi_get(digests)?)
    }

    fn notify_read_executed_effects_digests<'a>(
        &'a self,
        digests: &'a [TransactionDigest],
    ) -> BoxFuture<'a, IotaResult<Vec<TransactionEffectsDigest>>> {
<<<<<<< HEAD
        async move {
            let registrations = self
                .executed_effects_digests_notify_read
                .register_all(digests);

            let executed_effects_digests = self.multi_get_executed_effects_digests(digests)?;

            let results = executed_effects_digests
                .into_iter()
                .zip(registrations)
                .map(|(a, r)| match a {
                    // Note that Some() clause also drops registration that is already fulfilled
                    Some(ready) => Either::Left(futures::future::ready(ready)),
                    None => Either::Right(r),
                });

            Ok(join_all(results).await)
        }
        .boxed()
=======
        self.executed_effects_digests_notify_read
            .read(digests, |digests| {
                self.multi_get_executed_effects_digests(digests)
            })
            .boxed()
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
    }

    fn multi_get_events(
        &self,
        event_digests: &[TransactionEventsDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>> {
        self.store.multi_get_events(event_digests)
    }
<<<<<<< HEAD

    fn get_iota_system_state_object_unsafe(&self) -> IotaResult<IotaSystemState> {
        get_iota_system_state(self)
    }

    fn get_marker_value(
        &self,
        object_id: &ObjectID,
        version: SequenceNumber,
        epoch_id: EpochId,
    ) -> IotaResult<Option<MarkerValue>> {
        self.store.get_marker_value(object_id, &version, epoch_id)
    }

    fn get_latest_marker(
        &self,
        object_id: &ObjectID,
        epoch_id: EpochId,
    ) -> IotaResult<Option<(SequenceNumber, MarkerValue)>> {
        self.store.get_latest_marker(object_id, epoch_id)
    }
=======
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
}

impl ExecutionCacheWrite for PassthroughCache {
    #[instrument(level = "debug", skip_all)]
    fn write_transaction_outputs<'a>(
        &'a self,
        epoch_id: EpochId,
        tx_outputs: Arc<TransactionOutputs>,
    ) -> BoxFuture<'a, IotaResult> {
        async move {
            let tx_digest = *tx_outputs.transaction.digest();
            let effects_digest = tx_outputs.effects.digest();

            // NOTE: We just check here that locks exist, not that they are locked to a
            // specific TX. Why?
            // 1. Lock existence prevents re-execution of old certs when objects have been
            //    upgraded
            // 2. Not all validators lock, just 2f+1, so transaction should proceed
            //    regardless (But the lock should exist which means previous transactions
            //    finished)
            // 3. Equivocation possible (different TX) but as long as 2f+1 approves current
            //    TX its fine
            // 4. Locks may have existed when we started processing this tx, but could have
            //    since been deleted by a concurrent tx that finished first. In that case,
            //    check if the tx effects exist.
            self.store
                .check_owned_objects_are_live(&tx_outputs.locks_to_delete)?;

            self.store
                .write_transaction_outputs(epoch_id, &[tx_outputs])
                .await?;

            self.executed_effects_digests_notify_read
                .notify(&tx_digest, &effects_digest);

            self.metrics
                .pending_notify_read
                .set(self.executed_effects_digests_notify_read.num_pending() as i64);

            Ok(())
        }
        .boxed()
    }

    fn acquire_transaction_locks<'a>(
        &'a self,
        epoch_store: &'a AuthorityPerEpochStore,
        owned_input_objects: &'a [ObjectRef],
        transaction: VerifiedSignedTransaction,
    ) -> BoxFuture<'a, IotaResult> {
        self.store
            .acquire_transaction_locks(epoch_store, owned_input_objects, transaction)
            .boxed()
    }
}

impl AccumulatorStore for PassthroughCache {
    fn get_object_ref_prior_to_key_deprecated(
        &self,
        object_id: &ObjectID,
        version: iota_types::base_types::VersionNumber,
    ) -> IotaResult<Option<ObjectRef>> {
        self.store
            .get_object_ref_prior_to_key_deprecated(object_id, version)
    }

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
        include_wrapped_tombstone: bool,
    ) -> Box<dyn Iterator<Item = crate::authority::authority_store_tables::LiveObject> + '_> {
        self.store.iter_live_object_set(include_wrapped_tombstone)
    }
}

impl ExecutionCacheCommit for PassthroughCache {
    fn commit_transaction_outputs<'a>(
        &'a self,
        _epoch: EpochId,
<<<<<<< HEAD
        _digest: &TransactionDigest,
    ) -> BoxFuture<'_, IotaResult> {
=======
        _digests: &'a [TransactionDigest],
    ) -> BoxFuture<'a, IotaResult> {
        // Nothing needs to be done since they were already committed in
        // write_transaction_outputs
        async { Ok(()) }.boxed()
    }

    fn persist_transactions(&self, _digests: &[TransactionDigest]) -> BoxFuture<'_, IotaResult> {
>>>>>>> origin/slipstream/mainnet-v1.32.2/crates/iota-core
        // Nothing needs to be done since they were already committed in
        // write_transaction_outputs
        async { Ok(()) }.boxed()
    }
}

implement_passthrough_traits!(PassthroughCache);
