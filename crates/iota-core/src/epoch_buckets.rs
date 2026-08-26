// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch column families, shared by the stores that retain their rows
//! epoch by epoch: the RPC index history, the superseded object versions, the
//! ledger history of executed transactions, and the checkpoint history.
//!
//! Rows are partitioned by the epoch that produced them, one column family
//! per epoch, so pruning an epoch is one constant-time column-family drop
//! instead of per-row deletes. The stores differ only in what one bucket
//! holds; everything about creating, finding, and dropping buckets is
//! shared here.

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use iota_types::committee::EpochId;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use tracing::{info, warn};
use typed_store::{
    TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, list_tables, synced_write_options},
    rocksdb,
    traits::Map,
};

/// Options for the RPC index stores' history buckets. Each bucket is
/// write-once (appended during its epoch or the backfill, then only read)
/// and queried by bounded range scans plus exact-key digest probes, which
/// the block-based bloom filters answer from RAM. `set_block_options`
/// creates the single block cache that every clone of these options shares.
/// A store with another access pattern builds its own options.
pub(crate) fn history_cf_options(
    db_options: &DBOptions,
    block_cache_size_mb: usize,
) -> rocksdb::Options {
    db_options
        .clone()
        .optimize_for_write_throughput_no_deletion()
        .set_block_options(block_cache_size_mb, 16 << 10)
        .options
}

/// Options every bucket column family is opened with.
pub(crate) fn bucket_cf_options(db_options: &DBOptions) -> DBOptions {
    db_options
        .clone()
        .optimize_for_write_throughput_no_deletion()
}

/// The `(name, options)` pairs a store's open path must list for its buckets
/// and for `earliest_retained_cf`, the column family holding their retention
/// floor.
///
/// Every bucket column family already on disk under `db_path` is included,
/// since one left for auto-discovery would be reopened with default options
/// and a block cache of its own. If `db_path` has no database yet, or its
/// column families cannot be listed, only `earliest_retained_cf` is returned.
pub(crate) fn extra_column_family_options(
    db_path: &Path,
    db_options: &DBOptions,
    cf_prefix: &str,
    earliest_retained_cf: &str,
) -> Vec<(String, DBOptions)> {
    let cf_options = bucket_cf_options(db_options);
    let mut options = vec![(earliest_retained_cf.to_string(), cf_options.clone())];
    if !db_path.join("CURRENT").exists() {
        return options;
    }
    let existing_cfs = match list_tables(db_path.to_path_buf()) {
        Ok(existing_cfs) => existing_cfs,
        // Not an error: the open that follows reports what is wrong with the
        // database.
        Err(err) => {
            warn!(
                "failed to list the column families of {}: {err}",
                db_path.display()
            );
            return options;
        }
    };
    options.extend(
        existing_cfs
            .into_iter()
            .filter(|name| bucket_cf_epoch(cf_prefix, name).is_some())
            .map(|name| (name, cf_options.clone())),
    );
    options
}

/// Treats a read from a bucket whose column family has been dropped as
/// returning no rows, rather than as an error.
///
/// [`EpochBuckets::iter_with_epoch`] hands out handles and releases the lock,
/// so an expiry can drop a bucket while a walk is still working through them.
/// A dropped bucket is an epoch this node no longer serves.
pub(crate) fn absent_if_dropped<T: Default>(
    read: Result<T, TypedStoreError>,
) -> Result<T, TypedStoreError> {
    match read {
        Err(TypedStoreError::UnregisteredColumn(_)) => Ok(T::default()),
        other => other,
    }
}

/// Stands for "no bucket at all" in [`EpochBuckets::earliest_bucket_epoch`].
/// No real epoch reaches it.
const NO_BUCKET: EpochId = EpochId::MAX;

/// The column-family name of `epoch`'s bucket: `"{cf_prefix}{epoch}"`.
pub(crate) fn bucket_cf_name(cf_prefix: &str, epoch: EpochId) -> String {
    format!("{cf_prefix}{epoch}")
}

/// The epoch of a bucket's column family, `None` for other names.
pub(crate) fn bucket_cf_epoch(cf_prefix: &str, cf_name: &str) -> Option<EpochId> {
    cf_name
        .strip_prefix(cf_prefix)
        .and_then(|epoch| epoch.parse().ok())
}

