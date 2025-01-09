// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use fastcrypto::hash::MultisetHash;
use iota_metrics::monitored_scope;
use iota_types::{
    accumulator::Accumulator,
    base_types::{ObjectID, SequenceNumber},
    committee::EpochId,
    digests::ObjectDigest,
    effects::{TransactionEffects, TransactionEffectsAPI},
    error::IotaResult,
    in_memory_storage::InMemoryStorage,
    messages_checkpoint::{CheckpointSequenceNumber, ECMHLiveObjectSetDigest},
    storage::ObjectStore,
};
use prometheus::{IntGauge, Registry, register_int_gauge_with_registry};
use serde::Serialize;
use tracing::debug;

use crate::authority::{
    authority_per_epoch_store::AuthorityPerEpochStore, authority_store_tables::LiveObject,
};

pub struct StateAccumulatorMetrics {
    inconsistent_state: IntGauge,
}

impl StateAccumulatorMetrics {
    pub fn new(registry: &Registry) -> Arc<Self> {
        let this = Self {
            inconsistent_state: register_int_gauge_with_registry!(
                "accumulator_inconsistent_state",
                "1 if accumulated live object set differs from StateAccumulator root state hash for the previous epoch",
                registry
            )
            .unwrap(),
        };
        Arc::new(this)
    }
}

pub enum StateAccumulator {
    V1(StateAccumulatorV1),
}

pub struct StateAccumulatorV1 {
    store: Arc<dyn AccumulatorStore>,
    metrics: Arc<StateAccumulatorMetrics>,
}

pub trait AccumulatorStore: ObjectStore + Send + Sync {
    fn get_root_state_accumulator_for_epoch(
        &self,
        epoch: EpochId,
    ) -> IotaResult<Option<(CheckpointSequenceNumber, Accumulator)>>;

    fn get_root_state_accumulator_for_highest_epoch(
        &self,
    ) -> IotaResult<Option<(EpochId, (CheckpointSequenceNumber, Accumulator))>>;

    fn insert_state_accumulator_for_epoch(
        &self,
        epoch: EpochId,
        checkpoint_seq_num: &CheckpointSequenceNumber,
        acc: &Accumulator,
    ) -> IotaResult;

    fn iter_live_object_set(&self) -> Box<dyn Iterator<Item = LiveObject> + '_>;

    fn iter_cached_live_object_set_for_testing(&self) -> Box<dyn Iterator<Item = LiveObject> + '_> {
        self.iter_live_object_set()
    }
}

impl AccumulatorStore for InMemoryStorage {
    fn get_root_state_accumulator_for_epoch(
        &self,
        _epoch: EpochId,
    ) -> IotaResult<Option<(CheckpointSequenceNumber, Accumulator)>> {
        unreachable!("not used for testing")
    }

    fn get_root_state_accumulator_for_highest_epoch(
        &self,
    ) -> IotaResult<Option<(EpochId, (CheckpointSequenceNumber, Accumulator))>> {
        unreachable!("not used for testing")
    }

    fn insert_state_accumulator_for_epoch(
        &self,
        _epoch: EpochId,
        _checkpoint_seq_num: &CheckpointSequenceNumber,
        _acc: &Accumulator,
    ) -> IotaResult {
        unreachable!("not used for testing")
    }

    fn iter_live_object_set(&self) -> Box<dyn Iterator<Item = LiveObject> + '_> {
        unreachable!("not used for testing")
    }
}

/// Serializable representation of the ObjectRef of an
/// object that has been wrapped
/// TODO: This can be replaced with ObjectKey.
#[derive(Serialize, Debug)]
pub struct WrappedObject {
    id: ObjectID,
    wrapped_at: SequenceNumber,
    digest: ObjectDigest,
}

impl WrappedObject {
    pub fn new(id: ObjectID, wrapped_at: SequenceNumber) -> Self {
        Self {
            id,
            wrapped_at,
            digest: ObjectDigest::OBJECT_DIGEST_WRAPPED,
        }
    }
}

fn accumulate_effects(effects: Vec<TransactionEffects>) -> Accumulator {
    let mut acc = Accumulator::default();

    // process insertions to the set
    acc.insert_all(
        effects
            .iter()
            .flat_map(|fx| {
                fx.all_changed_objects()
                    .into_iter()
                    .map(|(object_ref, _, _)| object_ref.2)
            })
            .collect::<Vec<ObjectDigest>>(),
    );

    // process modified objects to the set
    acc.remove_all(
        effects
            .iter()
            .flat_map(|fx| {
                fx.old_object_metadata()
                    .into_iter()
                    .map(|(object_ref, _owner)| object_ref.2)
            })
            .collect::<Vec<ObjectDigest>>(),
    );

    acc
}

