// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Superseded object versions, bucketed by the epoch that superseded them.
//!
//! The buckets are column families of the perpetual database (see
//! [`crate::epoch_buckets`]), so moving a version out of the live `objects`
//! table and into a bucket is one atomic [`typed_store::rocks::DBBatch`].

use std::{
    collections::BTreeMap,
    fmt::Debug,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use iota_sdk_types::{ObjectId, TransactionDigest, TransactionEffects, Version};
use iota_types::{
    committee::EpochId,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    error::{IotaError, IotaResult},
    messages_checkpoint::CheckpointSequenceNumber,
    object::Object,
    storage::{ObjectKey, ObjectStore},
};
use tracing::{error, info, warn};
use typed_store::{
    DbIterator, TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, ReadWriteOptions, TaggedDBMap, list_tables, synced_write_options},
    traits::Map,
};

use crate::{
    authority::authority_store_types::{StoreObject, StoreObjectWrapper},
    epoch_buckets::{
        EpochBuckets, bucket_cf_epoch, bucket_cf_name, bucket_cf_options,
        extra_column_family_options,
    },
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
/// a bucket expires.
const TOMBSTONE_DELETE_BATCH_SIZE: usize = 10_000;

/// Column family holding the earliest-retained-epoch marker
/// [`EpochBuckets`] persists on a prune; empty until the first prune.
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
    /// epoch or an earlier one. Hence expiry goes oldest epoch first.
    ///
    /// A reader that also consults the live `objects` table must read it
    /// **before** asking the buckets. Bucket handles outlive the buckets read
    /// lock, so otherwise a bucket read could race an expiry that has already
    /// deleted the tombstone covering those rows. Reading live first means
    /// either the live read finds the tombstone, or the bucket read waits on
    /// the lock until the expiry has taken the bucket out of the map.
    pub(crate) tombstones: TaggedDBMap<ObjectKey, ()>,

    /// Present once this bucket is scheduled for expiry, after which reads
    /// skip it. Write it through [`Self::mark_expiring`].
    pub(crate) expiring: TaggedDBMap<(), ()>,

    /// Mirrors the `expiring` row, so a query need not look it up.
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

    /// Durably marks this bucket expiring, then stops serving its rows.
    ///
    /// Synced, because a column-family drop is durable at once while a
    /// default write may be lost, which after a crash would make a bucket
    /// whose tombstone heads are gone readable again.
    fn mark_expiring(&self) -> Result<(), TypedStoreError> {
        let mut batch = self.expiring.batch();
        batch.insert_batch_tagged(&self.expiring, [((), ())])?;
        batch.write_opt(&synced_write_options())?;
        self.expiring_marked.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Superseded object versions, bucketed by the epoch that superseded them.
pub struct HistoricObjects {
    buckets: EpochBuckets<HistoricObjectsBucket>,
    /// The live objects table of the same database, which holds the
    /// tombstones of every bucket that has not expired.
    objects: DBMap<ObjectKey, StoreObjectWrapper>,
    /// The checkpoint through which the objects pruner of earlier releases
    /// deleted superseded versions, `None` on a database it never ran on.
    pub(crate) objects_pruned_through: Option<CheckpointSequenceNumber>,
}

impl HistoricObjects {
    /// Options for a historic-object bucket's column family: written once,
    /// while the epoch that relocated its rows is current, then only ever
    /// read back by exact-key lookup.
    ///
    /// `db_options` are the perpetual database's base options; build this
    /// once and clone it per column family.
    /// The `(name, options)` pairs of the column families this store needs,
    /// for the perpetual store's open path to list alongside its own tables.
    /// See
    /// [`extra_column_family_options`](crate::epoch_buckets::extra_column_family_options).
    pub fn extra_column_family_options(
        perpetual_path: &Path,
        db_options: &DBOptions,
    ) -> Vec<(String, DBOptions)> {
        extra_column_family_options(
            perpetual_path,
            db_options,
            HISTORIC_OBJECTS_CF_PREFIX,
            EARLIEST_RETAINED_CF,
        )
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
    /// have cost it. Such a bucket's heads are already partly deleted, so
    /// finishing is the only way to a consistent table.
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

        let cf_options = bucket_cf_options(db_options).options;
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
        // below the floor is an unfinished expiry even without its marker.
        // `EpochBuckets::open` would just drop it, leaving its tombstone heads
        // in the live `objects` table for good.
        //
        // Oldest first: a bounded read that stops at one of this bucket's
        // deleted heads falls through to the older buckets, so those must be
        // gone first.
        let interrupted: Vec<(EpochId, Arc<HistoricObjectsBucket>)> = buckets
            .iter()
            .filter(|(&epoch, bucket)| epoch < earliest_retained || bucket.is_expiring())
            .map(|(&epoch, bucket)| (epoch, bucket.clone()))
            .collect();
        for (epoch, bucket) in interrupted {
            let cf_name = bucket_cf_name(HISTORIC_OBJECTS_CF_PREFIX, epoch);
            if let Err(e) = Self::expire_bucket(&objects, epoch, &bucket) {
                // Named so that an operator whose node will not start can
                // reach the column family with external tooling.
                error!(
                    epoch,
                    cf_name, "cannot finish the interrupted expiry of a historic bucket: {e}"
                );
                return Err(e);
            }
            buckets.remove(&epoch);
            info!(
                epoch,
                "dropping the bucket of an interrupted expiry at open"
            );
            if let Err(e) = db.drop_cf(&cf_name) {
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
        Ok(Self {
            buckets,
            objects,
            objects_pruned_through: None,
        })
    }

    /// The oldest epoch this store holds a bucket for, `None` when it holds
    /// none; versions superseded before it are not readable. A node restored
    /// from a formal snapshot starts with no bucket, whatever the retention.
    pub fn earliest_bucket_epoch(&self) -> Option<EpochId> {
        self.buckets.earliest_epoch()
    }

    /// The newest epoch this store holds a bucket for, `None` when it holds
    /// none at all.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn newest_bucket_epoch(&self) -> Option<EpochId> {
        self.buckets.newest_epoch()
    }

    /// The bucket holding `epoch`'s relocated versions, created if absent.
    pub fn ensure(&self, epoch: EpochId) -> IotaResult<Arc<HistoricObjectsBucket>> {
        self.buckets
            .ensure(epoch)
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// The object relocated under `key`, `None` if it was never relocated (or
    /// its bucket has since been dropped).
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

    /// The newest relocated version of `id` at or below `version`, `None` if
    /// there is none (or its bucket has since been dropped).
    ///
    /// A caller must read the live `objects` table first (see
    /// [`HistoricObjectsBucket::tombstones`]) and take the newer of the two
    /// answers: this ignores tombstones, so only a live tombstone newer than
    /// what it returns means the object is gone.
    pub fn find_lt_or_eq_version(
        &self,
        id: ObjectId,
        version: Version,
    ) -> IotaResult<Option<Object>> {
        // Versions are relocated in increasing order, so the newest bucket
        // with a match holds the newest version.
        for bucket in self.readable_buckets(true) {
            let newest = bucket
                .objects
                .safe_range_iter_reversed(ObjectKey::min_for_id(&id)..=ObjectKey(id, version))
                .next()
                .transpose()
                .map_err(|e| IotaError::Storage(e.to_string()))?;
            if let Some((_, object)) = newest {
                return Ok(Some(object));
            }
        }
        Ok(None)
    }

    /// The buckets a query may read, by ascending epoch or descending if
    /// `reverse`, leaving out any marked expiring.
    ///
    /// A caller that also reads the live `objects` table must read it
    /// **before** calling this; see [`HistoricObjectsBucket::tombstones`].
    fn readable_buckets(&self, reverse: bool) -> Vec<Arc<HistoricObjectsBucket>> {
        self.buckets
            .iter(reverse)
            .into_iter()
            .filter(|bucket| !bucket.is_expiring())
            .collect()
    }

    /// Keeps the buckets of `current_epoch` and the `epochs_to_retain` epochs
    /// below it, drops older ones, and deletes the tombstone heads each
    /// dropped epoch recorded. Returns the earliest epoch still retained,
    /// `None` when there is no bucket at all.
    ///
    /// Blocks queries for the duration, so an async caller must use
    /// `spawn_blocking`.
    pub fn prune(
        &self,
        current_epoch: EpochId,
        epochs_to_retain: u64,
    ) -> IotaResult<Option<EpochId>> {
        self.buckets
            .prune(current_epoch, epochs_to_retain, |epoch, bucket| {
                Self::expire_bucket(&self.objects, epoch, bucket).map_err(|e| {
                    TypedStoreError::RocksDB(format!("expiring the bucket of epoch {epoch}: {e}"))
                })
            })
            .map_err(|e| IotaError::Storage(e.to_string()))
    }

    /// Marks `bucket` expiring, then deletes the tombstone heads it recorded
    /// from the live `objects` table.
    ///
    /// Safe to run again on the same bucket: the marker is rewritten as it
    /// was and a head already deleted is deleted again.
    fn expire_bucket(
        objects: &DBMap<ObjectKey, StoreObjectWrapper>,
        epoch: EpochId,
        bucket: &Arc<HistoricObjectsBucket>,
    ) -> Result<(), TypedStoreError> {
        bucket.mark_expiring()?;

        // Synced: a column-family drop is durable at once, so a lost deletion
        // would leave tombstone heads with nothing left to delete them.
        let delete = |heads: Vec<ObjectKey>| -> Result<(), TypedStoreError> {
            let mut batch = objects.batch();
            batch.delete_batch(objects, heads)?;
            batch.write_opt(&synced_write_options())
        };

        // A version left beneath a head in the live table would become the
        // newest again once the head is deleted, bringing the object back.
        // That only happens where the backlog sweep did not relocate it, so
        // such a head is kept. Tombstones beneath it don't count: an object
        // wrapped and deleted in one epoch leaves two heads in this bucket.
        let buried_alive = |head: &ObjectKey| -> Result<bool, TypedStoreError> {
            for row in objects.safe_range_iter_reversed(ObjectKey::min_for_id(&head.0)..*head) {
                let (_, below) = row?;
                if matches!(below.migrate().into_inner(), StoreObject::Value(_)) {
                    return Ok(true);
                }
            }
            Ok(false)
        };

        let mut deleted = 0;
        let mut kept = 0;
        let mut heads = Vec::with_capacity(TOMBSTONE_DELETE_BATCH_SIZE);
        for row in bucket.tombstones.safe_iter() {
            let (key, ()) = row?;
            if buried_alive(&key)? {
                kept += 1;
                continue;
            }
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

        if kept > 0 {
            error!(
                epoch,
                kept,
                "kept tombstone heads that still have a live version beneath them: those \
                 objects were superseded before this build and the backlog sweep did not \
                 relocate them. They stay deleted, which is why the heads stay too"
            );
        }
        info!(
            epoch,
            tombstones = deleted,
            kept,
            "expired a historic bucket"
        );
        Ok(())
    }

    /// Writes a row of the wrong type into `epoch`'s tombstone-head table, so
    /// that expiring that bucket fails.
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
        batch.insert_batch_tagged(&unreadable, [(ObjectKey(ObjectId::ZERO, 1.into()), epoch)])?;
        batch.write()
    }

    /// One page of the rows of `cf_name` if it is one of this store's column
    /// families, `None` otherwise. For the `iota-tool` table dump, which
    /// cannot reach these through `AuthorityPerpetualTables`.
    ///
    /// Tombstone-head and expiring-marker rows are prefixed by table name.
    /// `db` may be a read-only or secondary handle.
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

/// Like [`iota_types::storage::get_transaction_input_objects`], but also
/// reads the relocated versions.
pub fn get_transaction_input_objects(
    object_store: &dyn ObjectStore,
    historic_objects: &HistoricObjects,
    effects: &TransactionEffects,
) -> IotaResult<Vec<Object>> {
    let keys = effects
        .modified_at_versions()
        .into_iter()
        .map(|modified| ObjectKey(*modified.object_id(), modified.version()))
        .collect::<Vec<_>>();
    multi_get_objects_with_historic_fallback(
        object_store,
        historic_objects,
        &keys,
        effects.transaction_digest(),
    )
}

/// Like [`iota_types::storage::get_transaction_output_objects`], but also
/// reads the relocated versions.
pub fn get_transaction_output_objects(
    object_store: &dyn ObjectStore,
    historic_objects: &HistoricObjects,
    effects: &TransactionEffects,
) -> IotaResult<Vec<Object>> {
    let keys = effects
        .all_changed_objects()
        .into_iter()
        .map(|(changed, _kind)| ObjectKey::from(*changed.reference()))
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
