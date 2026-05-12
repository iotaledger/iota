// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use iota_types::{
    base_types::SequenceNumber,
    digests::TransactionEventsDigest,
    effects::{TransactionEffects, TransactionEvents},
    global_state_hash::GlobalStateHash,
    storage::MarkerValue,
};
use serde::{Deserialize, Serialize};
use tracing::error;
use typed_store::{
    DBMapUtils, DbIterator,
    metrics::SamplingInterval,
    rocks::{
        DBBatch, DBMap, DBMapTableConfigMap, DBOptions, MetricConf, default_db_options,
        read_size_from_env,
    },
    rocksdb::compaction_filter::Decision,
    traits::Map,
};

use super::*;
use crate::authority::{
    authority_store_pruner::ObjectsCompactionFilter,
    authority_store_types::{
        SENTINEL_PREVIOUS_TRANSACTION_CHECKPOINT, StoreObject, StoreObjectValueV2,
        StoreObjectWrapper, get_store_object, try_construct_object,
    },
    epoch_start_configuration::EpochStartConfiguration,
};

const ENV_VAR_OBJECTS_BLOCK_CACHE_SIZE: &str = "OBJECTS_BLOCK_CACHE_MB";
pub(crate) const ENV_VAR_LOCKS_BLOCK_CACHE_SIZE: &str = "LOCKS_BLOCK_CACHE_MB";
const ENV_VAR_TRANSACTIONS_BLOCK_CACHE_SIZE: &str = "TRANSACTIONS_BLOCK_CACHE_MB";
const ENV_VAR_EFFECTS_BLOCK_CACHE_SIZE: &str = "EFFECTS_BLOCK_CACHE_MB";
const ENV_VAR_EVENTS_BLOCK_CACHE_SIZE: &str = "EVENTS_BLOCK_CACHE_MB";

/// Options to apply to every column family of the `perpetual` DB.
#[derive(Default)]
pub struct AuthorityPerpetualTablesOptions {
    /// Whether to enable write stalling on all column families.
    pub enable_write_stall: bool,
    pub compaction_filter: Option<ObjectsCompactionFilter>,
}

impl AuthorityPerpetualTablesOptions {
    fn apply_to(&self, mut db_options: DBOptions) -> DBOptions {
        if !self.enable_write_stall {
            db_options = db_options.disable_write_throttling();
        }
        db_options
    }
}

/// AuthorityPerpetualTables contains data that must be preserved from one epoch
/// to the next.
#[derive(DBMapUtils)]
pub struct AuthorityPerpetualTables {
    /// This is a map between the object (ID, version) and the latest state of
    /// the object, namely the state that is needed to process new
    /// transactions. State is represented by `StoreObject` enum, which is
    /// either a move module or a move object.
    ///
    /// Note that while this map can store all versions of an object, we will
    /// eventually prune old object versions from the db.
    ///
    /// IMPORTANT: object versions must *only* be pruned if they appear as
    /// inputs in some TransactionEffects. Simply pruning all objects but
    /// the most recent is an error! This is because there can be partially
    /// executed transactions whose effects have not yet been written out,
    /// and which must be retried. But, they cannot be retried unless their
    /// input objects are still accessible!
    pub(crate) objects: DBMap<ObjectKey, StoreObjectWrapper>,

    /// Object references of currently active objects that can be mutated.
    pub(crate) live_owned_object_markers: DBMap<ObjectRef, ()>,

    /// This is a map between the transaction digest and the corresponding
    /// transaction that's known to be executable. This means that it may
    /// have been executed locally, or it may have been synced through
    /// state-sync but hasn't been executed yet.
    pub(crate) transactions: DBMap<TransactionDigest, TrustedTransaction>,

    /// A map between the transaction digest of a certificate to the effects of
    /// its execution. We store effects into this table in two different
    /// cases:
    /// 1. When a transaction is synced through state_sync, we store the effects
    ///    here. These effects are known to be final in the network, but may not
    ///    have been executed locally yet.
    /// 2. When the transaction is executed locally on this node, we store the
    ///    effects here. This means that it's possible to store the same effects
    ///    twice (once for the synced transaction, and once for the executed).
    ///
    /// It's also possible for the effects to be reverted if the transaction
    /// didn't make it into the epoch.
    pub(crate) effects: DBMap<TransactionEffectsDigest, TransactionEffects>,

