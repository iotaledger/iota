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
use iota_sdk_types::Version;
use iota_types::global_state_hash::GlobalStateHash;
use serde::{Deserialize, Serialize};
pub use typed_store::DbSnapshot;
use typed_store::{
    DBMapUtils, DbIterator,
    metrics::SamplingInterval,
    rocks::{
        DBMap, DBMapTableConfigMap, DBOptions, MetricConf, default_db_options, read_size_from_env,
    },
    rocksdb::LiveFile,
    traits::Map,
};

use super::*;
use crate::{
    authority::{
        authority_store_types::{
            StoreObject, StoreObjectValueV2, StoreObjectWrapper, get_store_object,
            try_construct_object,
        },
        epoch_markers::EpochMarkers,
        epoch_start_configuration::EpochStartConfiguration,
        historic_ledger::HistoricLedger,
        historic_objects::HistoricObjects,
    },
    epoch_buckets::historic_root,
};

const ENV_VAR_OBJECTS_BLOCK_CACHE_SIZE: &str = "OBJECTS_BLOCK_CACHE_MB";
pub(crate) const ENV_VAR_LOCKS_BLOCK_CACHE_SIZE: &str = "LOCKS_BLOCK_CACHE_MB";
const ENV_VAR_TRANSACTIONS_BLOCK_CACHE_SIZE: &str = "TRANSACTIONS_BLOCK_CACHE_MB";
const ENV_VAR_EFFECTS_BLOCK_CACHE_SIZE: &str = "EFFECTS_BLOCK_CACHE_MB";

/// Options to apply to every column family of the `perpetual` DB.
#[derive(Default)]
pub struct AuthorityPerpetualTablesOptions {
    /// Whether to enable write stalling on all column families.
    pub enable_write_stall: bool,
    /// The directory the historic object and ledger buckets keep their files
    /// in. Anything opening a node's database must pass the same path the
    /// node does; unset, it is
    /// [`HISTORIC_DB_DIR`](crate::epoch_buckets::HISTORIC_DB_DIR) in the
    /// perpetual database directory.
    pub historic_db_path: Option<PathBuf>,
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
    /// Superseded versions are moved into the [`HistoricObjects`] buckets
    /// and expire with them; see [`HistoricObjects::prune`].
    pub(crate) objects: DBMap<ObjectKey, StoreObjectWrapper>,

    /// Object references of currently active objects that can be mutated.
    pub(crate) live_owned_object_markers: DBMap<ObjectReference, ()>,

    /// The flat ledger tables [`HistoricLedger`] replaced; dropped when the
    /// database is opened.
    // TODO(https://github.com/iotaledger/iota/issues/13576): remove these tables.
    #[rename = "transactions"]
    #[deprecated_db_map]
    _transactions: Option<DBMap<(), ()>>,
    #[rename = "effects"]
    #[deprecated_db_map]
    _effects: Option<DBMap<(), ()>>,
    #[rename = "executed_effects"]
    #[deprecated_db_map]
    _executed_effects: Option<DBMap<(), ()>>,
    #[rename = "events_2"]
    #[deprecated_db_map]
    _events_2: Option<DBMap<(), ()>>,
    #[rename = "executed_transactions_to_checkpoint"]
    #[deprecated_db_map]
    _executed_transactions_to_checkpoint: Option<DBMap<(), ()>>,

    // Finalized root state hash for epoch, to be included in CheckpointSummary
    // of last checkpoint of epoch. These values should only ever be written once
    // and never changed
    pub(crate) root_state_hash_by_epoch:
        DBMap<EpochId, (CheckpointSequenceNumber, GlobalStateHash)>,

    /// Parameters of the system fixed at the epoch start
    pub(crate) epoch_start_configuration: DBMap<(), EpochStartConfiguration>,

    /// The total IOTA supply and the epoch at which it was stored.
    /// We check and update it at the end of each epoch if expensive checks are
    /// enabled.
    pub(crate) total_iota_supply: DBMap<(), TotalIotaSupplyCheck>,

