// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Superseded object versions, bucketed by the epoch that superseded them.
//!
//! The buckets are extra column families of the perpetual database rather
//! than a store of their own (see [`crate::epoch_buckets`]), so relocating a
//! version out of the live `objects` table and in here is one atomic
//! [`typed_store::rocks::DBBatch`] instead of a cross-database move.

use std::{collections::BTreeMap, fmt::Debug, path::Path, sync::Arc};

use iota_sdk_types::{TransactionDigest, TransactionEffects};
use iota_types::{
    committee::EpochId,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult},
    object::Object,
    storage::{ObjectKey, ObjectStore},
};
use typed_store::{
    DbIterator, TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, TaggedDBMap, list_tables},
    traits::Map,
};

use crate::epoch_buckets::{EpochBuckets, bucket_cf_epoch};

/// Column-family prefix of the historic object buckets; a bucket's family
/// is `{prefix}{epoch}`.
const HISTORIC_OBJECTS_CF_PREFIX: &str = "hist_obj_e";

/// Tag of the relocated-versions table inside a bucket's column family.
/// Do not reuse a tag for a different table: mark it retired in a comment
/// instead, so an older bucket's rows can never be read as the wrong type.
const DB_PREFIX_HISTORIC_OBJECTS: u8 = 0;

/// Tag of the tombstone-head table inside a bucket's column family.
const DB_PREFIX_HISTORIC_TOMBSTONES: u8 = 1;

/// Tag of the expiring marker inside a bucket's column family.
const DB_PREFIX_HISTORIC_EXPIRING: u8 = 2;

/// Column family holding the earliest-retained-epoch marker `EpochBuckets`
/// would persist on a prune. Nothing prunes yet, so this column family stays
/// empty and every bucket is retained.
///
/// The name must not begin with [`HISTORIC_OBJECTS_CF_PREFIX`], since that is
/// how a bucket's column family is told from every other one in this
/// database.
const EARLIEST_RETAINED_CF: &str = "hist_obj_retention";

/// One epoch's relocated object versions.
pub struct HistoricObjectsBucket {
    /// Object versions superseded during this epoch, keyed exactly as the
    /// live `objects` table keys them.
    pub(crate) objects: TaggedDBMap<ObjectKey, Object>,

    /// The objects deleted or wrapped during this epoch. Their tombstones
    /// stay in the live `objects` table until this bucket expires: a
    /// tombstone has to outlive every version beneath it, and a tombstone
    /// written in this epoch can only sit above versions relocated in this
    /// epoch or an earlier one.
    pub(crate) tombstones: TaggedDBMap<ObjectKey, ()>,

    /// Present once this bucket has been scheduled for expiry. A bucket
    /// carrying it is skipped by reads and its expiry is resumed at open.
    pub(crate) expiring: TaggedDBMap<(), ()>,
}

impl HistoricObjectsBucket {
    fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
        Ok(Self {
            objects: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_OBJECTS,
                &ReadWriteOptions::default(),
                true,
            )?,
            tombstones: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_TOMBSTONES,
                &ReadWriteOptions::default(),
                true,
            )?,
            expiring: TaggedDBMap::reopen(
                db,
                cf_name,
                DB_PREFIX_HISTORIC_EXPIRING,
                &ReadWriteOptions::default(),
                true,
            )?,
        })
    }
}

/// Superseded object versions, bucketed by the epoch that superseded them.
///
/// The buckets are column families of the perpetual database rather than a
/// store of their own, so a version can leave `objects` and arrive here in
/// one atomic batch.
pub struct HistoricObjects {
    buckets: EpochBuckets<HistoricObjectsBucket>,
}

impl HistoricObjects {
    /// Options for a historic-object bucket's column family: written once,
    /// while the epoch that relocated its rows is current, then only ever
    /// read back by exact-key lookup.
    ///
    /// `db_options` are the perpetual database's base options. Build this
    /// once and clone it per column family: the clones share the base
    /// options' block cache, the same one every column family that takes
    /// those options unchanged uses, whereas a fresh value per bucket would
    /// allocate a cache each.
    fn cf_options(db_options: &DBOptions) -> DBOptions {
        db_options
            .clone()
            .optimize_for_write_throughput_no_deletion()
    }

