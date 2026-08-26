// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iota_metrics::spawn_monitored_task;
use iota_sdk_types::{TransactionEffects, TransactionEvents, Version};
use iota_types::{global_state_hash::GlobalStateHash, storage::MarkerValue};
use serde::{Deserialize, Serialize};
use typed_store::{
    DBMapUtils, DbIterator,
    database::Database,
    metrics::SamplingInterval,
    rocks::{
        DBMap, DBMapTableConfigMap, DBOptions, MetricConf, ReadWriteOptions, default_db_options,
        read_size_from_env,
    },
    rocksdb::LiveFile,
    traits::Map,
};

use super::*;
use crate::authority::{
    authority_store_types::{
        StoreObject, StoreObjectValueV2, StoreObjectWrapper, get_store_object, try_construct_object,
    },
    epoch_markers::EpochMarkers,
    epoch_start_configuration::EpochStartConfiguration,
    historic_ledger::HistoricLedger,
    historic_objects::HistoricObjects,
    ledger_backlog_migration::LedgerBacklogMigrationProgress,
    object_backlog_sweep::ObjectBacklogSweepProgress,
};

const ENV_VAR_OBJECTS_BLOCK_CACHE_SIZE: &str = "OBJECTS_BLOCK_CACHE_MB";
pub(crate) const ENV_VAR_LOCKS_BLOCK_CACHE_SIZE: &str = "LOCKS_BLOCK_CACHE_MB";
const ENV_VAR_TRANSACTIONS_BLOCK_CACHE_SIZE: &str = "TRANSACTIONS_BLOCK_CACHE_MB";
const ENV_VAR_EFFECTS_BLOCK_CACHE_SIZE: &str = "EFFECTS_BLOCK_CACHE_MB";

/// Copies the objects pruner's progress watermark out of `pruned_checkpoint`
/// into `object_backlog_sweep_bound`, so that the one-time sweep can still
/// read it after the deprecated column family has been dropped.
///
/// The pruner of an earlier build wrote that watermark in the same batch as
/// its deletes, so it says exactly how far the deletes reached. Nothing is
/// written when the table is absent or empty, which leaves the sweep to walk
/// the whole table as it would have anyway.
// TODO: remove this together with the sweep it bounds,
// <https://github.com/iotaledger/iota/issues/12712>
fn rescue_objects_pruner_watermark(db: &Arc<Database>) -> Result<(), TypedStoreError> {
    let pruned: DBMap<(), CheckpointSequenceNumber> = DBMap::reopen(
        db,
        Some("pruned_checkpoint"),
        &ReadWriteOptions::default(),
        true,
    )?;
    let Some(watermark) = pruned.get(&())? else {
        return Ok(());
    };
    let bound: DBMap<(), CheckpointSequenceNumber> = DBMap::reopen(
        db,
        Some("object_backlog_sweep_bound"),
        &ReadWriteOptions::default(),
        false,
    )?;
    // Only ever written here, and this runs once before the column family it
    // reads is dropped, so there is nothing to overwrite.
    bound.insert(&(), &watermark)?;
    info!(
        watermark,
        "carried the objects pruner's watermark over for the one-time sweep"
    );
    Ok(())
}

/// Options to apply to every column family of the `perpetual` DB.
#[derive(Default)]
pub struct AuthorityPerpetualTablesOptions {
    /// Whether to enable write stalling on all column families.
    pub enable_write_stall: bool,
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
    ///
    /// Non-latest versions are historical state, needed only to derive
    /// `showBalanceChanges` / `showObjectChanges` for a past transaction.
    /// They prune with their own knob rather than the ledger's: peers never
    /// need old versions to sync, and both APIs already degrade explicitly
    /// when a version is gone.
    pub(crate) objects: DBMap<ObjectKey, StoreObjectWrapper>,

    /// Object references of currently active objects that can be mutated.
    pub(crate) live_owned_object_markers: DBMap<ObjectReference, ()>,

    /// This is a map between the transaction digest and the corresponding
    /// transaction that's known to be executable. This means that it may
    /// have been executed locally, or it may have been synced through
    /// state-sync but hasn't been executed yet.
    ///
    /// Superseded by [`HistoricLedger`]: a transaction body is written to and
    /// read from the bucket of the epoch that executes it. Rows written before
    /// the move are still on disk here, and the one-time migration into the
    /// buckets is their only reader.
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
    ///
    /// Superseded by [`HistoricLedger`] the same way `transactions` is, and
    /// with the same one reader left for the rows written before the move.
    pub(crate) effects: DBMap<TransactionEffectsDigest, TransactionEffects>,