    /// Transactions that have been executed locally on this node. We need this
    /// table since the `effects` table doesn't say anything about the
    /// execution status of the transaction on this node. When we wait for
    /// transactions to be executed, we wait for them to appear in this
    /// table. When we revert transactions, we remove them from both tables.
    pub(crate) executed_effects: DBMap<TransactionDigest, TransactionEffectsDigest>,

    // Currently this is needed in the validator for returning events during process certificates.
    // We could potentially remove this if we decided not to provide events in the execution path.
    // TODO: Figure out what to do with this table in the long run.
    // Also we need a pruning policy for this table. We can prune this table along with tx/effects.
    pub(crate) events: DBMap<(TransactionEventsDigest, usize), Event>,

    // Events keyed by the digest of the transaction that produced them.
    pub(crate) events_2: DBMap<TransactionDigest, TransactionEvents>,

    /// Epoch and checkpoint of transactions finalized by checkpoint
    /// executor. Currently, mainly used to implement JSON RPC `ReadApi`.
    /// Note, there is a table with the same name in
    /// `AuthorityEpochTables`/`AuthorityPerEpochStore`.
    pub(crate) executed_transactions_to_checkpoint:
        DBMap<TransactionDigest, (EpochId, CheckpointSequenceNumber)>,

    // Finalized root state hash for epoch, to be included in CheckpointSummary
    // of last checkpoint of epoch. These values should only ever be written once
    // and never changed
    pub(crate) root_state_hash_by_epoch:
        DBMap<EpochId, (CheckpointSequenceNumber, GlobalStateHash)>,

    /// Parameters of the system fixed at the epoch start
    pub(crate) epoch_start_configuration: DBMap<(), EpochStartConfiguration>,

    /// A singleton table that stores latest pruned checkpoint. Used to keep
    /// objects pruner progress
    pub(crate) pruned_checkpoint: DBMap<(), CheckpointSequenceNumber>,

    /// The total IOTA supply and the epoch at which it was stored.
    /// We check and update it at the end of each epoch if expensive checks are
    /// enabled.
    pub(crate) total_iota_supply: DBMap<(), TotalIotaSupplyCheck>,

    /// Expected imbalance between storage fund balance and the sum of storage
    /// rebate of all live objects. This could be non-zero due to bugs in
    /// earlier protocol versions. This number is the result of
    /// storage_fund_balance - sum(storage_rebate).
    pub(crate) expected_storage_fund_imbalance: DBMap<(), i64>,

    /// Table that stores the set of received objects and deleted objects and
    /// the version at which they were received. This is used to prevent
    /// possible race conditions around receiving objects (since they are
    /// not locked by the transaction manager) and for tracking shared
    /// objects that have been deleted. This table is meant to be pruned
    /// per-epoch, and all previous epochs other than the current epoch may
    /// be pruned safely.
    pub(crate) object_per_epoch_marker_table: DBMap<(EpochId, ObjectKey), MarkerValue>,
}

#[derive(DBMapUtils)]
pub struct AuthorityPrunerTables {
    pub(crate) object_tombstones: DBMap<ObjectID, SequenceNumber>,
}

impl AuthorityPrunerTables {
    pub fn path(parent_path: &Path) -> PathBuf {
        parent_path.join("pruner")
    }

    pub fn open(parent_path: &Path) -> Self {
        Self::open_tables_read_write(
            Self::path(parent_path),
            MetricConf::new("pruner")
                .with_sampling(SamplingInterval::new(Duration::from_secs(60), 0)),
            None,
            None,
        )
    }
}

/// The total IOTA supply used during conservation checks.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TotalIotaSupplyCheck {
    /// The IOTA supply at the time of `last_check_epoch`.
    pub(crate) total_supply: u64,
    /// The epoch at which the total supply was last checked or updated.
    pub(crate) last_check_epoch: EpochId,
}

impl AuthorityPerpetualTables {
    pub fn path(parent_path: &Path) -> PathBuf {
        parent_path.join("perpetual")
    }