/// The per-epoch buckets of one store. `B` is that store's view of one
/// bucket, built by `reopen` from the bucket's column-family name.
///
/// On-disk column-family names are the ground truth for which buckets exist;
/// the map here mirrors them for reads.
pub(crate) struct EpochBuckets<B> {
    db: Arc<Database>,
    /// What this store is called in log lines, which several stores write
    /// for the same epoch.
    name: &'static str,
    cf_prefix: &'static str,
    /// Template options for the buckets' column families. All clones share
    /// one block cache through the cloned table factory.
    cf_options: rocksdb::Options,
    reopen: fn(&Arc<Database>, &str) -> Result<B, TypedStoreError>,
    buckets: RwLock<BTreeMap<EpochId, Arc<B>>>,
    /// The earliest retained epoch recorded by the last [`Self::prune`]
    /// call, mirroring the persisted row; never moves backwards.
    earliest_retained_epoch: AtomicU64,
    earliest_retained_table: DBMap<(), EpochId>,
    /// Mirrors the oldest epoch in `buckets` ([`NO_BUCKET`] when empty),
    /// updated under the map's write lock. Kept apart from the map so that
    /// request-path readers do not block on a [`Self::prune`], which holds
    /// the write lock throughout.
    earliest_bucket_epoch: AtomicU64,
}

impl<B> EpochBuckets<B> {
    /// Assembles the store's buckets from the ones discovered on disk,
    /// dropping those below the persisted retention floor.
    ///
    /// A bucket below the floor is one whose drop failed: RocksDB
    /// unregisters a column family before dropping it, so the failure
    /// survives only on disk. It is dropped here rather than served again,
    /// and a drop that fails again still leaves the epoch out of the
    /// history. A floor read error fails the open instead of passing for a
    /// store with no retention floor.
    pub(crate) fn open(
        db: Arc<Database>,
        name: &'static str,
        cf_prefix: &'static str,
        cf_options: rocksdb::Options,
        earliest_retained_table: DBMap<(), EpochId>,
        mut buckets: BTreeMap<EpochId, Arc<B>>,
        reopen: fn(&Arc<Database>, &str) -> Result<B, TypedStoreError>,
    ) -> Result<Self, TypedStoreError> {
        let earliest_retained_epoch = earliest_retained_table.get(&())?.unwrap_or(0);
        let pruned: Vec<EpochId> = buckets
            .range(..earliest_retained_epoch)
            .map(|(&epoch, _)| epoch)
            .collect();
        for epoch in pruned {
            info!(
                store = name,
                epoch, "dropping a pruned bucket column family at open"
            );
            buckets.remove(&epoch);
            if let Err(e) = db.drop_cf(&bucket_cf_name(cf_prefix, epoch)) {
                warn!(epoch, "failed to drop a pruned bucket column family: {e}");
            }
        }
        let earliest_bucket_epoch = Self::earliest_epoch_of(&buckets);
        Ok(Self {
            db,
            name,
            cf_prefix,
            cf_options,
            reopen,
            buckets: RwLock::new(buckets),
            earliest_retained_epoch: AtomicU64::new(earliest_retained_epoch),
            earliest_retained_table,
            earliest_bucket_epoch: AtomicU64::new(earliest_bucket_epoch),
        })
    }

    /// The oldest epoch in `buckets`, [`NO_BUCKET`] when there is none.
    fn earliest_epoch_of(buckets: &BTreeMap<EpochId, Arc<B>>) -> EpochId {
        buckets
            .first_key_value()
            .map_or(NO_BUCKET, |(&epoch, _)| epoch)
    }

    /// Republishes the mirror of the oldest epoch in `buckets`. The caller
    /// must hold the write lock.
    fn publish_earliest_epoch(&self, buckets: &BTreeMap<EpochId, Arc<B>>) {
        self.earliest_bucket_epoch
            .store(Self::earliest_epoch_of(buckets), Ordering::Relaxed);
    }

    /// The retained buckets in scan order: ascending epochs, or descending
    /// when `reverse`. Chaining per-bucket scans in this order preserves the
    /// global order.
    pub(crate) fn iter(&self, reverse: bool) -> Vec<Arc<B>> {
        let buckets = self.buckets.read();
        if reverse {
            buckets.values().rev().cloned().collect()
        } else {
            buckets.values().cloned().collect()
        }
    }

    /// [`Self::iter`], with the epoch each bucket holds.
    pub(crate) fn iter_with_epoch(&self, reverse: bool) -> Vec<(EpochId, Arc<B>)> {
        let buckets = self.buckets.read();
        let rows = buckets
            .iter()
            .map(|(&epoch, bucket)| (epoch, bucket.clone()));
        if reverse {
            rows.rev().collect()
        } else {
            rows.collect()
        }
    }