    /// Transactions that have been executed locally on this node. We need this
    /// table since the `effects` table doesn't say anything about the
    /// execution status of the transaction on this node. When we wait for
    /// transactions to be executed, we wait for them to appear in this
    /// table. When we revert transactions, we remove them from both tables.
    ///
    /// Superseded by [`HistoricLedger`] the same way `transactions` is, and
    /// with the same one reader left for the rows written before the move.
    pub(crate) executed_effects: DBMap<TransactionDigest, TransactionEffectsDigest>,

    /// Events produced by each transaction, keyed by the transaction's
    /// digest.
    ///
    /// Superseded by [`HistoricLedger`] the same way `transactions` is, and
    /// with the same one reader left for the rows written before the move.
    pub(crate) events_2: DBMap<TransactionDigest, TransactionEvents>,

    /// Epoch and checkpoint of transactions finalized by checkpoint
    /// executor.
    ///
    /// Superseded by [`HistoricLedger`], which keys the same answer by
    /// transaction digest inside the bucket of the epoch that finalized it, so
    /// the epoch is the bucket's and the row holds only the sequence number.
    /// Rows written before the move are still on disk here; besides the
    /// one-time migration into the buckets, they are also what tells that
    /// migration which epoch a transaction's other rows belong to.
    ///
    /// The value keeps the epoch rather than collapsing to just the sequence
    /// number: every row here predates this build, `bcs` rejects the trailing
    /// bytes a shorter value would leave unread, and the migration is the only
    /// remaining reader, so there is no way to reshape the value without
    /// making its own reads of a still-unmigrated database fail.
    ///
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

    /// Deprecated: was the objects pruner's progress watermark. The objects
    /// pruner has been replaced by per-epoch bucket expiry, which has no use
    /// for this table — but the one-time sweep does, so the watermark is
    /// copied into `object_backlog_sweep_bound` before the column family is
    /// dropped.
    #[allow(dead_code)]
    #[deprecated_db_map(migration = "rescue_objects_pruner_watermark")]
    pruned_checkpoint: Option<DBMap<(), CheckpointSequenceNumber>>,

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

    /// How far the one-time sweep of the object versions superseded before
    /// this build has got through `objects`, and whether it has reached the
    /// end. Empty until the sweep first writes a slice.
    /// TODO: remove this table once every database has swept the pre-bucket
    /// backlog, <https://github.com/iotaledger/iota/issues/12712>
    pub(crate) object_backlog_sweep_progress: DBMap<(), ObjectBacklogSweepProgress>,

    /// The last checkpoint the objects pruner of an earlier build reported
    /// having pruned, copied out of `pruned_checkpoint` before that column
    /// family was dropped. Absent on a database no such build ever pruned.
    ///
    /// The pruner wrote it in the same batch as its deletes, so every version
    /// superseded at or below it is already gone and the sweep only has to
    /// look above it. See [`crate::authority::object_backlog_sweep`].
    /// TODO: remove this table once every database has swept the pre-bucket
    /// backlog, <https://github.com/iotaledger/iota/issues/12712>
    pub(crate) object_backlog_sweep_bound: DBMap<(), CheckpointSequenceNumber>,

    /// The last checkpoint whose superseded versions the bounded sweep has
    /// relocated. Empty until that sweep first writes a slice, and unused by
    /// the unbounded walk, which records its place in
    /// `object_backlog_sweep_progress` instead.
    /// TODO: remove this table once every database has swept the pre-bucket
    /// backlog, <https://github.com/iotaledger/iota/issues/12712>
    pub(crate) object_backlog_sweep_checkpoint: DBMap<(), CheckpointSequenceNumber>,