    pub fn open(
        parent_path: &Path,
        db_options_override: Option<AuthorityPerpetualTablesOptions>,
    ) -> Self {
        let db_options_override = db_options_override.unwrap_or_default();
        let db_options =
            db_options_override.apply_to(default_db_options().optimize_db_for_write_throughput(4));
        let table_options = DBMapTableConfigMap::new(BTreeMap::from([
            (
                "objects".to_string(),
                objects_table_config(db_options.clone(), db_options_override.compaction_filter),
            ),
            (
                "live_owned_object_markers".to_string(),
                live_owned_object_markers_table_config(db_options.clone()),
            ),
            (
                "transactions".to_string(),
                transactions_table_config(db_options.clone()),
            ),
            (
                "effects".to_string(),
                effects_table_config(db_options.clone()),
            ),
            (
                "events".to_string(),
                events_table_config(db_options.clone()),
            ),
        ]));
        Self::open_tables_read_write(
            Self::path(parent_path),
            MetricConf::new("perpetual")
                .with_sampling(SamplingInterval::new(Duration::from_secs(60), 0)),
            Some(db_options.options),
            Some(table_options),
        )
    }

    pub fn open_readonly(parent_path: &Path) -> AuthorityPerpetualTablesReadOnly {
        Self::get_read_only_handle(
            Self::path(parent_path),
            None,
            None,
            MetricConf::new("perpetual_readonly"),
        )
    }