    /// The newest epoch holding a bucket, `None` when there is none.
    pub(crate) fn newest_epoch(&self) -> Option<EpochId> {
        self.buckets
            .read()
            .last_key_value()
            .map(|(&epoch, _)| epoch)
    }

    /// The oldest epoch holding a bucket, `None` when there is none. This can
    /// be above [`Self::earliest_retained`], e.g. on a node restored from a
    /// formal snapshot. Takes no lock, so it is safe to call on a request path.
    pub(crate) fn earliest_epoch(&self) -> Option<EpochId> {
        match self.earliest_bucket_epoch.load(Ordering::Relaxed) {
            NO_BUCKET => None,
            epoch => Some(epoch),
        }
    }

    /// The earliest epoch [`Self::prune`] retains; buckets below it are gone
    /// and are never recreated.
    pub(crate) fn earliest_retained(&self) -> EpochId {
        self.earliest_retained_epoch.load(Ordering::Relaxed)
    }

    /// The bucket holding `epoch`'s rows, `None` when there is none — either
    /// because nothing has been written for that epoch yet or because it has
    /// been pruned.
    ///
    /// For a reader that knows which epoch it wants and must not create a
    /// column family to find out that the answer is nothing.
    pub(crate) fn get(&self, epoch: EpochId) -> Option<Arc<B>> {
        self.buckets.read().get(&epoch).cloned()
    }

    /// The bucket holding `epoch`'s rows, created if absent. Pruned
    /// epochs are refused: recreating a pruned epoch's column family would
    /// resurrect it under the same name, and a reader holding the dropped
    /// bucket would silently read the new, empty one.
    pub(crate) fn ensure(&self, epoch: EpochId) -> Result<Arc<B>, TypedStoreError> {
        self.ensure_retained(epoch)?.ok_or_else(|| {
            TypedStoreError::Pruned(format!(
                "the bucket of epoch {epoch} was pruned: only epochs from {} on are retained",
                self.earliest_retained()
            ))
        })
    }

    /// The bucket holding `epoch`'s rows, created if absent, and `None` when
    /// `epoch` is below the retention floor. For a writer that can be handed
    /// rows of an expired epoch, such as state sync; others want
    /// [`Self::ensure`].
    pub(crate) fn ensure_retained(
        &self,
        epoch: EpochId,
    ) -> Result<Option<Arc<B>>, TypedStoreError> {
        if epoch < self.earliest_retained() {
            return Ok(None);
        }
        if let Some(bucket) = self.buckets.read().get(&epoch) {
            return Ok(Some(bucket.clone()));
        }
        let mut buckets = self.buckets.write();
        if let Some(bucket) = buckets.get(&epoch) {
            return Ok(Some(bucket.clone()));
        }
        // The epoch may have been pruned between the check above and taking
        // the lock that `prune` holds.
        if epoch < self.earliest_retained() {
            return Ok(None);
        }
        let cf_name = bucket_cf_name(self.cf_prefix, epoch);
        // The column family may already exist if a previous run crashed
        // between `create_cf` and the first batch write.
        if self.db.cf_handle(&cf_name).is_none() {
            self.db.create_cf(&cf_name, &self.cf_options)?;
        }
        let bucket = Arc::new((self.reopen)(&self.db, &cf_name)?);
        buckets.insert(epoch, bucket.clone());
        self.publish_earliest_epoch(&buckets);
        Ok(Some(bucket))
    }