    /// Expected imbalance between storage fund balance and the sum of storage
    /// rebate of all live objects. This could be non-zero due to bugs in
    /// earlier protocol versions. This number is the result of
    /// storage_fund_balance - sum(storage_rebate).
    pub(crate) expected_storage_fund_imbalance: DBMap<(), i64>,

    /// Tables of the one-time move into the per-epoch buckets; dropped when
    /// the database is opened.
    // TODO(https://github.com/iotaledger/iota/issues/13576): remove these tables.
    #[rename = "object_per_epoch_marker_table"]
    #[deprecated_db_map]
    _object_per_epoch_marker_table: Option<DBMap<(), ()>>,
    #[rename = "object_backlog_sweep_progress"]
    #[deprecated_db_map]
    _object_backlog_sweep_progress: Option<DBMap<(), ()>>,
    #[rename = "object_backlog_sweep_bound"]
    #[deprecated_db_map]
    _object_backlog_sweep_bound: Option<DBMap<(), ()>>,
    #[rename = "object_backlog_sweep_epoch"]
    #[deprecated_db_map]
    _object_backlog_sweep_epoch: Option<DBMap<(), ()>>,
    #[rename = "ledger_backlog_migration_progress"]
    #[deprecated_db_map]
    _ledger_backlog_migration_progress: Option<DBMap<(), ()>>,
    #[rename = "ledger_backlog_migration_floor"]
    #[deprecated_db_map]
    _ledger_backlog_migration_floor: Option<DBMap<(), ()>>,
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

    /// Fails if the perpetual database under `parent_path` was last written by
    /// a release from before the per-epoch history buckets (1.36). Such a
    /// release keeps its history in flat tables, which this release no longer
    /// reads and drops when it opens the database. A missing database passes.
    ///
    /// Call this before opening the database read-write.
    pub fn ensure_per_epoch_layout(parent_path: &Path) -> IotaResult {
        let path = Self::path(parent_path);
        if !path.join("CURRENT").exists() {
            return Ok(());
        }
        let tables = typed_store::rocks::list_tables(path).map_err(|e| {
            IotaError::Storage(format!(
                "cannot list the column families of the perpetual store: {e}"
            ))
        })?;
        // The first start with the per-epoch history buckets drops this column
        // family before it moves the history into the buckets.
        if tables.iter().any(|table| table == "pruned_checkpoint") {
            return Err(IotaError::Storage(format!(
                "the database under {} was last written by a release from before the \
                 per-epoch history buckets (1.36); start it with a 1.36 release until the \
                 node serves, then upgrade",
                parent_path.display()
            )));
        }
        Ok(())
    }

    pub fn open(
        parent_path: &Path,
        db_options_override: Option<AuthorityPerpetualTablesOptions>,
    ) -> Self {
        Self::open_with_db_options(parent_path, db_options_override).0
    }

    /// Opens the perpetual tables together with the historic object, ledger
    /// and epoch marker buckets, which are column families of the same
    /// database.
    pub fn open_with_historic_objects(
        parent_path: &Path,
        db_options_override: Option<AuthorityPerpetualTablesOptions>,
    ) -> Result<(Self, HistoricObjects, HistoricLedger, EpochMarkers), TypedStoreError> {
        let (tables, db_options, ledger_db_options, historic_root) =
            Self::open_with_db_options(parent_path, db_options_override);
        let historic_objects = HistoricObjects::open(
            tables.objects.db.clone(),
            &db_options,
            tables.objects.clone(),
            &historic_root,
        )?;
        let historic_ledger = HistoricLedger::open(
            tables.objects.db.clone(),
            &ledger_db_options,
            &historic_root,
        )?;
        let epoch_markers = EpochMarkers::open(tables.objects.db.clone(), &db_options)?;
        Ok((tables, historic_objects, historic_ledger, epoch_markers))
    }