    /// The `(name, options)` pairs of the column families this store needs,
    /// for the perpetual store's open path to list alongside its own tables:
    /// a column family left for auto-discovery would otherwise be reopened
    /// with default options and a block cache of its own. The buckets already
    /// on disk under `perpetual_path` are listed together with the
    /// retention-floor column family, which the open path creates when it is
    /// missing.
    ///
    /// A path with no database yet, or one whose column families cannot be
    /// listed, yields the retention floor alone — the perpetual store's own
    /// open then either creates the database fresh or fails on the same
    /// listing problem.
    pub fn extra_column_family_options(
        perpetual_path: &Path,
        db_options: &DBOptions,
    ) -> Vec<(String, DBOptions)> {
        let cf_options = Self::cf_options(db_options);
        let mut options = vec![(EARLIEST_RETAINED_CF.to_string(), cf_options.clone())];
        if !perpetual_path.join("CURRENT").exists() {
            return options;
        }
        let Ok(existing_cfs) = list_tables(perpetual_path.to_path_buf()) else {
            return options;
        };
        options.extend(
            existing_cfs
                .into_iter()
                .filter(|name| bucket_cf_epoch(HISTORIC_OBJECTS_CF_PREFIX, name).is_some())
                .map(|name| (name, cf_options.clone())),
        );
        options
    }

    /// Opens the historic-object buckets already present among `db`'s
    /// column families. `db` is the perpetual database's own handle: the
    /// buckets are its column families, not a database of their own, and
    /// `db_options` are the options its tables were opened with.
    pub fn open(db: Arc<Database>, db_options: &DBOptions) -> Result<Self, TypedStoreError> {
        let existing_cfs = list_tables(db.path_for_pruning().to_path_buf())
            .map_err(|e| TypedStoreError::RocksDB(format!("failed to list buckets: {e}")))?;

        let mut buckets = BTreeMap::new();
        for cf_name in &existing_cfs {
            if let Some(epoch) = bucket_cf_epoch(HISTORIC_OBJECTS_CF_PREFIX, cf_name) {
                buckets.insert(
                    epoch,
                    Arc::new(HistoricObjectsBucket::reopen(&db, cf_name)?),
                );
            }
        }

        let cf_options = Self::cf_options(db_options).options;
        if db.cf_handle(EARLIEST_RETAINED_CF).is_none() {
            db.create_cf(EARLIEST_RETAINED_CF, &cf_options)?;
        }
        let earliest_retained_table: DBMap<(), EpochId> = DBMap::reopen(
            &db,
            Some(EARLIEST_RETAINED_CF),
            &ReadWriteOptions::default(),
            true,
        )?;

        let buckets = EpochBuckets::open(
            db,
            "historic objects",
            HISTORIC_OBJECTS_CF_PREFIX,
            cf_options,
            earliest_retained_table,
            buckets,
            HistoricObjectsBucket::reopen,
        )?;
        Ok(Self { buckets })
    }

    /// The bucket holding `epoch`'s relocated versions, created if absent.
    pub fn ensure(&self, epoch: EpochId) -> IotaResult<Arc<HistoricObjectsBucket>> {
        self.buckets
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The object relocated under `key`, probed newest-epoch bucket first,
    /// `None` if it was never relocated (or its bucket has since been
    /// dropped).
    pub fn get(&self, key: &ObjectKey) -> IotaResult<Option<Object>> {
        for bucket in self.buckets.iter(true) {
            if let Some(object) = bucket
                .objects
                .get(key)
                .map_err(|e| IotaError::Storage(e.to_string()))?
            {
                return Ok(Some(object));
            }
        }
        Ok(None)
    }

    /// Fills each `None` in `objects` — the live table's answers for `keys`, in
    /// the same order — with the version the buckets hold under that key.
    ///
    /// Only for reads that serve a response: consensus and execution read
    /// current versions, which never leave the live table, so a miss there is a
    /// bug and must stay one.
    ///
    /// The caller reads the live table itself, so it can use
    /// [`crate::execution_cache::ObjectCacheRead`]'s batched multi-get rather
    /// than [`iota_types::storage::ObjectStore`]'s per-key one.
    pub fn fill_missing(
        &self,
        keys: &[ObjectKey],
        objects: &mut [Option<Object>],
    ) -> IotaResult<()> {
        for (object, key) in objects.iter_mut().zip(keys) {
            if object.is_none() {
                *object = self.get(key)?;
            }
        }
        Ok(())
    }

    /// One page of the rows `cf_name` holds, if it is one of this store's
    /// column families: a bucket of relocated versions, or the
    /// retention-floor family. `None` for any other name, leaving the caller
    /// to report it as unknown.
    ///
    /// A bucket packs its tombstone heads and its expiring marker into the
    /// same column family as its relocated versions, tagged apart; the page
    /// carries all three, the tombstone and marker rows prefixed by table
    /// name to keep them apart from an object key formatted the same way.
    ///
    /// For the table dump of `iota-tool`, which walks the perpetual
    /// database's column families by name: these are not fields of
    /// `AuthorityPerpetualTables`, so the dump derived from it cannot read
    /// them. `db` may be a read-only or secondary handle — nothing here
    /// writes, and no column family is created.
    pub fn dump_column_family(
        db: &Arc<Database>,
        cf_name: &str,
        page_size: u16,
        page_number: usize,
    ) -> Result<Option<BTreeMap<String, String>>, TypedStoreError> {
        fn format_rows<'a, K: Debug + 'a, V: Debug + 'a>(
            prefix: &'static str,
            rows: DbIterator<'a, (K, V)>,
        ) -> impl Iterator<Item = Result<(String, String), TypedStoreError>> + 'a {
            rows.map(move |row| {
                row.map(|(key, value)| (format!("{prefix}{key:?}"), format!("{value:?}")))
            })
        }