    /// Drops the buckets older than `current_epoch` and the `epochs_to_retain`
    /// epochs below it; buckets above `current_epoch` are left alone. `0`
    /// keeps `current_epoch`'s bucket alone.
    ///
    /// `current_epoch` is passed in because state sync can write buckets
    /// ahead of execution, which must not count towards the retention.
    ///
    /// Returns the earliest epoch to retain, `None` when there is no bucket at
    /// all. It is persisted before the drops and never moves backwards, so
    /// dropped epochs are never recreated, even after a reopen or a raised
    /// `epochs_to_retain`. An epoch whose drop failed is gone all the same,
    /// and the next open drops it.
    ///
    /// Queries block for the duration of the drops, so callers on an async
    /// runtime must use `spawn_blocking`.
    ///
    /// `before_drop` runs for each expiring epoch in ascending order, under
    /// the write lock and before the drop. Its error stops the prune and keeps
    /// that epoch's bucket, without rolling back what `before_drop` already
    /// wrote, so it must be safe to run again on the same epoch and must not
    /// durably change how a bucket may be read until it has made that safe.
    pub(crate) fn prune(
        &self,
        current_epoch: EpochId,
        epochs_to_retain: u64,
        mut before_drop: impl FnMut(EpochId, &Arc<B>) -> Result<(), TypedStoreError>,
    ) -> Result<Option<EpochId>, TypedStoreError> {
        // Runs once per executed checkpoint, usually with nothing to do, so
        // it takes the write lock queries block on only when it has to.
        let buckets = self.buckets.upgradable_read();
        let persisted = self.earliest_retained();
        let Some(earliest_retained) =
            Self::earliest_epoch_to_retain(&buckets, current_epoch, epochs_to_retain, persisted)
        else {
            return Ok(None);
        };
        if earliest_retained == persisted && buckets.range(..earliest_retained).next().is_none() {
            return Ok(Some(earliest_retained));
        }

        // Under the write lock, so `ensure` cannot hand out a bucket whose
        // column family is about to be dropped.
        let mut buckets = RwLockUpgradableReadGuard::upgrade(buckets);
        if earliest_retained != persisted {
            // Synced before any drop: RocksDB makes a column-family drop
            // durable at once, and a lost floor would let a reopen backfill
            // the dropped epochs again.
            let mut batch = self.earliest_retained_table.batch();
            batch.insert_batch(&self.earliest_retained_table, [((), earliest_retained)])?;
            batch.write_opt(&synced_write_options())?;
            self.earliest_retained_epoch
                .store(earliest_retained, Ordering::Relaxed);
        }
        let expired: Vec<(EpochId, Arc<B>)> = buckets
            .range(..earliest_retained)
            .map(|(&e, bucket)| (e, bucket.clone()))
            .collect();
        for (epoch, bucket) in expired {
            before_drop(epoch, &bucket)?;
            info!(
                store = self.name,
                epoch, "dropping the bucket of an expired epoch"
            );
            if let Err(e) = self.db.drop_cf(&bucket_cf_name(self.cf_prefix, epoch)) {
                warn!(epoch, "failed to drop an expired bucket column family: {e}");
            }
            // RocksDB unregisters the column family before it attempts the
            // drop, so even after a failed drop the bucket cannot be read.
            buckets.remove(&epoch);
            // Per epoch, so an early `before_drop` error leaves the mirror
            // matching the map.
            self.publish_earliest_epoch(&buckets);
        }
        Ok(Some(earliest_retained))
    }