    /// The perpetual tables, the base options their column families were
    /// opened with, the base options of the historic ledger buckets, and the
    /// root of the historic buckets.
    fn open_with_db_options(
        parent_path: &Path,
        db_options_override: Option<AuthorityPerpetualTablesOptions>,
    ) -> (Self, DBOptions, DBOptions, PathBuf) {
        let db_options_override = db_options_override.unwrap_or_default();
        let db_options =
            db_options_override.apply_to(default_db_options().optimize_db_for_write_throughput(4));
        let path = Self::path(parent_path);
        let historic_root = historic_root(&path, db_options_override.historic_db_path.as_deref());
        let ledger_db_options = historic_ledger_db_options(&db_options);
        let table_options =
            Self::table_options(&path, &db_options, &ledger_db_options, &historic_root);
        let tables = Self::open_tables_read_write(
            path,
            MetricConf::new("perpetual")
                .with_sampling(SamplingInterval::new(Duration::from_secs(60), 0)),
            Some(db_options.options.clone()),
            Some(table_options),
        );
        (tables, db_options, ledger_db_options, historic_root)
    }

    /// The options of the column families of the perpetual database at
    /// `path` that do not take `db_options` unchanged, including the historic
    /// buckets. The historic ledger buckets take `ledger_db_options`, from
    /// [`historic_ledger_db_options`]. Every open of the database must pass
    /// these.
    fn table_options(
        path: &Path,
        db_options: &DBOptions,
        ledger_db_options: &DBOptions,
        historic_root: &Path,
    ) -> DBMapTableConfigMap {
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
        table_options.extend(HistoricObjects::extra_column_family_options(
            path,
            db_options,
            historic_root,
        ));
        table_options.extend(HistoricLedger::extra_column_family_options(
            path,
            ledger_db_options,
            historic_root,
        ));
        table_options.extend(EpochMarkers::extra_column_family_options(path, db_options));
        DBMapTableConfigMap::new(table_options)
    }

    /// Opens the tables read-only. `historic_db_path` is the root of the
    /// historic buckets, as for [`AuthorityPerpetualTablesOptions`].
    pub fn open_readonly(
        parent_path: &Path,
        historic_db_path: Option<&Path>,
    ) -> AuthorityPerpetualTablesReadOnly {
        let path = Self::path(parent_path);
        let db_options = default_db_options().optimize_db_for_write_throughput(4);
        let table_options = Self::table_options(
            &path,
            &db_options,
            &historic_ledger_db_options(&db_options),
            &historic_root(&path, historic_db_path),
        );
        Self::get_read_only_handle_with_table_options(
            path,
            None,
            None,
            MetricConf::new("perpetual_readonly"),
            table_options,
        )
    }

