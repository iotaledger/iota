// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Superseded object versions, bucketed by the epoch that superseded them.
//!
//! The buckets are extra column families of the perpetual database rather
//! than a store of their own (see [`crate::epoch_buckets`]), so relocating a
//! version out of the live `objects` table and in here is one atomic
//! [`typed_store::rocks::DBBatch`] instead of a cross-database move.

use std::{
    collections::BTreeMap,
    fmt::Debug,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use iota_sdk_types::{TransactionDigest, TransactionEffects};
use iota_types::{
    committee::EpochId,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult},
    object::Object,
    storage::{ObjectKey, ObjectStore},
};
use tracing::{info, warn};
use typed_store::{
    DbIterator, TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, TaggedDBMap, list_tables, synced_write_options},
    traits::Map,
};

use crate::{
    authority::authority_store_types::StoreObjectWrapper,
    epoch_buckets::{EpochBuckets, bucket_cf_epoch, bucket_cf_name},
};

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

/// Tombstone heads deleted from the live `objects` table per write batch when
/// a bucket expires. An epoch can hold millions of them, so they are streamed
/// out in batches of this size rather than gathered into one; the whole epoch
/// is still deleted before the bucket's column family is dropped.
const TOMBSTONE_DELETE_BATCH_SIZE: usize = 10_000;

/// Column family holding the earliest-retained-epoch marker
/// [`EpochBuckets`] persists on a prune. It is empty until the first prune,
/// which is the same as retaining every bucket.
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
    /// Write it through [`Self::mark_expiring`], which also stops the reads.
    pub(crate) expiring: TaggedDBMap<(), ()>,

    /// Mirrors the `expiring` row, read once when the bucket is opened and
    /// set again when the marker is written, so a query does not pay a
    /// lookup to find out whether the bucket may still be read.
    expiring_marked: AtomicBool,
}