impl StateAccumulator {
    pub fn new(store: Arc<dyn AccumulatorStore>, metrics: Arc<StateAccumulatorMetrics>) -> Self {
        StateAccumulator::V1(StateAccumulatorV1::new(store, metrics))
    }

    pub fn new_for_tests(store: Arc<dyn AccumulatorStore>) -> Self {
        Self::new(store, StateAccumulatorMetrics::new(&Registry::new()))
    }

    pub fn metrics(&self) -> Arc<StateAccumulatorMetrics> {
        match self {
            StateAccumulator::V1(impl_v1) => impl_v1.metrics.clone(),
        }
    }

    pub fn set_inconsistent_state(&self, is_inconsistent_state: bool) {
        match self {
            StateAccumulator::V1(impl_v1) => &impl_v1.metrics,
        }
        .inconsistent_state
        .set(is_inconsistent_state as i64);
    }

    /// Accumulates the effects of a single checkpoint and persists the
    /// accumulator.
    pub fn accumulate_checkpoint(
        &self,
        effects: Vec<TransactionEffects>,
        checkpoint_seq_num: CheckpointSequenceNumber,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<Accumulator> {
        let _scope = monitored_scope("AccumulateCheckpoint");
        if let Some(acc) = epoch_store.get_state_hash_for_checkpoint(&checkpoint_seq_num)? {
            return Ok(acc);
        }

        let acc = self.accumulate_effects(effects);

        epoch_store.insert_state_hash_for_checkpoint(&checkpoint_seq_num, &acc)?;
        debug!("Accumulated checkpoint {}", checkpoint_seq_num);

        epoch_store
            .checkpoint_state_notify_read
            .notify(&checkpoint_seq_num, &acc);

        Ok(acc)
    }

    pub async fn accumulate_running_root(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        checkpoint_seq_num: CheckpointSequenceNumber,
        checkpoint_acc: Option<Accumulator>,
    ) -> IotaResult {
        match self {
            StateAccumulator::V1(impl_v1) => {
                impl_v1
                    .accumulate_running_root(epoch_store, checkpoint_seq_num, checkpoint_acc)
                    .await
            }
        }
    }

    pub async fn accumulate_epoch(
        &self,
        epoch_store: Arc<AuthorityPerEpochStore>,
        last_checkpoint_of_epoch: CheckpointSequenceNumber,
    ) -> IotaResult<Accumulator> {
        match self {
            StateAccumulator::V1(impl_v1) => {
                impl_v1.accumulate_epoch(epoch_store, last_checkpoint_of_epoch)
            }
        }
    }

    pub fn accumulate_cached_live_object_set_for_testing(&self) -> Accumulator {
        match self {
            StateAccumulator::V1(impl_v1) => Self::accumulate_live_object_set_impl(
                impl_v1.store.iter_cached_live_object_set_for_testing(),
            ),
        }
    }

    /// Returns the result of accumulating the live object set, without side
    /// effects
    pub fn accumulate_live_object_set(&self) -> Accumulator {
        match self {
            StateAccumulator::V1(impl_v1) => {
                Self::accumulate_live_object_set_impl(impl_v1.store.iter_live_object_set())
            }
        }
    }

    /// Accumulates given effects and returns the accumulator without side
    /// effects.
    pub fn accumulate_effects(&self, effects: Vec<TransactionEffects>) -> Accumulator {
        match self {
            StateAccumulator::V1(impl_v1) => impl_v1.accumulate_effects(effects),
        }
    }

    fn accumulate_live_object_set_impl(iter: impl Iterator<Item = LiveObject>) -> Accumulator {
        let mut acc = Accumulator::default();
        iter.for_each(|live_object| {
            Self::accumulate_live_object(&mut acc, &live_object);
        });
        acc
    }

    pub fn accumulate_live_object(acc: &mut Accumulator, live_object: &LiveObject) {
        match live_object {
            LiveObject::Normal(object) => {
                acc.insert(object.compute_object_reference().2);
            }
            LiveObject::Wrapped(key) => {
                acc.insert(
                    bcs::to_bytes(&WrappedObject::new(key.0, key.1))
                        .expect("Failed to serialize WrappedObject"),
                );
            }
        }
    }

    pub fn digest_live_object_set(&self) -> ECMHLiveObjectSetDigest {
        let acc = self.accumulate_live_object_set();
        acc.digest().into()
    }

    pub async fn digest_epoch(
        &self,
        epoch_store: Arc<AuthorityPerEpochStore>,
        last_checkpoint_of_epoch: CheckpointSequenceNumber,
    ) -> IotaResult<ECMHLiveObjectSetDigest> {
        Ok(self
            .accumulate_epoch(epoch_store, last_checkpoint_of_epoch)
            .await?
            .digest()
            .into())
    }
}

impl StateAccumulatorV1 {
    pub fn new(store: Arc<dyn AccumulatorStore>, metrics: Arc<StateAccumulatorMetrics>) -> Self {
        Self { store, metrics }
    }