    // This is used by indexer to find the correct version of dynamic field child
    // object. We do not store the version of the child object, but because of
    // lamport timestamp, we know the child must have version number less then
    // or eq to the parent.
    pub fn find_object_lt_or_eq_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        let mut iter = self.objects.reversed_safe_iter_with_bounds(
            Some(ObjectKey::min_for_id(&object_id)),
            Some(ObjectKey(object_id, version)),
        )?;
        match iter.next() {
            Some(Ok((key, o))) => self.object(&key, o),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    fn construct_object(
        &self,
        object_key: &ObjectKey,
        store_object: StoreObjectValueV2,
    ) -> Result<Object, IotaError> {
        try_construct_object(object_key, store_object)
    }

    // Constructs `iota_types::object::Object` from `StoreObjectWrapper`.
    // Returns `None` if object was deleted/wrapped
    pub fn object(
        &self,
        object_key: &ObjectKey,
        store_object: StoreObjectWrapper,
    ) -> Result<Option<Object>, IotaError> {
        let StoreObject::Value(store_object) = store_object.migrate().into_inner() else {
            return Ok(None);
        };
        Ok(Some(self.construct_object(object_key, *store_object)?))
    }

    pub fn object_reference(
        &self,
        object_key: &ObjectKey,
        store_object: StoreObjectWrapper,
    ) -> Result<ObjectRef, IotaError> {
        let obj_ref = match store_object.migrate().into_inner() {
            StoreObject::Value(object) => self
                .construct_object(object_key, *object)?
                .compute_object_reference(),
            StoreObject::Deleted => {
                ObjectRef::new(object_key.0, object_key.1, ObjectDigest::OBJECT_DELETED)
            }
            StoreObject::Wrapped => {
                ObjectRef::new(object_key.0, object_key.1, ObjectDigest::OBJECT_WRAPPED)
            }
        };
        Ok(obj_ref)
    }

    pub fn tombstone_reference(
        &self,
        object_key: &ObjectKey,
        store_object: &StoreObjectWrapper,
    ) -> Result<Option<ObjectRef>, IotaError> {
        let obj_ref = match store_object.inner() {
            StoreObject::Deleted => Some(ObjectRef::new(
                object_key.0,
                object_key.1,
                ObjectDigest::OBJECT_DELETED,
            )),
            StoreObject::Wrapped => Some(ObjectRef::new(
                object_key.0,
                object_key.1,
                ObjectDigest::OBJECT_WRAPPED,
            )),
            _ => None,
        };
        Ok(obj_ref)
    }

    pub fn get_latest_object_ref_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> Result<Option<ObjectRef>, IotaError> {
        let mut iterator = self.objects.reversed_safe_iter_with_bounds(
            Some(ObjectKey::min_for_id(&object_id)),
            Some(ObjectKey::max_for_id(&object_id)),
        )?;

        if let Some(Ok((object_key, value))) = iterator.next() {
            if object_key.0 == object_id {
                return Ok(Some(self.object_reference(&object_key, value)?));
            }
        }
        Ok(None)
    }

    pub fn get_latest_object_or_tombstone(
        &self,
        object_id: ObjectID,
    ) -> Result<Option<(ObjectKey, StoreObjectWrapper)>, IotaError> {
        let mut iterator = self.objects.reversed_safe_iter_with_bounds(
            Some(ObjectKey::min_for_id(&object_id)),
            Some(ObjectKey::max_for_id(&object_id)),
        )?;

        if let Some(Ok((object_key, value))) = iterator.next() {
            if object_key.0 == object_id {
                return Ok(Some((object_key, value)));
            }
        }
        Ok(None)
    }

    pub fn get_recovery_epoch_at_restart(&self) -> IotaResult<EpochId> {
        Ok(self
            .epoch_start_configuration
            .get(&())?
            .expect("Must have current epoch.")
            .epoch_start_state()
            .epoch())
    }

    pub fn set_epoch_start_configuration(
        &self,
        epoch_start_configuration: &EpochStartConfiguration,
    ) -> IotaResult {
        let mut wb = self.epoch_start_configuration.batch();
        wb.insert_batch(
            &self.epoch_start_configuration,
            std::iter::once(((), epoch_start_configuration)),
        )?;
        wb.write()?;
        Ok(())
    }

    pub fn get_highest_pruned_checkpoint(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>, TypedStoreError> {
        self.pruned_checkpoint.get(&())
    }

    pub fn set_highest_pruned_checkpoint(
        &self,
        wb: &mut DBBatch,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> IotaResult {
        wb.insert_batch(&self.pruned_checkpoint, [((), checkpoint_number)])?;
        Ok(())
    }

    pub fn get_transaction(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TrustedTransaction>> {
        let Some(transaction) = self.transactions.get(digest)? else {
            return Ok(None);
        };
        Ok(Some(transaction))
    }

    pub fn get_effects(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<TransactionEffects>> {
        let Some(effect_digest) = self.executed_effects.get(digest)? else {
            return Ok(None);
        };
        Ok(self.effects.get(&effect_digest)?)
    }

    pub fn get_checkpoint_sequence_number(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<(EpochId, CheckpointSequenceNumber)>> {
        Ok(self.executed_transactions_to_checkpoint.get(digest)?)
    }

    pub fn get_newer_object_keys(
        &self,
        object: &(ObjectID, SequenceNumber),
    ) -> IotaResult<Vec<ObjectKey>> {
        let mut objects = vec![];
        for result in self.objects.safe_iter_with_bounds(
            Some(ObjectKey(object.0, object.1.next().unwrap())),
            Some(ObjectKey(object.0, VersionNumber::MAX_VALID_EXCL)),
        ) {
            let (key, _) = result?;
            objects.push(key);
        }
        Ok(objects)
    }

    pub fn set_highest_pruned_checkpoint_without_wb(
        &self,
        checkpoint_number: CheckpointSequenceNumber,
    ) -> IotaResult {
        let mut wb = self.pruned_checkpoint.batch();
        self.set_highest_pruned_checkpoint(&mut wb, checkpoint_number)?;
        wb.write()?;
        Ok(())
    }

    pub fn database_is_empty(&self) -> IotaResult<bool> {
        Ok(self.objects.safe_iter().next().is_none())
    }

    pub fn iter_live_object_set(&self) -> LiveSetIter<'_> {
        LiveSetIter(self.iter_live_object_set_v2())
    }

    pub fn range_iter_live_object_set(
        &self,
        lower_bound: Option<ObjectID>,
        upper_bound: Option<ObjectID>,
    ) -> LiveSetIter<'_> {
        let lower_bound = lower_bound.as_ref().map(ObjectKey::min_for_id);
        let upper_bound = upper_bound.as_ref().map(ObjectKey::max_for_id);

        LiveSetIter(LiveSetIterV2 {
            iter: Box::new(self.objects.safe_iter_with_bounds(lower_bound, upper_bound)),
            tables: self,
            prev: None,
        })
    }

    /// Like `iter_live_object_set` but additionally surfaces each live
    /// object's `previous_transaction_checkpoint`. Used by the snapshot V2
    /// writer to populate the per-object trailer of the reference file.
    pub fn iter_live_object_set_v2(&self) -> LiveSetIterV2<'_> {
        LiveSetIterV2 {
            iter: Box::new(self.objects.safe_iter()),
            tables: self,
            prev: None,
        }
    }

    pub fn checkpoint_db(&self, path: &Path) -> IotaResult {
        // This checkpoints the entire db and not just objects table
        self.objects.checkpoint_db(path).map_err(Into::into)
    }

    pub fn get_root_state_hash(
        &self,
        epoch: EpochId,
    ) -> IotaResult<Option<(CheckpointSequenceNumber, GlobalStateHash)>> {
        Ok(self.root_state_hash_by_epoch.get(&epoch)?)
    }

    pub fn insert_root_state_hash(
        &self,
        epoch: EpochId,
        last_checkpoint_of_epoch: CheckpointSequenceNumber,
        hash: GlobalStateHash,
    ) -> IotaResult {
        self.root_state_hash_by_epoch
            .insert(&epoch, &(last_checkpoint_of_epoch, hash))?;
        Ok(())
    }

    pub fn insert_object_test_only(&self, object: Object) -> IotaResult {
        let object_reference = object.compute_object_reference();
        let wrapper = get_store_object(object, SENTINEL_PREVIOUS_TRANSACTION_CHECKPOINT);
        let mut wb = self.objects.batch();
        wb.insert_batch(
            &self.objects,
            std::iter::once((ObjectKey::from(object_reference), wrapper)),
        )?;
        wb.write()?;
        Ok(())
    }
}

impl ObjectStore for AuthorityPerpetualTables {
    /// Read an object and return it, or Ok(None) if the object was not found.
    fn try_get_object(
        &self,
        object_id: &ObjectID,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        let obj_entry = self
            .objects
            .reversed_safe_iter_with_bounds(None, Some(ObjectKey::max_for_id(object_id)))
            .map_err(iota_types::storage::error::Error::custom)?
            .next();

        match obj_entry.transpose()? {
            Some((ObjectKey(obj_id, version), obj)) if obj_id == *object_id => Ok(self
                .object(&ObjectKey(obj_id, version), obj)
                .map_err(iota_types::storage::error::Error::custom)?),
            _ => Ok(None),
        }
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        Ok(self
            .objects
            .get(&ObjectKey(*object_id, version))
            .map_err(iota_types::storage::error::Error::custom)?
            .map(|object| self.object(&ObjectKey(*object_id, version), object))
            .transpose()
            .map_err(iota_types::storage::error::Error::custom)?
            .flatten())
    }
}

/// Yields the same live-object set as `LiveSetIterV2` but strips the
/// per-object `previous_transaction_checkpoint` trailer, for callers that do
/// not need it.
pub struct LiveSetIter<'a>(LiveSetIterV2<'a>);

/// A row that the live-set iterator surfaces. `LiveSetIter` filters
/// `StoreObject::Wrapped` and `StoreObject::Deleted` at the source, so
/// wrapped and deleted objects never reach downstream consumers (snapshot
/// writer, state-hash accumulator, restore path) - every yielded row is
/// a live `Object`.
// `Serialize`/`Deserialize` are load-bearing: the snapshot writer
// BCS-encodes each `LiveObject` into the bucketed `.obj` files
// (`iota-snapshot::writer::write_object`), and the reader BCS-decodes
// them (`iota-snapshot::reader::LiveObjectIter`). Collapsing this from
// the previous single-variant enum drops the 1-byte variant tag from
// the wire format - that's fine because snapshot V2 is new (V1 readers
// reject V2 magic and vice versa), so no existing files carry the old
// shape.
#[derive(Eq, PartialEq, Debug, Clone, Deserialize, Serialize, Hash)]
pub struct LiveObject(pub Object);

impl LiveObject {
    pub fn object_id(&self) -> ObjectID {
        self.0.id()
    }

    pub fn version(&self) -> SequenceNumber {
        self.0.version()
    }

    pub fn object_reference(&self) -> ObjectRef {
        self.0.compute_object_reference()
    }
}

impl Iterator for LiveSetIter<'_> {
    type Item = LiveObject;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|v| v.live)
    }
}