    /// The earliest epoch to retain when `current_epoch` is kept together
    /// with the `epochs_to_retain` epochs below it, never below `persisted`.
    /// `None` when there is no bucket at all. `u64::MAX` retains everything.
    fn earliest_epoch_to_retain(
        buckets: &BTreeMap<EpochId, Arc<B>>,
        current_epoch: EpochId,
        epochs_to_retain: u64,
        persisted: EpochId,
    ) -> Option<EpochId> {
        if buckets.is_empty() {
            return None;
        }
        Some(
            current_epoch
                .saturating_sub(epochs_to_retain)
                .max(persisted),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use typed_store::rocks::{
        DBMap, MetricConf, ReadWriteOptions, default_db_options, open_cf_opts,
    };

    use super::{
        Arc, BTreeMap, Database, EpochBuckets, EpochId, TypedStoreError, bucket_cf_epoch,
        bucket_cf_name, rocksdb,
    };

    /// The name mapping must round-trip and reject other stores' prefixes:
    /// a shared database relies on it to tell bucket column families apart.
    #[test]
    fn cf_name_round_trips_within_its_prefix() {
        assert_eq!(bucket_cf_name("hist_e", 42), "hist_e42");
        assert_eq!(bucket_cf_epoch("hist_e", "hist_e42"), Some(42));
        assert_eq!(bucket_cf_epoch("hist_e", "hist_e"), None);
        assert_eq!(bucket_cf_epoch("hist_e", "hist_e4x"), None);
        assert_eq!(bucket_cf_epoch("hist_e", "owner_index"), None);
        assert_eq!(bucket_cf_epoch("other_", "hist_e42"), None);
    }

    const TEST_CF_PREFIX: &str = "test_e";
    const RETENTION_CF: &str = "test_retention";

    /// A bucket holding none of a store's own data, for exercising
    /// `EpochBuckets` on its own.
    struct TestBucket;

    impl TestBucket {
        fn reopen(_db: &Arc<Database>, _cf_name: &str) -> Result<Self, TypedStoreError> {
            Ok(Self)
        }
    }

    /// An `EpochBuckets` with one bucket per epoch in `epochs`, backed by a
    /// fresh temporary database whose directory must outlive the buckets.
    fn test_buckets(
        epochs: &[EpochId],
    ) -> (EpochBuckets<TestBucket>, iota_common::random_util::TempDir) {
        let dir = iota_common::tempdir();
        let db_options = default_db_options().options;
        let cf_names: Vec<String> = epochs
            .iter()
            .map(|&epoch| bucket_cf_name(TEST_CF_PREFIX, epoch))
            .chain([RETENTION_CF.to_string()])
            .collect();
        let opt_cfs: Vec<(&str, rocksdb::Options)> = cf_names
            .iter()
            .map(|name| (name.as_str(), db_options.clone()))
            .collect();
        let db = open_cf_opts(dir.path(), None, MetricConf::new("test"), &opt_cfs).unwrap();

        let earliest_retained_table: DBMap<(), EpochId> =
            DBMap::reopen(&db, Some(RETENTION_CF), &ReadWriteOptions::default(), true).unwrap();
        let buckets: BTreeMap<EpochId, Arc<TestBucket>> = epochs
            .iter()
            .map(|&epoch| (epoch, Arc::new(TestBucket)))
            .collect();

        let buckets = EpochBuckets::open(
            db,
            "test buckets",
            TEST_CF_PREFIX,
            db_options,
            earliest_retained_table,
            buckets,
            TestBucket::reopen,
        )
        .unwrap();
        (buckets, dir)
    }

    /// `before_drop` must see every expiring epoch, oldest first: a later
    /// consumer relies on this order to carry state forward from one
    /// dropped epoch to the next.
    #[tokio::test]
    async fn prune_calls_back_in_ascending_epoch_order() {
        let (buckets, _dir) = test_buckets(&[3, 4, 5, 6]);
        let seen = Mutex::new(Vec::new());
        let earliest = buckets
            .prune(6, 1, |epoch, _| {
                seen.lock().unwrap().push(epoch);
                Ok(())
            })
            .unwrap();
        assert_eq!(earliest, Some(5));
        assert_eq!(*seen.lock().unwrap(), vec![3, 4]);
    }

    /// Counting back from an epoch below the newest bucket must neither
    /// spend the retention on the buckets above it nor drop them: those are
    /// the epochs a store fed by state sync has run ahead into.
    #[tokio::test]
    async fn prune_ignores_the_buckets_above_the_current_epoch() {
        let (buckets, _dir) = test_buckets(&[3, 4, 5, 6]);
        let earliest = buckets.prune(4, 1, |_, _| Ok(())).unwrap();
        assert_eq!(earliest, Some(3));
        assert_eq!(buckets.iter(false).len(), 4);

        let earliest = buckets.prune(5, 1, |_, _| Ok(())).unwrap();
        assert_eq!(earliest, Some(4));
        assert_eq!(buckets.earliest_epoch(), Some(4));
        assert_eq!(buckets.newest_epoch(), Some(6));
    }

    /// A writer handed an expired epoch is told there is no bucket, rather
    /// than being refused or handed a recreated one. The refusing form stays
    /// available for writers to which an expired epoch is a fault.
    #[tokio::test]
    async fn an_expired_epoch_has_no_bucket_for_a_writer_that_tolerates_it() {
        let (buckets, _dir) = test_buckets(&[3, 4, 5]);
        buckets.prune(5, 1, |_, _| Ok(())).unwrap();
        assert_eq!(buckets.earliest_retained(), 4);

        assert!(buckets.ensure_retained(3).unwrap().is_none());
        assert!(buckets.ensure(3).is_err());
        // Nothing recreated the pruned column family.
        assert_eq!(buckets.earliest_epoch(), Some(4));
        assert_eq!(buckets.iter(false).len(), 2);

        assert!(buckets.ensure_retained(4).unwrap().is_some());
        assert!(buckets.ensure_retained(9).unwrap().is_some());
        assert_eq!(buckets.newest_epoch(), Some(9));
    }

    /// A callback error must abort that epoch's drop instead of leaving the
    /// bucket dropped with the store none the wiser.
    #[tokio::test]
    async fn a_callback_error_keeps_the_bucket() {
        let (buckets, _dir) = test_buckets(&[3, 4]);
        let result = buckets.prune(4, 0, |_, _| Err(TypedStoreError::RocksDB("no".to_string())));
        assert!(result.is_err());
        assert_eq!(buckets.iter(false).len(), 2);
    }
}
