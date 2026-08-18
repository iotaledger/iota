// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch column families, shared by the stores that retain their rows
//! epoch by epoch: the RPC index history and the superseded object
//! versions.
//!
//! Rows are partitioned by the epoch that produced them, one column family
//! per epoch, so pruning an epoch is one constant-time column-family drop
//! instead of per-row deletes. The stores differ only in what one bucket
//! holds; everything about creating, finding, and dropping buckets is
//! shared here.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use iota_types::committee::EpochId;
use parking_lot::RwLock;
use tracing::{info, warn};
use typed_store::{
    TypedStoreError,
    database::Database,
    rocks::{DBMap, DBOptions, synced_write_options},
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
    /// The database holding the buckets' column families; used to create
    /// and drop them at runtime.
    db: Arc<Database>,
    /// What this store is called in a log line, e.g. `"JSON-RPC index
    /// history"`. Several stores drop a bucket of the same epoch from one
    /// reconfiguration, so the events below have to say which one.
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
        Ok(Self {
            db,
            name,
            cf_prefix,
            cf_options,
            reopen,
            buckets: RwLock::new(buckets),
            earliest_retained_epoch: AtomicU64::new(earliest_retained_epoch),
            earliest_retained_table,
        })
    }

    /// The retained buckets in scan order: ascending epochs for forward
    /// scans, descending for reverse scans. Buckets are disjoint,
    /// epoch-ordered segments of the history, so chaining per-bucket scans
    /// in this order preserves the global order.
    pub(crate) fn iter(&self, reverse: bool) -> Vec<Arc<B>> {
        let buckets = self.buckets.read();
        if reverse {
            buckets.values().rev().cloned().collect()
        } else {
            buckets.values().cloned().collect()
        }
    }

    /// The newest epoch holding a bucket, `None` when there is none.
    pub(crate) fn newest_epoch(&self) -> Option<EpochId> {
        self.buckets
            .read()
            .last_key_value()
            .map(|(&epoch, _)| epoch)
    }

    /// The earliest epoch [`Self::prune`] retains; buckets below it are gone
    /// and are never recreated.
    pub(crate) fn earliest_retained(&self) -> EpochId {
        self.earliest_retained_epoch.load(Ordering::Relaxed)
    }

    /// The bucket holding `epoch`'s rows, created if absent. Pruned
    /// epochs are refused: recreating a pruned epoch's column family would
    /// resurrect it under the same name, and a reader holding the dropped
    /// bucket would silently read the new, empty one.
    pub(crate) fn ensure(&self, epoch: EpochId) -> Result<Arc<B>, TypedStoreError> {
        let refuse_pruned = |earliest_retained: EpochId| {
            if epoch < earliest_retained {
                return Err(TypedStoreError::Pruned(format!(
                    "the bucket of epoch {epoch} was pruned: only epochs from \
                     {earliest_retained} on are retained"
                )));
            }
            Ok(())
        };
        refuse_pruned(self.earliest_retained())?;
        if let Some(bucket) = self.buckets.read().get(&epoch) {
            return Ok(bucket.clone());
        }
        let mut buckets = self.buckets.write();
        if let Some(bucket) = buckets.get(&epoch) {
            return Ok(bucket.clone());
        }
        // Re-check under the lock `prune` publishes under: the epoch may
        // have been pruned between the check above and taking the lock, and
        // recreating its column family would hand stale readers an empty
        // bucket instead of an error.
        refuse_pruned(self.earliest_retained())?;
        let cf_name = bucket_cf_name(self.cf_prefix, epoch);
        // The column family may already exist if a previous run crashed
        // between `create_cf` and the first batch write.
        if self.db.cf_handle(&cf_name).is_none() {
            self.db.create_cf(&cf_name, &self.cf_options)?;
        }
        let bucket = Arc::new((self.reopen)(&self.db, &cf_name)?);
        buckets.insert(epoch, bucket.clone());
        Ok(bucket)
    }

    /// Drops the buckets of expired epochs: with `epochs_to_retain` = N, the
    /// buckets of the newest N epochs are kept and every older bucket is
    /// dropped wholesale. `0` keeps the newest bucket, exactly as `1` does,
    /// so a caller that clamps its own retention to at least 1 changes
    /// nothing here.
    ///
    /// Returns the earliest epoch to retain, `None` when there is no history
    /// at all. It is persisted before the drops and never moves backwards,
    /// so dropped epochs are never backfilled or recreated, even across a
    /// reopen or a raised `epochs_to_retain`. Writing below it is refused,
    /// and an epoch whose drop failed is gone from the store all the same:
    /// RocksDB unregisters the column family before dropping it, so the
    /// bucket can no longer be read, and the next open drops the column
    /// family it left on disk instead of serving that epoch again.
    ///
    /// A query racing a drop may report an error for the dropped epoch's
    /// rows; a retry no longer sees the bucket. Queries block for the
    /// duration of the drops, so callers on an async runtime must use
    /// `spawn_blocking`.
    ///
    /// `before_drop` runs for each expiring epoch, in ascending epoch order,
    /// while the write lock is held and before the column family is dropped.
    /// A store whose buckets have no side effects passes a closure that does
    /// nothing. An error from it leaves that epoch's bucket in place and
    /// stops the prune.
    pub(crate) fn prune(
        &self,
        epochs_to_retain: u64,
        mut before_drop: impl FnMut(EpochId, &Arc<B>) -> Result<(), TypedStoreError>,
    ) -> Result<Option<EpochId>, TypedStoreError> {
        // Runs once per executed checkpoint, where there is usually nothing
        // to drop and nothing to persist; that case must not take the write
        // lock queries block on.
        {
            let buckets = self.buckets.read();
            let persisted = self.earliest_retained();
            let Some(earliest_retained) =
                Self::earliest_epoch_to_retain(&buckets, epochs_to_retain, persisted)
            else {
                return Ok(None);
            };
            if earliest_retained == persisted && buckets.range(..earliest_retained).next().is_none()
            {
                return Ok(Some(earliest_retained));
            }
        }

        // The drops run under the map's write lock: `ensure` could otherwise
        // hand out a bucket for an epoch whose column family is dropped a
        // moment later.
        let mut buckets = self.buckets.write();
        let persisted = self.earliest_retained();
        let Some(earliest_retained) =
            Self::earliest_epoch_to_retain(&buckets, epochs_to_retain, persisted)
        else {
            return Ok(None);
        };
        if earliest_retained != persisted {
            // Persisted before dropping anything, so a reopen refuses the
            // dropped epochs from the start instead of backfilling them
            // again. Synced, because RocksDB makes a column-family drop
            // durable at once while a default write may still be lost, which
            // would leave the floor below an epoch that is already gone.
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
        // One column-family drop per epoch: constant time, no per-row
        // deletes and no compaction churn.
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
            // drop, so a failed drop leaves a bucket that can neither be read
            // nor dropped again; keeping it in the map would only break every
            // query that walks it.
            buckets.remove(&epoch);
        }
        Ok(Some(earliest_retained))
    }

    /// The earliest epoch to retain when the newest bucket in `buckets` is
    /// kept together with the `epochs_to_retain - 1` buckets below it, never
    /// below `persisted`. `None` when there is no bucket at all.
    ///
    /// Raising `epochs_to_retain` must not move the earliest retained epoch
    /// back down over epochs whose buckets are already gone: they would be
    /// backfilled and recreated, contradicting what queries were told.
    fn earliest_epoch_to_retain(
        buckets: &BTreeMap<EpochId, Arc<B>>,
        epochs_to_retain: u64,
        persisted: EpochId,
    ) -> Option<EpochId> {
        let (&newest, _) = buckets.last_key_value()?;
        Some(
            newest
                .saturating_sub(epochs_to_retain.saturating_sub(1))
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
    /// fresh temporary database. The returned guard must outlive the
    /// buckets, or the directory is removed while they still hold it open.
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
            .prune(2, |epoch, _| {
                seen.lock().unwrap().push(epoch);
                Ok(())
            })
            .unwrap();
        assert_eq!(earliest, Some(5));
        assert_eq!(*seen.lock().unwrap(), vec![3, 4]);
    }

    /// A callback error must abort that epoch's drop instead of leaving the
    /// bucket dropped with the store none the wiser.
    #[tokio::test]
    async fn a_callback_error_keeps_the_bucket() {
        let (buckets, _dir) = test_buckets(&[3, 4]);
        let result = buckets.prune(1, |_, _| Err(TypedStoreError::RocksDB("no".to_string())));
        assert!(result.is_err());
        assert_eq!(buckets.iter(false).len(), 2);
    }
}