        fn page(
            rows: impl Iterator<Item = Result<(String, String), TypedStoreError>>,
            page_size: u16,
            page_number: usize,
        ) -> Result<BTreeMap<String, String>, TypedStoreError> {
            rows.skip(page_number * page_size as usize)
                .take(page_size as usize)
                .collect()
        }

        if bucket_cf_epoch(HISTORIC_OBJECTS_CF_PREFIX, cf_name).is_some() {
            let bucket = HistoricObjectsBucket::reopen(db, cf_name)?;
            bucket.objects.try_catch_up_with_primary()?;
            bucket.tombstones.try_catch_up_with_primary()?;
            bucket.expiring.try_catch_up_with_primary()?;
            let rows = format_rows("", bucket.objects.safe_iter())
                .chain(format_rows("tombstone:", bucket.tombstones.safe_iter()))
                .chain(format_rows("expiring:", bucket.expiring.safe_iter()));
            return page(rows, page_size, page_number).map(Some);
        }
        if cf_name == EARLIEST_RETAINED_CF {
            let earliest_retained_table: DBMap<(), EpochId> =
                DBMap::reopen(db, Some(cf_name), &ReadWriteOptions::default(), true)?;
            earliest_retained_table.try_catch_up_with_primary()?;
            return page(
                format_rows("", earliest_retained_table.safe_iter()),
                page_size,
                page_number,
            )
            .map(Some);
        }
        Ok(None)
    }
}

/// The fallback-aware counterpart of
/// [`iota_types::storage::get_transaction_input_objects`]. These are the
/// versions the transaction superseded, so its own checkpoint commit relocates
/// them out of the live table.
pub fn get_transaction_input_objects(
    object_store: &dyn ObjectStore,
    historic_objects: &HistoricObjects,
    effects: &TransactionEffects,
) -> IotaResult<Vec<Object>> {
    let keys = effects
        .modified_at_versions()
        .into_iter()
        .map(|modified| ObjectKey(modified.object_id, modified.version))
        .collect::<Vec<_>>();
    multi_get_objects_with_historic_fallback(
        object_store,
        historic_objects,
        &keys,
        effects.transaction_digest(),
    )
}

/// The fallback-aware counterpart of
/// [`iota_types::storage::get_transaction_output_objects`]. These versions are
/// current when written, but a later transaction supersedes them and relocates
/// them out of the live table.
pub fn get_transaction_output_objects(
    object_store: &dyn ObjectStore,
    historic_objects: &HistoricObjects,
    effects: &TransactionEffects,
) -> IotaResult<Vec<Object>> {
    let keys = effects
        .all_changed_objects()
        .into_iter()
        .map(|(changed, _kind)| ObjectKey::from(changed.reference))
        .collect::<Vec<_>>();
    multi_get_objects_with_historic_fallback(
        object_store,
        historic_objects,
        &keys,
        effects.transaction_digest(),
    )
}

/// The objects at exactly `keys`, erroring on any the live table and the
/// buckets both lack.
fn multi_get_objects_with_historic_fallback(
    object_store: &dyn ObjectStore,
    historic_objects: &HistoricObjects,
    keys: &[ObjectKey],
    transaction_digest: &TransactionDigest,
) -> IotaResult<Vec<Object>> {
    let mut objects = object_store.multi_get_objects_by_key(keys);
    historic_objects.fill_missing(keys, &mut objects)?;
    objects
        .into_iter()
        .zip(keys)
        .map(|(object, key)| {
            object.ok_or_else(|| {
                IotaError::Storage(format!(
                    "missing object key {key:?} from tx {transaction_digest}"
                ))
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "../unit_tests/historic_objects_tests.rs"]
mod tests;