impl HistoricObjectsBucket {
    fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError> {
        let expiring: TaggedDBMap<(), ()> = TaggedDBMap::reopen(
            db,
            cf_name,
            DB_PREFIX_HISTORIC_EXPIRING,
            &ReadWriteOptions::default(),
            true,
        )?;
        let expiring_marked = AtomicBool::new(expiring.get(&())?.is_some());
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
            expiring,
            expiring_marked,
        })
    }

    /// Whether this bucket has been marked expiring, in which case its rows
    /// must no longer be served: the tombstone heads it recorded may already
    /// be deleted from the live `objects` table, and a version served from
    /// under a deleted tombstone resurrects a deleted object.
    fn is_expiring(&self) -> bool {
        self.expiring_marked.load(Ordering::Relaxed)
    }

    /// Marks this bucket expiring and makes the marker durable, then stops
    /// serving its rows.
    ///
    /// Synced, because a column-family drop is durable at once while a
    /// default write may still be lost, which would leave a bucket whose
    /// tombstone heads are gone readable again after a crash.
    fn mark_expiring(&self) -> Result<(), TypedStoreError> {
        let mut batch = self.expiring.batch();
        batch.insert_batch_tagged(&self.expiring, [((), ())])?;
        batch.write_opt(&synced_write_options())?;
        self.expiring_marked.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Superseded object versions, bucketed by the epoch that superseded them.
///
/// The buckets are column families of the perpetual database rather than a
/// store of their own, so a version can leave `objects` and arrive here in
/// one atomic batch.
pub struct HistoricObjects {
    buckets: EpochBuckets<HistoricObjectsBucket>,
    /// The live objects table of the same database, holding the tombstones a
    /// bucket's heads point at until that bucket expires.
    objects: DBMap<ObjectKey, StoreObjectWrapper>,
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
    /// `db_options` are the options its tables were opened with. `objects` is
    /// that database's live objects table, which holds the tombstones the
    /// buckets' heads point at.
    ///
    /// A bucket an interrupted prune left behind is finished here, oldest
    /// first, before any query can reach it: one marked expiring, and one
    /// below the persisted retention floor, whose marker the same crash may
    /// have cost it.
    pub fn open(
        db: Arc<Database>,
        db_options: &DBOptions,
        objects: DBMap<ObjectKey, StoreObjectWrapper>,
    ) -> Result<Self, TypedStoreError> {
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
        let earliest_retained = earliest_retained_table.get(&())?.unwrap_or(0);

        // A prune persists the floor before it marks anything, so a bucket
        // below the floor is one whose expiry did not finish, whether or not
        // it got as far as its marker. Left to `EpochBuckets::open` it would
        // have its column family dropped with its tombstone heads still in
        // the live `objects` table, which nothing would ever delete.
        //
        // Ascending, as `BTreeMap` iterates, and the buckets below the floor
        // are the oldest of the two kinds: an expiring bucket's tombstone
        // heads are already deleted while its versions still exist, so a scan
        // bounded at one of those tombstones falls through to the buckets
        // below it, and is only answered correctly because every one of them
        // is gone by then.
        let interrupted: Vec<(EpochId, Arc<HistoricObjectsBucket>)> = buckets
            .iter()
            .filter(|(&epoch, bucket)| epoch < earliest_retained || bucket.is_expiring())
            .map(|(&epoch, bucket)| (epoch, bucket.clone()))
            .collect();
        for (epoch, bucket) in interrupted {
            Self::expire_bucket(&objects, epoch, &bucket)?;
            buckets.remove(&epoch);
            info!(
                epoch,
                "dropping the bucket of an interrupted expiry at open"
            );
            if let Err(e) = db.drop_cf(&bucket_cf_name(HISTORIC_OBJECTS_CF_PREFIX, epoch)) {
                warn!(
                    epoch,
                    "failed to drop an expiring bucket column family: {e}"
                );
            }
        }

        let buckets = EpochBuckets::open(
            db,
            "historic objects",
            HISTORIC_OBJECTS_CF_PREFIX,
            cf_options,
            earliest_retained_table,
            buckets,
            HistoricObjectsBucket::reopen,
        )?;
        Ok(Self { buckets, objects })
    }

    /// The oldest epoch this store still holds a bucket for, `None` when it
    /// holds none at all. No object version superseded before this epoch is
    /// readable any more.
    ///
    /// This is what the store holds, not what its retention would keep: a node
    /// restored from a formal snapshot starts with no bucket at all, whatever
    /// the retention says.
    pub fn earliest_bucket_epoch(&self) -> Option<EpochId> {
        self.buckets.earliest_epoch()
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
        for bucket in self.readable_buckets(true) {
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

    /// The buckets a query may read, in scan order: ascending epochs for
    /// forward scans, descending for reverse scans.
    ///
    /// A bucket marked expiring is left out. Its rows are dropped with its
    /// column family a moment later, but the marker is what a reader has to
    /// go by: an expiry that failed after the marker leaves the bucket in the
    /// map until the caller retries, and its tombstone heads may already be
    /// gone from the live `objects` table by then.
    ///
    /// A caller that consults the live `objects` table as well must read it
    /// **before** calling this. The returned handles outlive the buckets read
    /// lock, so a caller holding them from before an expiry started keeps
    /// reading a bucket whose tombstone heads are being deleted meanwhile,
    /// and a live read taken afterwards no longer finds the tombstone that
    /// covers those rows. Reading live first closes that window: either the
    /// live read precedes the deletion and finds the tombstone, or this call
    /// waits on the read lock until the expiry has taken the bucket out of
    /// the map.
    fn readable_buckets(&self, reverse: bool) -> Vec<Arc<HistoricObjectsBucket>> {
        self.buckets
            .iter(reverse)
            .into_iter()
            .filter(|bucket| !bucket.is_expiring())
            .collect()
    }

    /// Drops the buckets outside `epochs_to_retain` — the newest bucket and
    /// the `epochs_to_retain - 1` below it — and deletes the tombstone heads
    /// each dropped epoch recorded. Returns the earliest epoch still
    /// retained, `None` when there is no bucket at all.
    ///
    /// Blocks queries for the duration, so an async caller must use
    /// `spawn_blocking`.
    pub fn prune(&self, epochs_to_retain: u64) -> IotaResult<Option<EpochId>> {
        self.buckets
            .prune(epochs_to_retain, |epoch, bucket| {
                Self::expire_bucket(&self.objects, epoch, bucket)
            })
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// Marks `bucket` expiring, then deletes the tombstone heads it recorded
    /// from the live `objects` table.
    ///
    /// The marker is written and made durable first, since a tombstone head
    /// may only be deleted once the versions beneath it are out of reach.
    /// [`EpochBuckets::prune`] holds the buckets write lock while this runs,
    /// so a query that has not yet taken the read lock cannot reach the
    /// bucket at all, and one that reaches it after the marker is written
    /// skips it; a crash between the marker and the drop is resumed at open.
    /// A query still holding the handles it took before the marker was
    /// written does keep reading this bucket, which is the window
    /// [`Self::readable_buckets`] tells its callers how to close.
    ///
    /// Safe to run again on the same bucket: the marker is rewritten as it
    /// was and a tombstone head already deleted is deleted again. That also
    /// covers a run that failed part-way through the deletion, since the heads
    /// stay in the bucket and are read again from there.
    fn expire_bucket(
        objects: &DBMap<ObjectKey, StoreObjectWrapper>,
        epoch: EpochId,
        bucket: &Arc<HistoricObjectsBucket>,
    ) -> Result<(), TypedStoreError> {
        bucket.mark_expiring()?;

        let delete = |heads: Vec<ObjectKey>| -> Result<(), TypedStoreError> {
            let mut batch = objects.batch();
            batch.delete_batch(objects, heads)?;
            batch.write()
        };

        let mut deleted = 0;
        let mut heads = Vec::with_capacity(TOMBSTONE_DELETE_BATCH_SIZE);
        for row in bucket.tombstones.safe_iter() {
            let (key, ()) = row?;
            heads.push(key);
            if heads.len() == TOMBSTONE_DELETE_BATCH_SIZE {
                deleted += heads.len();
                delete(std::mem::replace(
                    &mut heads,
                    Vec::with_capacity(TOMBSTONE_DELETE_BATCH_SIZE),
                ))?;
            }
        }
        deleted += heads.len();
        delete(heads)?;

        info!(epoch, tombstones = deleted, "expired a historic bucket");
        Ok(())
    }

    /// Writes a row of the wrong type into `epoch`'s tombstone-head table, so
    /// that reading its heads back fails and that bucket's expiry cannot
    /// finish. For exercising what a node does when a bucket cannot be
    /// expired; there is no production hook that makes one fail.
    #[cfg(test)]
    pub(super) fn corrupt_tombstone_heads_for_testing(
        db: &Arc<Database>,
        epoch: EpochId,
    ) -> Result<(), TypedStoreError> {
        let unreadable: TaggedDBMap<ObjectKey, u64> = TaggedDBMap::reopen(
            db,
            &bucket_cf_name(HISTORIC_OBJECTS_CF_PREFIX, epoch),
            DB_PREFIX_HISTORIC_TOMBSTONES,
            &ReadWriteOptions::default(),
            true,
        )?;
        let mut batch = unreadable.batch();
        batch.insert_batch_tagged(
            &unreadable,
            [(ObjectKey(iota_sdk_types::ObjectId::ZERO, 1.into()), epoch)],
        )?;
        batch.write()
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