/// A live object together with the checkpoint sequence number that contained
/// the transaction whose effects produced this object version. Yielded by
/// `LiveSetIterV2`.
#[derive(Eq, PartialEq, Debug, Clone)]
pub struct LiveObjectV2 {
    pub live: LiveObject,
    pub previous_transaction_checkpoint: CheckpointSequenceNumber,
}

pub struct LiveSetIterV2<'a> {
    iter: DbIterator<'a, (ObjectKey, StoreObjectWrapper)>,
    tables: &'a AuthorityPerpetualTables,
    prev: Option<(ObjectKey, StoreObjectWrapper)>,
}

impl LiveSetIterV2<'_> {
    fn store_object_wrapper_to_live_object(
        &self,
        object_key: ObjectKey,
        store_object: StoreObjectWrapper,
    ) -> Option<LiveObjectV2> {
        match store_object.migrate().into_inner() {
            StoreObject::Value(value) => {
                let previous_transaction_checkpoint = value.previous_transaction_checkpoint;
                let object = self
                    .tables
                    .construct_object(&object_key, *value)
                    .expect("Constructing object from store cannot fail");
                Some(LiveObjectV2 {
                    live: LiveObject(object),
                    previous_transaction_checkpoint,
                })
            }
            StoreObject::Wrapped | StoreObject::Deleted => None,
        }
    }
}