    /// Which of the flat ledger tables the one-time migration into the
    /// per-epoch buckets is draining, and how far through it. Empty until the
    /// migration first writes a slice.
    /// TODO: remove this table once every database has migrated its
    /// pre-bucket ledger history,
    /// <https://github.com/iotaledger/iota/issues/12763>
    pub(crate) ledger_backlog_migration_progress: DBMap<(), LedgerBacklogMigrationProgress>,
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
        Self::open_with_db_options(parent_path, db_options_override).0
    }

    /// The perpetual tables together with the historic object and ledger
    /// buckets. Both bucket sets are column families of this same database,
    /// so they are opened from its handle, with options cloned from the ones
    /// its own tables use.
    pub fn open_with_historic_objects(
        parent_path: &Path,
        db_options_override: Option<AuthorityPerpetualTablesOptions>,
    ) -> Result<(Self, HistoricObjects, HistoricLedger, EpochMarkers), TypedStoreError> {
        let (tables, db_options) = Self::open_with_db_options(parent_path, db_options_override);
        let historic_objects = HistoricObjects::open(
            tables.objects.db.clone(),
            &db_options,
            tables.objects.clone(),
        )?;
        let historic_ledger = HistoricLedger::open(tables.objects.db.clone(), &db_options)?;
        let epoch_markers = EpochMarkers::open(tables.objects.db.clone(), &db_options)?;
        Ok((tables, historic_objects, historic_ledger, epoch_markers))
    }

    /// The perpetual tables and the base options their column families were
    /// opened with. The historic buckets clone these, so they share the base
    /// options' block cache with each other and with every column family that
    /// takes those options unchanged; `objects`, `live_owned_object_markers`,
    /// `transactions` and `effects` install caches of their own.
    fn open_with_db_options(
        parent_path: &Path,
        db_options_override: Option<AuthorityPerpetualTablesOptions>,
    ) -> (Self, DBOptions) {
        let db_options_override = db_options_override.unwrap_or_default();
        let db_options =
            db_options_override.apply_to(default_db_options().optimize_db_for_write_throughput(4));
        let path = Self::path(parent_path);
        let mut table_options = BTreeMap::from([
            (
                "objects".to_string(),
                objects_table_config(db_options.clone()),
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
        ]);
        // The historic object and ledger buckets are column families of this
        // database, so they are opened here together with the tables declared
        // above.
        table_options.extend(HistoricObjects::extra_column_family_options(
            &path,
            &db_options,
        ));
        table_options.extend(HistoricLedger::extra_column_family_options(
            &path,
            &db_options,
        ));
        table_options.extend(EpochMarkers::extra_column_family_options(
            &path,
            &db_options,
        ));
        let table_options = DBMapTableConfigMap::new(table_options);
        let tables = Self::open_tables_read_write(
            path,
            MetricConf::new("perpetual")
                .with_sampling(SamplingInterval::new(Duration::from_secs(60), 0)),
            Some(db_options.options.clone()),
            Some(table_options),
        );
        (tables, db_options)
    }

    pub fn open_readonly(parent_path: &Path) -> AuthorityPerpetualTablesReadOnly {
        Self::get_read_only_handle(
            Self::path(parent_path),
            None,
            None,
            MetricConf::new("perpetual_readonly"),
        )
    }

    /// The newest row for `object_id` at or below `version`, still wrapped.
    ///
    /// A `StoreObject::Value` is a live version; a `Deleted` or `Wrapped`
    /// row means the object was deleted or wrapped at or below the bound,
    /// which is a different answer from `None` — nothing at all in range —
    /// and callers must not collapse the two. Use [`Self::object`] to
    /// resolve a row once the two cases have been told apart.
    pub fn find_object_lt_or_eq_version(
        &self,
        object_id: ObjectId,
        version: Version,
    ) -> Result<Option<(ObjectKey, StoreObjectWrapper)>, IotaError> {
        let mut iter = self.objects.safe_range_iter_reversed(
            ObjectKey::min_for_id(&object_id)..=ObjectKey(object_id, version),
        );
        match iter.next() {
            // Migrate legacy V1 rows before returning; callers inspect the
            // wrapper via `inner()`, which panics on an un-migrated V1.
            Some(Ok((key, o))) => Ok(Some((key, o.migrate()))),
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
    ) -> Result<ObjectReference, IotaError> {
        let obj_ref = match store_object.migrate().into_inner() {
            StoreObject::Value(object) => self.construct_object(object_key, *object)?.object_ref(),
            StoreObject::Deleted => {
                ObjectReference::new(object_key.0, object_key.1, ObjectDigest::OBJECT_DELETED)
            }
            StoreObject::Wrapped => {
                ObjectReference::new(object_key.0, object_key.1, ObjectDigest::OBJECT_WRAPPED)
            }
        };
        Ok(obj_ref)
    }

    pub fn tombstone_reference(
        &self,
        object_key: &ObjectKey,
        store_object: &StoreObjectWrapper,
    ) -> Result<Option<ObjectReference>, IotaError> {
        let obj_ref = match store_object.inner() {
            StoreObject::Deleted => Some(ObjectReference::new(
                object_key.0,
                object_key.1,
                ObjectDigest::OBJECT_DELETED,
            )),
            StoreObject::Wrapped => Some(ObjectReference::new(
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
        object_id: ObjectId,
    ) -> Result<Option<ObjectReference>, IotaError> {
        let mut iterator = self.objects.safe_iter_with_prefix_reversed(&object_id);

        if let Some(Ok((object_key, value))) = iterator.next() {
            if object_key.0 == object_id {
                return Ok(Some(self.object_reference(&object_key, value)?));
            }
        }
        Ok(None)
    }

    pub fn get_latest_object_or_tombstone(
        &self,
        object_id: ObjectId,
    ) -> Result<Option<(ObjectKey, StoreObjectWrapper)>, IotaError> {
        let mut iterator = self.objects.safe_iter_with_prefix_reversed(&object_id);

        if let Some(Ok((object_key, value))) = iterator.next() {
            if object_key.0 == object_id {
                // Migrate legacy V1 rows before returning; callers inspect the
                // wrapper via `inner()`, which panics on an un-migrated V1.
                return Ok(Some((object_key, value.migrate())));
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

    pub fn get_newer_object_keys(
        &self,
        object: &(ObjectId, Version),
    ) -> IotaResult<Vec<ObjectKey>> {
        let mut objects = vec![];
        for result in self.objects.safe_iter_with_prefix_from(
            &object.0,
            std::ops::Bound::Included(&object.1.next().unwrap()),
        ) {
            let (key, _) = result?;
            objects.push(key);
        }
        Ok(objects)
    }

    pub fn database_is_empty(&self) -> IotaResult<bool> {
        Ok(self.objects.safe_iter().next().is_none())
    }

    pub fn iter_live_object_set(&self) -> LiveSetIter<'_> {
        LiveSetIter {
            iter: Box::new(self.objects.safe_iter()),
            tables: self,
            prev: None,
        }
    }

    pub fn range_iter_live_object_set(
        &self,
        lower_bound: Option<ObjectId>,
        upper_bound: Option<ObjectId>,
    ) -> LiveSetIter<'_> {
        let lower_bound = lower_bound.as_ref().map(ObjectKey::min_for_id);
        let upper_bound = upper_bound.as_ref().map(ObjectKey::max_for_id);

        LiveSetIter {
            iter: Box::new(self.objects.safe_iter_with_bounds(lower_bound, upper_bound)),
            tables: self,
            prev: None,
        }
    }

    pub fn checkpoint_db(&self, path: &Path) -> IotaResult {
        // This checkpoints the entire db and not just objects table
        self.objects.checkpoint_db(path).map_err(Into::into)
    }

    /// Compacts the whole key range of the live `objects` table, blocking
    /// until RocksDB has rewritten it.
    pub fn compact(&self) -> Result<(), TypedStoreError> {
        self.objects.compact_range(
            &ObjectKey(ObjectId::ZERO, Version::MIN_VALID_INCL),
            &ObjectKey(ObjectId::MAX, Version::MAX_VALID_EXCL),
        )
    }

    /// The column families whose aged SST files
    /// [`Self::spawn_periodic_compaction`] rewrites: the ones rows are
    /// deleted from. The live `objects` table loses the tombstone heads of a
    /// historic object bucket when that bucket expires; the rest hold the
    /// pre-bucket ledger history, which the one-time migration into the
    /// per-epoch buckets drains.
    fn periodically_compacted_tables(&self) -> BTreeSet<&str> {
        [
            self.objects.cf_name(),
            self.transactions.cf_name(),
            self.effects.cf_name(),
            self.executed_effects.cf_name(),
            self.events_2.cf_name(),
            self.executed_transactions_to_checkpoint.cf_name(),
        ]
        .into_iter()
        .collect()
    }

    /// Compacts the largest SST file that has gone untouched for `delay_days`
    /// and belongs to one of [`Self::periodically_compacted_tables`], and
    /// returns it. `None` when no file qualifies.
    ///
    /// Blocks for as long as the compaction takes, so a caller on an async
    /// runtime must use `spawn_blocking`. `last_processed` carries the files
    /// already compacted from one call to the next, so that the same file is
    /// not picked again within the delay.
    fn compact_next_sst_file(
        &self,
        delay_days: usize,
        last_processed: &Mutex<HashMap<String, SystemTime>>,
    ) -> Result<Option<LiveFile>, anyhow::Error> {
        let compacted_tables = self.periodically_compacted_tables();
        let db_path = self.objects.db.path_for_pruning();
        let mut state = last_processed
            .lock()
            .expect("failed to obtain a lock for last processed SST files");
        let mut sst_file_for_compaction: Option<LiveFile> = None;
        let time_threshold =
            SystemTime::now() - Duration::from_secs(delay_days as u64 * 24 * 60 * 60);
        for sst_file in self.objects.db.live_files()? {
            let file_path = db_path.join(sst_file.name.clone().trim_matches('/'));
            let last_modified = std::fs::metadata(file_path)?.modified()?;
            if !compacted_tables.contains(sst_file.column_family_name.as_str())
                || sst_file.level < 1
                || sst_file.start_key.is_none()
                || sst_file.end_key.is_none()
                || last_modified > time_threshold
                || state.get(&sst_file.name).unwrap_or(&UNIX_EPOCH) > &time_threshold
            {
                continue;
            }
            if let Some(candidate) = &sst_file_for_compaction {
                if candidate.size > sst_file.size {
                    continue;
                }
            }
            sst_file_for_compaction = Some(sst_file);
        }
        let Some(sst_file) = sst_file_for_compaction else {
            return Ok(None);
        };
        info!(
            "Manual compaction of sst file {:?}. Size: {:?}, level: {:?}",
            sst_file.name, sst_file.size, sst_file.level
        );
        self.objects.compact_range_raw(
            &sst_file.column_family_name,
            sst_file.start_key.clone().unwrap(),
            sst_file.end_key.clone().unwrap(),
        )?;
        state.insert(sst_file.name.clone(), SystemTime::now());
        Ok(Some(sst_file))
    }

    /// Spawns a task that keeps compacting SST files older than `delay_days`,
    /// one at a time, until these tables are dropped.
    ///
    /// RocksDB's own background compaction leaves files that stop being
    /// written to alone, so rows deleted from them are never reclaimed
    /// without this.
    pub fn spawn_periodic_compaction(self: &Arc<Self>, delay_days: usize) {
        // The task holds the tables weakly so that it cannot keep a dropped
        // node's database open, and exits once they are gone.
        let perpetual_tables = Arc::downgrade(self);
        spawn_monitored_task!(async move {
            let last_processed = Arc::new(Mutex::new(HashMap::new()));
            loop {
                let Some(tables) = perpetual_tables.upgrade() else {
                    break;
                };
                let state = last_processed.clone();
                let result = tokio::task::spawn_blocking(move || {
                    tables.compact_next_sst_file(delay_days, &state)
                })
                .await;
                let mut sleep_interval_secs = 1;
                match result {
                    Err(err) => error!("Failed to compact sst file: {:?}", err),
                    Ok(Err(err)) => error!("Failed to compact sst file: {:?}", err),
                    Ok(Ok(None)) => {
                        sleep_interval_secs = 3600;
                    }
                    _ => {}
                }
                tokio::time::sleep(Duration::from_secs(sleep_interval_secs)).await;
            }
        });
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

    /// Marks the one-time object-backlog sweep as already done, so that a
    /// later node start does not walk `objects` looking for versions to
    /// relocate.
    ///
    /// Call this only on a database that cannot hold a backlog to begin
    /// with, such as one just populated by a formal-snapshot restore: a
    /// snapshot is taken at an epoch boundary and carries only the live
    /// object set, so there are no superseded versions for the sweep to
    /// find, and recording `Done` up front skips a walk that would relocate
    /// nothing.
    pub fn mark_object_backlog_swept(&self) -> IotaResult {
        self.object_backlog_sweep_progress
            .insert(&(), &ObjectBacklogSweepProgress::Done)?;
        Ok(())
    }

    /// Marks the one-time migration of the flat ledger tables into the
    /// per-epoch buckets as already done, so that a later node start does not
    /// walk them for nothing.
    ///
    /// Call this only on a database that cannot hold pre-bucket ledger rows to
    /// begin with, such as one just populated by a formal-snapshot restore: a
    /// restore writes no ledger row at all, since a snapshot carries the live
    /// object set and the epochs' closing summaries and no transaction
    /// history.
    /// TODO: remove this together with the migration,
    /// <https://github.com/iotaledger/iota/issues/12763>
    pub fn mark_ledger_backlog_migrated(&self) -> IotaResult {
        self.ledger_backlog_migration_progress
            .insert(&(), &LedgerBacklogMigrationProgress::Done)?;
        Ok(())
    }

    pub fn insert_store_object_v1_test_only(&self, object: Object) -> IotaResult {
        use crate::authority::authority_store_types::{StoreObjectV1, StoreObjectValue};

        let object_reference = object.object_ref();
        let v2_value = match get_store_object(object, None).into_inner() {
            StoreObject::Value(v) => *v,
            other => unreachable!("get_store_object must produce a Value variant, got {other:?}"),
        };
        let v1_value = StoreObjectValue {
            data: v2_value.data,
            owner: v2_value.owner,
            previous_transaction: v2_value.previous_transaction,
            storage_rebate: v2_value.storage_rebate,
        };
        let wrapper = StoreObjectWrapper::V1(StoreObjectV1::Value(Box::new(v1_value)));

        let mut wb = self.objects.batch();
        wb.insert_batch(
            &self.objects,
            std::iter::once((ObjectKey::from(object_reference), wrapper)),
        )?;
        wb.write()?;
        Ok(())
    }

    pub fn insert_store_object_v2_test_only(
        &self,
        object: Object,
        previous_transaction_checkpoint: Option<CheckpointSequenceNumber>,
    ) -> IotaResult {
        let object_reference = object.object_ref();
        let wrapper = get_store_object(object, previous_transaction_checkpoint);

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
        object_id: &ObjectId,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        let obj_entry = self
            .objects
            .safe_iter_with_prefix_reversed(object_id)
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
        object_id: &ObjectId,
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

/// In-process iterator item for a live object together with the checkpoint
/// sequence number that contained the transaction whose effects produced this
/// object version. Yielded by [`LiveSetIter`].
///
/// `previous_transaction_checkpoint` is `Option`: production write paths
/// always produce `Some(seq)`, but `LiveSetIter` will yield `None` for rows
/// lifted from a pre-V2 on-disk format (the checkpoint was never recorded and
/// is unrecoverable).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct LiveObject {
    pub object: Object,
    pub previous_transaction_checkpoint: Option<CheckpointSequenceNumber>,
}

impl LiveObject {
    pub fn object_id(&self) -> ObjectId {
        self.object.id()
    }

    pub fn version(&self) -> Version {
        self.object.version()
    }

    pub fn object_reference(&self) -> ObjectReference {
        self.object.object_ref()
    }
}

/// On-disk record format for a live object as emitted into snapshot V2 `.obj`
/// files (`iota-snapshot::writer::write_object`) and decoded by
/// `iota-snapshot::reader::LiveObjectIter`.
#[derive(Deserialize, Serialize)]
pub struct SnapshotLiveObject {
    pub object: Object,
    pub previous_transaction_checkpoint: CheckpointSequenceNumber,
}

impl From<SnapshotLiveObject> for LiveObject {
    fn from(snap: SnapshotLiveObject) -> Self {
        let SnapshotLiveObject {
            object,
            previous_transaction_checkpoint,
        } = snap;
        LiveObject {
            object,
            previous_transaction_checkpoint: Some(previous_transaction_checkpoint),
        }
    }
}

/// Yields the latest live version of every object in range, surfacing a read
/// error instead of ending the scan.
pub struct LiveSetIter<'a> {
    iter: DbIterator<'a, (ObjectKey, StoreObjectWrapper)>,
    tables: &'a AuthorityPerpetualTables,
    prev: Option<(ObjectKey, StoreObjectWrapper)>,
}

impl LiveSetIter<'_> {
    fn store_object_wrapper_to_live_object(
        &self,
        object_key: ObjectKey,
        store_object: StoreObjectWrapper,
    ) -> Option<LiveObject> {
        match store_object.migrate().into_inner() {
            StoreObject::Value(value) => {
                let previous_transaction_checkpoint = value.previous_transaction_checkpoint;
                let object = self
                    .tables
                    .construct_object(&object_key, *value)
                    .expect("Constructing object from store cannot fail");
                Some(LiveObject {
                    object,
                    previous_transaction_checkpoint,
                })
            }
            StoreObject::Wrapped | StoreObject::Deleted => None,
        }
    }
}

impl Iterator for LiveSetIter<'_> {
    type Item = Result<LiveObject, TypedStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter.next() {
                Some(Ok((next_key, next_value))) => {
                    let prev = self.prev.take();
                    self.prev = Some((next_key, next_value));

                    if let Some((prev_key, prev_value)) = prev {
                        if prev_key.0 != next_key.0 {
                            if let Some(live_object) =
                                self.store_object_wrapper_to_live_object(prev_key, prev_value)
                            {
                                return Some(Ok(live_object));
                            }
                        }
                    }
                }
                Some(Err(err)) => {
                    // The buffered row may not be the object's latest version, so drop
                    // it rather than emit it as the tail of a scan that failed.
                    self.prev = None;
                    return Some(Err(err));
                }
                None => {
                    if let Some((key, value)) = self.prev.take() {
                        if let Some(live_object) =
                            self.store_object_wrapper_to_live_object(key, value)
                        {
                            return Some(Ok(live_object));
                        }
                    }
                    return None;
                }
            }
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

fn objects_table_config(db_options: DBOptions) -> DBOptions {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::authority_store_types::StoreObjectV2;

    /// `LiveSetIter` must filter `StoreObject::Wrapped` and
    /// `StoreObject::Deleted` rows at the source so downstream consumers
    /// (snapshot writer, state-hash accumulator, restore path) only ever
    /// observe live objects.
    #[tokio::test]
    async fn live_set_iter_filters_wrapped_and_deleted_store_rows() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        // A live `Normal` row alongside `Wrapped` and `Deleted` tombstones for
        // distinct object IDs.
        let live_id = ObjectId::random();
        let wrapped_id = ObjectId::random();
        let deleted_id = ObjectId::random();

        let live_object = Object::immutable_with_id_for_testing(live_id);
        perpetual_db
            .insert_store_object_v2_test_only(live_object, None)
            .unwrap();

        let mut wb = perpetual_db.objects.batch();
        let wrapped_key = ObjectKey(wrapped_id, Version::from_u64(1));
        wb.insert_batch(
            &perpetual_db.objects,
            std::iter::once::<(ObjectKey, StoreObjectWrapper)>((
                wrapped_key,
                StoreObjectV2::Wrapped.into(),
            )),
        )
        .unwrap();
        let deleted_key = ObjectKey(deleted_id, Version::from_u64(1));
        wb.insert_batch(
            &perpetual_db.objects,
            std::iter::once::<(ObjectKey, StoreObjectWrapper)>((
                deleted_key,
                StoreObjectV2::Deleted.into(),
            )),
        )
        .unwrap();
        wb.write().unwrap();

        let yielded: Vec<_> = perpetual_db
            .iter_live_object_set()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(yielded.len(), 1, "wrapped/deleted rows must be filtered");
        assert_eq!(yielded[0].object.id(), live_id);
    }

    /// `LiveSetIter` must surface the exact `previous_transaction_checkpoint`
    /// stored on `StoreObjectValueV2` - it is the load-bearing input to each
    /// `LiveObject` record the snapshot V2 writer emits into `.obj` files
    /// (and, on restore, to the `previous_transaction_checkpoint` field
    /// stamped onto `StoreObjectV2` via `bulk_insert_live_objects`). A bug
    /// that, e.g., always stamped `0` here would silently corrupt every
    /// snapshot's per-object record; this is the focused canary for that
    /// contract.
    #[tokio::test]
    async fn live_set_iter_propagates_previous_transaction_checkpoint() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        // Insert a live object with a distinct, recognizable checkpoint.
        let object = Object::immutable_with_id_for_testing(ObjectId::random());
        let object_ref = object.object_ref();
        let object_key = ObjectKey::from(object_ref);
        let distinct_checkpoint: u64 = 0xCAFE_F00D_BEEF_1234;

        let store_object_value =
            match get_store_object(object, Some(distinct_checkpoint)).into_inner() {
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

        let yielded: Vec<_> = perpetual_db
            .iter_live_object_set()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(yielded.len(), 1);
        assert_eq!(
            yielded[0].previous_transaction_checkpoint,
            Some(distinct_checkpoint),
            "LiveSetIter must surface the on-row checkpoint, not a default"
        );
    }

    /// A read error part way through the scan must be yielded, not treated as
    /// the end of the live object set.
    #[tokio::test]
    async fn live_set_iter_surfaces_read_errors() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        let object = Object::immutable_with_id_for_testing(ObjectId::random());
        let object_key = ObjectKey::from(object.object_ref());
        let rows = vec![
            Ok((object_key, get_store_object(object, Some(1)))),
            Err(TypedStoreError::RocksDB(
                "injected read failure".to_string(),
            )),
        ];

        let yielded: Vec<_> = LiveSetIter {
            iter: Box::new(rows.into_iter()),
            tables: &perpetual_db,
            prev: None,
        }
        .collect();

        assert_eq!(yielded.len(), 1);
        assert!(
            yielded[0].is_err(),
            "a failed read must not be reported as a complete scan"
        );
    }

    /// A legacy V1 row (written by a pre-V2 binary, e.g. restored from a V1
    /// formal snapshot) must be migrated to the latest version at the read
    /// boundary. `get_latest_object_or_tombstone` feeds its result to
    /// `tombstone_reference`, which reaches `StoreObjectWrapper::inner()` and
    /// panics on an un-migrated V1 wrapper.
    #[tokio::test]
    async fn get_latest_object_or_tombstone_migrates_legacy_v1_row() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        let object_id = ObjectId::random();
        let object = Object::immutable_with_id_for_testing(object_id);
        let object_ref = object.object_ref();
        perpetual_db
            .insert_store_object_v1_test_only(object)
            .unwrap();

        let (object_key, wrapper) = perpetual_db
            .get_latest_object_or_tombstone(object_id)
            .unwrap()
            .expect("row must be found");
        assert!(
            matches!(wrapper, StoreObjectWrapper::V2(_)),
            "read boundary must migrate the V1 row to V2"
        );

        // Both consumers of the returned wrapper must run without panicking.
        assert!(
            perpetual_db
                .tombstone_reference(&object_key, &wrapper)
                .unwrap()
                .is_none(),
            "a live value is not a tombstone"
        );
        let reconstructed = perpetual_db
            .object(&object_key, wrapper)
            .unwrap()
            .expect("value must reconstruct");
        assert_eq!(reconstructed.object_ref(), object_ref);
    }

    /// A formal-snapshot restore has no backlog of superseded versions to
    /// walk, so it records the sweep as done directly rather than paying for
    /// a walk over the live object set it just wrote.
    /// The hook must carry the objects pruner's watermark into the table the
    /// sweep reads, since the column family it was written to is dropped on
    /// the same open. Without it the sweep loses its bound and falls back to
    /// walking the whole live table.
    #[tokio::test]
    async fn the_objects_pruner_watermark_is_carried_over() {
        let tmp_dir = iota_common::tempdir();
        let db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        // Stand where a database written by a build with the pruner does.
        db.objects
            .db
            .create_cf(
                "pruned_checkpoint",
                &typed_store::rocksdb::Options::default(),
            )
            .unwrap();
        let pruned: DBMap<(), CheckpointSequenceNumber> = DBMap::reopen(
            &db.objects.db,
            Some("pruned_checkpoint"),
            &ReadWriteOptions::default(),
            true,
        )
        .unwrap();
        pruned.insert(&(), &4_242).unwrap();
        assert_eq!(db.object_backlog_sweep_bound.get(&()).unwrap(), None);

        rescue_objects_pruner_watermark(&db.objects.db).unwrap();

        assert_eq!(db.object_backlog_sweep_bound.get(&()).unwrap(), Some(4_242));
    }

    /// A database no such build ever pruned has no watermark to carry, and
    /// the hook leaves the sweep to walk the whole table.
    #[tokio::test]
    async fn no_watermark_is_carried_over_when_the_pruner_never_ran() {
        let tmp_dir = iota_common::tempdir();
        let db = AuthorityPerpetualTables::open(tmp_dir.path(), None);
        db.objects
            .db
            .create_cf(
                "pruned_checkpoint",
                &typed_store::rocksdb::Options::default(),
            )
            .unwrap();

        rescue_objects_pruner_watermark(&db.objects.db).unwrap();

        assert_eq!(db.object_backlog_sweep_bound.get(&()).unwrap(), None);
    }

    #[tokio::test]
    async fn mark_object_backlog_swept_records_done() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        assert_eq!(
            perpetual_db.object_backlog_sweep_progress.get(&()).unwrap(),
            None
        );

        perpetual_db.mark_object_backlog_swept().unwrap();

        assert_eq!(
            perpetual_db.object_backlog_sweep_progress.get(&()).unwrap(),
            Some(ObjectBacklogSweepProgress::Done)
        );
    }

    /// [`AuthorityPerpetualTables::compact`] must let RocksDB reclaim the
    /// space of deleted object versions, so that a caller that has just
    /// removed rows can shrink the database on demand.
    #[cfg(not(target_env = "msvc"))]
    #[tokio::test]
    async fn compact_reclaims_the_space_of_deleted_object_versions() {
        fn sst_size(path: &Path) -> u64 {
            let mut size = 0;
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|ext| ext == "sst") {
                    size += std::fs::metadata(path).unwrap().len();
                }
            }
            size
        }

        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);
        let total_unique_object_ids = 10_000;
        let num_versions_per_object = 10;
        let mut id = ObjectId::ZERO;
        let mut to_delete = vec![];
        for _ in 0..total_unique_object_ids {
            for i in (0..num_versions_per_object).rev() {
                if i < num_versions_per_object - 2 {
                    to_delete.push(ObjectKey(id, Version::from(i)));
                }
                let object = get_store_object(Object::immutable_with_id_for_testing(id), None);
                perpetual_db
                    .objects
                    .insert(&ObjectKey(id, Version::from(i)), &object)
                    .unwrap();
            }
            id = id.next_lexicographical();
        }

        let db_path = tmp_dir.path().join("perpetual");
        perpetual_db.compact().unwrap();
        let before_compaction_size = sst_size(&db_path);

        let mut batch = perpetual_db.objects.batch();
        batch
            .delete_batch(&perpetual_db.objects, to_delete.into_iter())
            .unwrap();
        batch.write().unwrap();

        perpetual_db.compact().unwrap();
        let after_compaction_size = sst_size(&db_path);

        more_asserts::assert_lt!(after_compaction_size, before_compaction_size);
    }
}