    pub async fn accumulate_running_root(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        checkpoint_seq_num: CheckpointSequenceNumber,
        checkpoint_acc: Option<Accumulator>,
    ) -> IotaResult {
        let _scope = monitored_scope("AccumulateRunningRoot");
        tracing::debug!(
            "accumulating running root for checkpoint {}",
            checkpoint_seq_num
        );

        // For the last checkpoint of the epoch, this function will be called once by
        // the checkpoint builder, and again by checkpoint executor.
        //
        // Normally this is fine, since the notify_read_running_root(checkpoint_seq_num
        // - 1) will work normally. But if there is only one checkpoint in the
        // epoch, that call will hang forever, since the previous checkpoint
        // belongs to the previous epoch.
        if epoch_store
            .get_running_root_accumulator(&checkpoint_seq_num)?
            .is_some()
        {
            debug!(
                "accumulate_running_root {:?} {:?} already exists",
                epoch_store.epoch(),
                checkpoint_seq_num
            );
            return Ok(());
        }

        let mut running_root = if checkpoint_seq_num == 0 {
            // we're at genesis and need to start from scratch
            Accumulator::default()
        } else if epoch_store
            .get_highest_running_root_accumulator()?
            .is_none()
        {
            // we're at the beginning of a new epoch and need to
            // bootstrap from the previous epoch's root state hash. Because this
            // should only occur at beginning of epoch, we shouldn't have to worry
            // about race conditions on reading the highest running root accumulator.
            if let Some((prev_epoch, (last_checkpoint_prev_epoch, prev_acc))) =
                self.store.get_root_state_accumulator_for_highest_epoch()?
            {
                if last_checkpoint_prev_epoch != checkpoint_seq_num - 1 {
                    epoch_store
                        .notify_read_running_root(checkpoint_seq_num - 1)
                        .await?
                } else {
                    assert_eq!(
                        prev_epoch + 1,
                        epoch_store.epoch(),
                        "Expected highest existing root state hash to be for previous epoch",
                    );
                    prev_acc
                }
            } else {
                // Rare edge case where we manage to somehow lag in checkpoint execution from
                // genesis such that the end of epoch checkpoint is built before
                // we execute any checkpoints.
                assert_eq!(
                    epoch_store.epoch(),
                    0,
                    "Expected epoch to be 0 if previous root state hash does not exist"
                );
                epoch_store
                    .notify_read_running_root(checkpoint_seq_num - 1)
                    .await?
            }
        } else {
            epoch_store
                .notify_read_running_root(checkpoint_seq_num - 1)
                .await?
        };

        let checkpoint_acc = checkpoint_acc.unwrap_or_else(|| {
            epoch_store
                .get_state_hash_for_checkpoint(&checkpoint_seq_num)
                .expect("Failed to get checkpoint accumulator from disk")
                .expect("Expected checkpoint accumulator to exist")
        });
        running_root.union(&checkpoint_acc);
        epoch_store.insert_running_root_accumulator(&checkpoint_seq_num, &running_root)?;
        debug!(
            "Accumulated checkpoint {} to running root accumulator",
            checkpoint_seq_num,
        );
        Ok(())
    }

    pub fn accumulate_epoch(
        &self,
        epoch_store: Arc<AuthorityPerEpochStore>,
        last_checkpoint_of_epoch: CheckpointSequenceNumber,
    ) -> IotaResult<Accumulator> {
        let _scope = monitored_scope("AccumulateEpoch");
        let running_root = epoch_store
            .get_running_root_accumulator(&last_checkpoint_of_epoch)?
            .expect("Expected running root accumulator to exist up to last checkpoint of epoch");

        self.store.insert_state_accumulator_for_epoch(
            epoch_store.epoch(),
            &last_checkpoint_of_epoch,
            &running_root,
        )?;
        debug!(
            "Finalized root state hash for epoch {} (up to checkpoint {})",
            epoch_store.epoch(),
            last_checkpoint_of_epoch
        );
        Ok(running_root.clone())
    }

    pub fn accumulate_effects(&self, effects: Vec<TransactionEffects>) -> Accumulator {
        accumulate_effects(effects)
    }
}