impl Iterator for LiveSetIterV2<'_> {
    type Item = LiveObjectV2;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(Ok((next_key, next_value))) = self.iter.next() {
                let prev = self.prev.take();
                self.prev = Some((next_key, next_value));

                if let Some((prev_key, prev_value)) = prev {
                    if prev_key.0 != next_key.0 {
                        let live_object =
                            self.store_object_wrapper_to_live_object(prev_key, prev_value);
                        if live_object.is_some() {
                            return live_object;
                        }
                    }
                }
                continue;
            }
            if let Some((key, value)) = self.prev.take() {
                let live_object = self.store_object_wrapper_to_live_object(key, value);
                if live_object.is_some() {
                    return live_object;
                }
            }
            return None;
        }
    }
}

// These functions are used to initialize the DB tables
fn live_owned_object_markers_table_config(db_options: DBOptions) -> DBOptions {
    DBOptions {
        options: db_options
            .clone()
            .optimize_for_write_throughput()
            .optimize_for_read(read_size_from_env(ENV_VAR_LOCKS_BLOCK_CACHE_SIZE).unwrap_or(1024))
            .options,
        rw_options: db_options.rw_options,
    }
}

fn objects_table_config(
    mut db_options: DBOptions,
    compaction_filter: Option<ObjectsCompactionFilter>,
) -> DBOptions {
    if let Some(mut compaction_filter) = compaction_filter {
        db_options
            .options
            .set_compaction_filter("objects", move |_, key, value| {
                match compaction_filter.filter(key, value) {
                    Ok(decision) => decision,
                    Err(err) => {
                        error!("Compaction error: {:?}", err);
                        Decision::Keep
                    }
                }
            });
    }
    db_options
        .optimize_for_write_throughput()
        .optimize_for_read(read_size_from_env(ENV_VAR_OBJECTS_BLOCK_CACHE_SIZE).unwrap_or(5 * 1024))
}

fn transactions_table_config(db_options: DBOptions) -> DBOptions {
    db_options
        .optimize_for_write_throughput()
        .optimize_for_point_lookup(
            read_size_from_env(ENV_VAR_TRANSACTIONS_BLOCK_CACHE_SIZE).unwrap_or(512),
        )
}

fn effects_table_config(db_options: DBOptions) -> DBOptions {
    db_options
        .optimize_for_write_throughput()
        .optimize_for_point_lookup(
            read_size_from_env(ENV_VAR_EFFECTS_BLOCK_CACHE_SIZE).unwrap_or(1024),
        )
}

fn events_table_config(db_options: DBOptions) -> DBOptions {
    db_options
        .optimize_for_write_throughput()
        .optimize_for_read(read_size_from_env(ENV_VAR_EVENTS_BLOCK_CACHE_SIZE).unwrap_or(1024))
}

#[cfg(test)]
mod tests {
    use iota_types::base_types::ObjectID;