    /// The newest row for `object_id` at or below `version`, still wrapped.
    ///
    /// A `Deleted` or `Wrapped` row means the object was deleted or wrapped
    /// at or below the bound, which callers must not treat like `None` (no
    /// row in range). Use [`Self::object`] to resolve a row.
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
        match iterator.next().transpose()? {
            Some((object_key, value)) if object_key.0 == object_id => {
                Ok(Some(self.object_reference(&object_key, value)?))
            }
            _ => Ok(None),
        }
    }

    pub fn get_latest_object_or_tombstone(
        &self,
        object_id: ObjectId,
    ) -> Result<Option<(ObjectKey, StoreObjectWrapper)>, IotaError> {
        let mut iterator = self.objects.safe_iter_with_prefix_reversed(&object_id);
        match iterator.next().transpose()? {
            // Migrate legacy V1 rows before returning; callers inspect the
            // wrapper via `inner()`, which panics on an un-migrated V1.
            Some((object_key, value)) if object_key.0 == object_id => {
                Ok(Some((object_key, value.migrate())))
            }
            _ => Ok(None),
        }
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

    /// A point-in-time snapshot of the perpetual store, for a scan that must
    /// see one state while the node keeps executing. See [`DbSnapshot`] for
    /// what holding one costs.
    pub fn db_snapshot(&self) -> DbSnapshot<'_> {
        self.objects.db.snapshot()
    }

    /// [`Self::iter_live_object_set`] reading as of `db_snapshot`.
    pub fn iter_live_object_set_at<'a>(
        &'a self,
        db_snapshot: &'a DbSnapshot<'a>,
    ) -> LiveSetIter<'a> {
        LiveSetIter {
            iter: Box::new(self.objects.safe_iter_at_snapshot(db_snapshot)),
            tables: self,
            prev: None,
        }
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
    /// deleted from.
    fn periodically_compacted_tables(&self) -> BTreeSet<&str> {
        [self.objects.cf_name()].into_iter().collect()
    }

    /// Compacts and returns the largest SST file of
    /// [`Self::periodically_compacted_tables`] untouched for `delay_days`, or
    /// `None` when no file qualifies. `last_processed` must be carried from one
    /// call to the next so that a file is not picked again within the delay.
    ///
    /// Blocks until the compaction finishes, so an async caller must use
    /// `spawn_blocking`.
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
            // Checked first: the historic buckets keep their files outside
            // `db_path`.
            if !compacted_tables.contains(sst_file.column_family_name.as_str()) {
                continue;
            }
            let file_path = db_path.join(sst_file.name.clone().trim_matches('/'));
            let last_modified = std::fs::metadata(file_path)?.modified()?;
            if sst_file.level < 1
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
    /// one at a time, until these tables are dropped. RocksDB does not compact
    /// files that are no longer written to, so their deleted rows are not
    /// reclaimed otherwise.
    pub fn spawn_periodic_compaction(self: &Arc<Self>, delay_days: usize) {
        // Held weakly so the task cannot keep a dropped node's database open.
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

    /// The object references that have a live owned object marker.
    pub fn live_owned_object_markers_test_only(&self) -> IotaResult<Vec<ObjectReference>> {
        Ok(self
            .live_owned_object_markers
            .safe_iter()
            .map(|marker| marker.map(|(object_ref, ())| object_ref))
            .collect::<Result<_, _>>()?)
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

/// `db_options` with one block cache that every historic ledger bucket opened
/// from them shares, as large as the transactions and effects caches together,
/// since the buckets hold what those tables held. Build them once per open of
/// the database: each call makes a new cache.
fn historic_ledger_db_options(db_options: &DBOptions) -> DBOptions {
    db_options.clone().set_block_options(
        read_size_from_env(ENV_VAR_TRANSACTIONS_BLOCK_CACHE_SIZE).unwrap_or(512)
            + read_size_from_env(ENV_VAR_EFFECTS_BLOCK_CACHE_SIZE).unwrap_or(1024),
        16 << 10,
    )
}

#[cfg(test)]
mod tests {
    use typed_store::rocks::ReadWriteOptions;

    use super::*;
    use crate::authority::authority_store_types::StoreObjectV2;

    /// A scan through a database snapshot sees the object set as it stood
    /// when the snapshot was taken, not writes made afterwards.
    #[tokio::test]
    async fn a_db_snapshot_scan_is_unchanged_by_later_writes() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        let kept_id = ObjectId::random();
        let deleted_id = ObjectId::random();
        for id in [kept_id, deleted_id] {
            perpetual_db
                .insert_store_object_v2_test_only(
                    Object::immutable_with_id_for_testing(id),
                    Some(0),
                )
                .unwrap();
        }
        let live_set = |tables: &AuthorityPerpetualTables| -> Vec<(ObjectId, Version)> {
            tables
                .iter_live_object_set()
                .map(|live| live.map(|live| (live.object_id(), live.version())))
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let before = live_set(&perpetual_db);
        assert_eq!(before.len(), 2);

        let db_snapshot = perpetual_db.db_snapshot();

        // What a running node does to the table while a scan is in flight: a
        // new object appears, and another is tombstoned.
        perpetual_db
            .insert_store_object_v2_test_only(
                Object::immutable_with_id_for_testing(ObjectId::random()),
                Some(1),
            )
            .unwrap();
        let mut wb = perpetual_db.objects.batch();
        wb.insert_batch(
            &perpetual_db.objects,
            std::iter::once::<(ObjectKey, StoreObjectWrapper)>((
                ObjectKey(deleted_id, Version::from_u64(2)),
                StoreObjectV2::Deleted.into(),
            )),
        )
        .unwrap();
        wb.write().unwrap();

        let through_view: Vec<_> = perpetual_db
            .iter_live_object_set_at(&db_snapshot)
            .map(|live| live.map(|live| (live.object_id(), live.version())))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            through_view, before,
            "the snapshot must not show writes made after it was taken"
        );

        // Without this the test would also pass against a snapshot that saw
        // everything, or one that saw nothing.
        assert_ne!(
            live_set(&perpetual_db),
            before,
            "a scan outside the snapshot must show the later writes"
        );
    }

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

    /// A database a release from before the per-epoch history buckets wrote
    /// last is refused; one this release opened and a missing one are not.
    #[tokio::test]
    async fn only_a_database_in_the_per_epoch_layout_is_accepted() {
        let old = tempfile::tempdir().unwrap();
        let mut options = typed_store::rocksdb::Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        typed_store::rocksdb::DB::open_cf(
            &options,
            AuthorityPerpetualTables::path(old.path()),
            ["objects", "pruned_checkpoint"],
        )
        .unwrap();
        assert!(AuthorityPerpetualTables::ensure_per_epoch_layout(old.path()).is_err());

        let current = tempfile::tempdir().unwrap();
        drop(AuthorityPerpetualTables::open(current.path(), None));
        AuthorityPerpetualTables::ensure_per_epoch_layout(current.path()).unwrap();

        let missing = tempfile::tempdir().unwrap().path().join("missing");
        AuthorityPerpetualTables::ensure_per_epoch_layout(&missing).unwrap();
    }

    /// A newest row that does not decode is a storage error, not a missing
    /// object and not the older version below it.
    #[tokio::test]
    async fn latest_object_lookups_return_the_row_decode_error() {
        let tmp_dir = iota_common::tempdir();
        let perpetual_db = AuthorityPerpetualTables::open(tmp_dir.path(), None);

        let object_id = ObjectId::random();
        let older = Object::immutable_with_id_for_testing(object_id);
        let corrupt_version = Version::from_u64(7);
        assert!(older.version() < corrupt_version);
        perpetual_db
            .insert_store_object_v2_test_only(older, Some(0))
            .unwrap();
        // The objects table's own keys, with bytes that are no
        // `StoreObjectWrapper`.
        DBMap::<ObjectKey, Vec<u8>>::reopen(
            &perpetual_db.objects.db,
            Some(perpetual_db.objects.cf_name()),
            &ReadWriteOptions::default(),
            true,
        )
        .unwrap()
        .insert(&ObjectKey(object_id, corrupt_version), &vec![u8::MAX; 8])
        .unwrap();

        let error = perpetual_db
            .get_latest_object_ref_or_tombstone(object_id)
            .expect_err("a newest row that does not decode must not read as missing or older");
        assert!(matches!(error, IotaError::Storage(_)), "{error:?}");
        let error = perpetual_db
            .get_latest_object_or_tombstone(object_id)
            .expect_err("a newest row that does not decode must not read as missing or older");
        assert!(matches!(error, IotaError::Storage(_)), "{error:?}");
    }

    /// [`AuthorityPerpetualTables::compact`] reclaims the space of deleted
    /// object versions.
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

    /// The periodic compaction skips the historic bucket files, which live
    /// outside the perpetual database directory, instead of failing on them.
    #[tokio::test]
    async fn periodic_compaction_passes_over_the_historic_bucket_files() {
        let tmp_dir = iota_common::tempdir();
        let (perpetual_db, historic_objects, _historic_ledger, _epoch_markers) =
            AuthorityPerpetualTables::open_with_historic_objects(tmp_dir.path(), None).unwrap();
        let object = Object::immutable_with_id_for_testing(ObjectId::random());
        let bucket = historic_objects.ensure(1).unwrap();
        let mut batch = perpetual_db.objects.batch();
        batch
            .insert_batch_tagged(
                &bucket.objects,
                [(ObjectKey(object.id(), object.version()), object)],
            )
            .unwrap();
        batch.write().unwrap();
        perpetual_db.objects.flush_all().unwrap();

        perpetual_db
            .compact_next_sst_file(0, &Mutex::new(HashMap::new()))
            .unwrap();
    }
}