    use super::*;
    use crate::authority::authority_store_types::StoreObjectV2;

    /// Combined into one `#[tokio::test]` to sidestep the
    /// `typed_store::DBMetrics` global Prometheus registry race (concurrent
    /// `AuthorityPerpetualTables::open` calls hit `AlreadyReg`). The two cases
    /// are independent; do not split until the metrics registry is made
    /// re-entrant.
    #[tokio::test]
    async fn live_set_iter_invariants() {
        live_set_iter_filters_wrapped_and_deleted_store_rows();
        live_set_iter_v2_propagates_previous_transaction_checkpoint();
    }

    /// `LiveSetIter` must filter `StoreObject::Wrapped` and
    /// `StoreObject::Deleted` rows at the source so downstream consumers
    /// (snapshot writer, state-hash accumulator, restore path) only ever
    /// observe live objects. This invariant is what lets `LiveObject` be
    /// a plain `Object` wrapper.
    fn live_set_iter_filters_wrapped_and_deleted_store_rows() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        // A live `Normal` row alongside `Wrapped` and `Deleted` tombstones for
        // distinct object IDs.
        let live_id = ObjectID::random();
        let wrapped_id = ObjectID::random();
        let deleted_id = ObjectID::random();

        let live_object = Object::immutable_with_id_for_testing(live_id);
        perpetual_db.insert_object_test_only(live_object).unwrap();

        let mut wb = perpetual_db.objects.batch();
        let wrapped_key = ObjectKey(wrapped_id, SequenceNumber::from_u64(1));
        wb.insert_batch(
            &perpetual_db.objects,
            std::iter::once::<(ObjectKey, StoreObjectWrapper)>((
                wrapped_key,
                StoreObjectV2::Wrapped.into(),
            )),
        )
        .unwrap();
        let deleted_key = ObjectKey(deleted_id, SequenceNumber::from_u64(1));
        wb.insert_batch(
            &perpetual_db.objects,
            std::iter::once::<(ObjectKey, StoreObjectWrapper)>((
                deleted_key,
                StoreObjectV2::Deleted.into(),
            )),
        )
        .unwrap();
        wb.write().unwrap();

        let yielded: Vec<_> = perpetual_db.iter_live_object_set().collect();
        assert_eq!(yielded.len(), 1, "wrapped/deleted rows must be filtered");
        let LiveObject(only) = yielded.into_iter().next().unwrap();
        assert_eq!(only.id(), live_id);
    }

    /// `LiveSetIterV2` must surface the exact `previous_transaction_checkpoint`
    /// stored on `StoreObjectValueV2` - it is the load-bearing input to the
    /// snapshot V2 writer's per-record trailer. A bug that, e.g., always
    /// stamped `0` here would silently corrupt every snapshot's per-record
    /// trailer; this is the focused canary for that contract.
    fn live_set_iter_v2_propagates_previous_transaction_checkpoint() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        // Insert a live object via the standard test path. This stamps the
        // sentinel checkpoint by default; we then overwrite the row with a
        // hand-built value carrying a distinct, recognizable checkpoint.
        let object = Object::immutable_with_id_for_testing(ObjectID::random());
        let object_ref = object.compute_object_reference();
        let object_key = ObjectKey::from(object_ref);
        let distinct_checkpoint: u64 = 0xCAFE_F00D_BEEF_1234;

        let store_object_value = match get_store_object(object, distinct_checkpoint).into_inner() {
            StoreObject::Value(value) => value,
            other => panic!("expected StoreObject::Value, got {other:?}"),
        };
        let wrapper: StoreObjectWrapper = StoreObjectV2::Value(store_object_value).into();
        let mut wb = perpetual_db.objects.batch();
        wb.insert_batch(
            &perpetual_db.objects,
            std::iter::once((object_key, wrapper)),
        )
        .unwrap();
        wb.write().unwrap();

        let yielded: Vec<_> = perpetual_db.iter_live_object_set_v2().collect();
        assert_eq!(yielded.len(), 1);
        assert_eq!(
            yielded[0].previous_transaction_checkpoint, distinct_checkpoint,
            "LiveSetIterV2 must surface the on-row checkpoint, not a default"
        );
    }
}
