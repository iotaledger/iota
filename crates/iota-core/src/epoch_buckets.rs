// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-epoch column families ("buckets") for the stores that retain their
//! rows epoch by epoch, so that pruning an epoch is one column-family drop
//! instead of per-row deletes.
//!
//! The history stores keep their buckets' SST files outside the database
//! directory (see [`BucketPaths`]).

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
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
    rocksdb::{self, DBPath},
    traits::Map,
};

/// The directory in a node's live database directory that holds the
/// per-epoch history of all its stores.
pub const HISTORIC_DB_DIR: &str = "historic";

/// The root a database's buckets live under: `historic_db_path` if given,
/// otherwise [`HISTORIC_DB_DIR`] in the database directory itself.
pub(crate) fn historic_root(db_path: &Path, historic_db_path: Option<&Path>) -> PathBuf {
    historic_db_path.map_or_else(|| db_path.join(HISTORIC_DB_DIR), Path::to_path_buf)
}

/// Where one store keeps the SST files of its buckets: `epoch`'s column family
/// keeps them in `<root>/epoch_<epoch>/<store_dir>`.
///
/// RocksDB does not record where a column family's files are and looks for
/// them only where its options say, so every open of a database must pass
/// the same root.
#[derive(Clone, Debug)]
pub(crate) struct BucketPaths {
    root: PathBuf,
    store_dir: &'static str,
}

impl BucketPaths {
    pub(crate) fn new(root: &Path, store_dir: &'static str) -> Self {
        Self {
            root: root.to_path_buf(),
            store_dir,
        }
    }

    fn epoch_dir(&self, epoch: EpochId) -> PathBuf {
        self.root.join(format!("epoch_{epoch}"))
    }

    fn bucket_dir(&self, epoch: EpochId) -> PathBuf {
        self.epoch_dir(epoch).join(self.store_dir)
    }

    /// `options` with the SST files of `epoch`'s bucket placed in that
    /// epoch's directory.
    pub(crate) fn cf_options(
        &self,
        options: &rocksdb::Options,
        epoch: EpochId,
    ) -> rocksdb::Options {
        let mut options = options.clone();
        let path = DBPath::new(self.bucket_dir(epoch), u64::MAX)
            .expect("RocksDB allocates a path for every valid directory");
        options.set_cf_paths(&[path]);
        options
    }

    /// [`Self::cf_options`] for a bucket already on disk. Creates the
    /// bucket's directory if it is missing, as the open would fail without it.
    pub(crate) fn existing_cf_options(
        &self,
        options: &rocksdb::Options,
        epoch: EpochId,
    ) -> rocksdb::Options {
        if let Err(e) = self.create_dir(epoch) {
            warn!("{e}");
        }
        self.cf_options(options, epoch)
    }

    /// Creates the directory `epoch`'s bucket keeps its files in. RocksDB
    /// creates only the last component of a column family's path, not the
    /// directories above it.
    fn create_dir(&self, epoch: EpochId) -> Result<(), TypedStoreError> {
        let dir = self.bucket_dir(epoch);
        fs::create_dir_all(&dir)
            .map_err(|e| TypedStoreError::RocksDB(format!("cannot create {}: {e}", dir.display())))
    }

    /// The epochs that have a directory under the root, with that directory.
    fn epoch_dirs(&self) -> io::Result<Vec<(EpochId, PathBuf)>> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut dirs = Vec::new();
        for entry in entries {
            let entry = entry?;
            if let Some(epoch) = epoch_of_dir(&entry.file_name()) {
                dirs.push((epoch, entry.path()));
            }
        }
        Ok(dirs)
    }

    /// Deletes this store's directory of every epoch `has_bucket` says has no
    /// bucket, with whatever untracked files are left in it.
    ///
    /// Only for a database opened read-write, before it creates a bucket.
    fn remove_dirs_without_bucket(&self, has_bucket: impl Fn(EpochId) -> bool) {
        let dirs = match self.epoch_dirs() {
            Ok(dirs) => dirs,
            Err(e) => {
                warn!(root = ?self.root, "cannot list the historic directory: {e}");
                return;
            }
        };
        for (epoch, epoch_dir) in dirs {
            if has_bucket(epoch) {
                continue;
            }
            let dir = epoch_dir.join(self.store_dir);
            match fs::remove_dir_all(&dir) {
                Ok(()) => info!(?dir, "removed the files of a bucket that no longer exists"),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => warn!(?dir, "cannot remove the files of a dropped bucket: {e}"),
            }
            // Other stores' directories of the same epoch keep it in use.
            let _ = fs::remove_dir(&epoch_dir);
        }
    }

    /// Removes this store's directories of the epochs below
    /// `earliest_retained` that are empty, and the epoch directories they
    /// leave empty.
    ///
    /// RocksDB deletes a dropped bucket's files only once nothing reads it,
    /// so a directory still holding files is left for a later call.
    fn remove_empty_dirs_below(&self, earliest_retained: EpochId) {
        let Ok(dirs) = self.epoch_dirs() else {
            return;
        };
        for (_, epoch_dir) in dirs
            .into_iter()
            .filter(|(epoch, _)| *epoch < earliest_retained)
        {
            let _ = fs::remove_dir(epoch_dir.join(self.store_dir));
            let _ = fs::remove_dir(&epoch_dir);
        }
    }
}

/// The epoch an `epoch_<N>` directory under a historic root holds, `None` for
/// any other name.
fn epoch_of_dir(name: &OsStr) -> Option<EpochId> {
    name.to_str()?.strip_prefix("epoch_")?.parse().ok()
}

/// Options for the RPC index stores' history buckets, which are written once
/// and then read by range scans and exact-key probes. Every clone of the
/// returned options shares one block cache.
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

/// The `(name, options)` pairs a store's open must list for its buckets and
/// for `earliest_retained_cf`, the column family holding their retention
/// floor. With `paths`, every bucket's options place its files there.
///
/// Every bucket column family already on disk under `db_path` is included,
/// since auto-discovery would reopen it with default options and without its
/// `paths`. If the column families cannot be listed, only
/// `earliest_retained_cf` is returned.
pub(crate) fn extra_column_family_options(
    db_path: &Path,
    db_options: &DBOptions,
    cf_prefix: &str,
    earliest_retained_cf: &str,
    paths: Option<&BucketPaths>,
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
    options.extend(existing_cfs.into_iter().filter_map(|name| {
        let epoch = bucket_cf_epoch(cf_prefix, &name)?;
        let options = match paths {
            Some(paths) => DBOptions {
                options: paths.existing_cf_options(&cf_options.options, epoch),
                rw_options: cf_options.rw_options.clone(),
            },
            None => cf_options.clone(),
        };
        Some((name, options))
    }));
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

/// A store's view of one per-epoch bucket, which [`EpochBuckets`] opens from
/// the bucket's column-family name.
pub(crate) trait BucketReopen: Sized {
    fn reopen(db: &Arc<Database>, cf_name: &str) -> Result<Self, TypedStoreError>;
}

/// The per-epoch buckets of one store, each viewed as a `B`.
///
/// On-disk column-family names are the ground truth for which buckets exist;
/// the map here mirrors them for reads.
pub(crate) struct EpochBuckets<B> {
    db: Arc<Database>,
    /// What this store is called in log lines, which several stores write
    /// for the same epoch.
    name: &'static str,
    cf_prefix: &'static str,
    /// Template options for new buckets; all clones share one block cache.
    cf_options: rocksdb::Options,
    /// Where the buckets keep their files, `None` for a store that keeps them
    /// in the database directory.
    paths: Option<BucketPaths>,
    buckets: RwLock<BTreeMap<EpochId, Arc<B>>>,
    /// Mirrors the persisted retention floor; never moves backwards.
    earliest_retained_epoch: AtomicU64,
    earliest_retained_table: DBMap<(), EpochId>,
    /// Mirrors the oldest epoch in `buckets` ([`NO_BUCKET`] when empty),
    /// updated under the map's write lock. Kept apart from the map so that
    /// request-path readers do not block on a [`Self::prune`], which holds
    /// the write lock throughout.
    earliest_bucket_epoch: AtomicU64,
}

impl<B: BucketReopen> EpochBuckets<B> {
    /// Assembles the store's buckets from the ones found on disk, dropping
    /// those below the persisted retention floor, which a failed drop leaves
    /// behind.
    ///
    /// `paths` must be the ones `db` was opened with, and `db` must be open
    /// read-write: the directories of epochs without a bucket are deleted
    /// here.
    ///
    /// # Errors
    ///
    /// Fails if the retention floor cannot be read.
    pub(crate) fn open(
        db: Arc<Database>,
        name: &'static str,
        cf_prefix: &'static str,
        cf_options: rocksdb::Options,
        paths: Option<BucketPaths>,
        earliest_retained_table: DBMap<(), EpochId>,
        mut buckets: BTreeMap<EpochId, Arc<B>>,
    ) -> Result<Self, TypedStoreError> {
        let earliest_retained_epoch = earliest_retained_table.get(&())?.unwrap_or(0);
        // Listed from the database rather than taken from `buckets`: a column
        // family whose drop failed before this call is still there, and so
        // are its files.
        let on_disk: Option<BTreeSet<EpochId>> = match &paths {
            Some(_) => match list_tables(db.path_for_pruning().to_path_buf()) {
                Ok(cfs) => Some(
                    cfs.iter()
                        .filter_map(|cf_name| bucket_cf_epoch(cf_prefix, cf_name))
                        .collect(),
                ),
                Err(e) => {
                    warn!(store = name, "cannot list the bucket column families: {e}");
                    None
                }
            },
            None => None,
        };
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
        if let Some(paths) = &paths {
            if let Some(on_disk) = on_disk {
                paths.remove_dirs_without_bucket(|epoch| on_disk.contains(&epoch));
            }
            paths.remove_empty_dirs_below(earliest_retained_epoch);
        }
        let earliest_bucket_epoch = Self::earliest_epoch_of(&buckets);
        Ok(Self {
            db,
            name,
            cf_prefix,
            cf_options,
            paths,
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

    /// The bucket holding `epoch`'s rows, `None` when nothing has been
    /// written for that epoch yet or it has been pruned. Never creates one.
    pub(crate) fn get(&self, epoch: EpochId) -> Option<Arc<B>> {
        self.buckets.read().get(&epoch).cloned()
    }

    /// The bucket holding `epoch`'s rows, created if absent.
    ///
    /// # Errors
    ///
    /// Returns [`TypedStoreError::Pruned`] if `epoch` is below the retention
    /// floor.
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
            match &self.paths {
                Some(paths) => {
                    paths.create_dir(epoch)?;
                    self.db
                        .create_cf(&cf_name, &paths.cf_options(&self.cf_options, epoch))?
                }
                None => self.db.create_cf(&cf_name, &self.cf_options)?,
            }
        }
        let bucket = Arc::new(B::reopen(&self.db, &cf_name)?);
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
        if let Some(paths) = &self.paths {
            paths.remove_empty_dirs_below(earliest_retained);
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

    use typed_store::{
        database::wait_for_database_close,
        rocks::{DBMap, MetricConf, ReadWriteOptions, default_db_options, open_cf_opts},
    };

    use super::{
        Arc, BTreeMap, BucketPaths, BucketReopen, Database, EpochBuckets, EpochId, Map, Path,
        PathBuf, TypedStoreError, bucket_cf_epoch, bucket_cf_name, fs, rocksdb,
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

    impl BucketReopen for TestBucket {
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
            None,
            earliest_retained_table,
            buckets,
        )
        .unwrap();
        (buckets, dir)
    }

    /// The paths of a test store's buckets under `dir`, and the directory its
    /// database lives in.
    fn test_paths(dir: &Path) -> (BucketPaths, PathBuf) {
        let paths = BucketPaths {
            root: dir.join("historic"),
            store_dir: "test",
        };
        (paths, dir.join("db"))
    }

    /// An `EpochBuckets` over the database at `db_path` with a bucket for each
    /// of `epochs`, keeping their files under `paths`.
    fn placed_buckets(
        db_path: &Path,
        paths: &BucketPaths,
        epochs: &[EpochId],
    ) -> (EpochBuckets<TestBucket>, Arc<Database>) {
        placed_buckets_without(db_path, paths, epochs, &[])
    }

    /// Like [`placed_buckets`], but with the column families of `left_out`
    /// opened and kept out of the buckets, as a store does with one it failed
    /// to drop before calling `EpochBuckets::open`.
    fn placed_buckets_without(
        db_path: &Path,
        paths: &BucketPaths,
        epochs: &[EpochId],
        left_out: &[EpochId],
    ) -> (EpochBuckets<TestBucket>, Arc<Database>) {
        let db_options = default_db_options().options;
        let mut opt_cfs: Vec<(String, rocksdb::Options)> = epochs
            .iter()
            .chain(left_out)
            .map(|&epoch| {
                (
                    bucket_cf_name(TEST_CF_PREFIX, epoch),
                    paths.existing_cf_options(&db_options, epoch),
                )
            })
            .collect();
        opt_cfs.push((RETENTION_CF.to_string(), db_options.clone()));
        let opt_cfs: Vec<(&str, rocksdb::Options)> = opt_cfs
            .iter()
            .map(|(name, options)| (name.as_str(), options.clone()))
            .collect();
        let db = open_cf_opts(db_path, None, MetricConf::new("test"), &opt_cfs).unwrap();
        let earliest_retained_table: DBMap<(), EpochId> =
            DBMap::reopen(&db, Some(RETENTION_CF), &ReadWriteOptions::default(), true).unwrap();
        let buckets = EpochBuckets::open(
            db.clone(),
            "test buckets",
            TEST_CF_PREFIX,
            db_options,
            Some(paths.clone()),
            earliest_retained_table,
            epochs
                .iter()
                .map(|&epoch| (epoch, Arc::new(TestBucket)))
                .collect(),
        )
        .unwrap();
        (buckets, db)
    }

    fn bucket_table(db: &Arc<Database>, epoch: EpochId) -> DBMap<u64, u64> {
        DBMap::reopen(
            db,
            Some(&bucket_cf_name(TEST_CF_PREFIX, epoch)),
            &ReadWriteOptions::default(),
            false,
        )
        .unwrap()
    }

    /// A bucket writes its files into its epoch's directory rather than the
    /// database directory, and a reopen that names the same directory finds
    /// them there.
    #[tokio::test]
    async fn a_bucket_keeps_its_files_in_its_epoch_directory() {
        let dir = iota_common::tempdir();
        let (paths, db_path) = test_paths(dir.path());
        {
            let (buckets, db) = placed_buckets(&db_path, &paths, &[]);
            buckets.ensure(3).unwrap();
            let table = bucket_table(&db, 3);
            table.insert(&1, &2).unwrap();
            table.flush().unwrap();

            let files: Vec<_> = fs::read_dir(paths.bucket_dir(3))
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert!(
                files
                    .iter()
                    .any(|name| name.to_string_lossy().ends_with(".sst")),
                "the bucket's table file belongs in its epoch directory: {files:?}"
            );
            let weak_db = Arc::downgrade(&db);
            drop((buckets, table, db));
            assert!(wait_for_database_close(weak_db).await);
        }

        let (_buckets, db) = placed_buckets(&db_path, &paths, &[3]);
        assert_eq!(bucket_table(&db, 3).get(&1).unwrap(), Some(2));
    }

    /// A prune takes the directory of a bucket it drops along with the
    /// bucket, and the epoch's directory with it once no store has anything
    /// left there.
    #[tokio::test]
    async fn a_prune_removes_the_directories_of_the_buckets_it_drops() {
        let dir = iota_common::tempdir();
        let (paths, db_path) = test_paths(dir.path());
        let (buckets, _db) = placed_buckets(&db_path, &paths, &[]);
        buckets.ensure(1).unwrap();
        buckets.ensure(2).unwrap();
        assert!(paths.bucket_dir(1).exists());

        buckets.prune(2, 0, |_, _| Ok(())).unwrap();

        assert!(!paths.epoch_dir(1).exists());
        assert!(paths.bucket_dir(2).exists());
    }

    /// An open deletes this store's directory of every epoch it has no bucket
    /// for, files and all, and leaves another store's directory of the same
    /// epoch alone.
    #[tokio::test]
    async fn an_open_removes_the_directories_of_buckets_that_no_longer_exist() {
        let dir = iota_common::tempdir();
        let (paths, db_path) = test_paths(dir.path());
        fs::create_dir_all(paths.bucket_dir(9)).unwrap();
        fs::write(
            paths.bucket_dir(9).join("000012.sst"),
            b"left by a wiped database",
        )
        .unwrap();
        let other_store = paths.epoch_dir(9).join("other");
        fs::create_dir_all(&other_store).unwrap();

        let (_buckets, _db) = placed_buckets(&db_path, &paths, &[]);

        assert!(!paths.bucket_dir(9).exists());
        assert!(other_store.exists());
    }

    /// A column family the store kept out of its buckets, as after a failed
    /// drop, keeps its directory: the column family is still in the
    /// database, and the next open needs its files.
    #[tokio::test]
    async fn an_open_keeps_the_directory_of_a_column_family_still_in_the_database() {
        let dir = iota_common::tempdir();
        let (paths, db_path) = test_paths(dir.path());
        {
            let (buckets, db) = placed_buckets(&db_path, &paths, &[]);
            buckets.ensure(3).unwrap();
            let table = bucket_table(&db, 3);
            table.insert(&1, &2).unwrap();
            table.flush().unwrap();
            let weak_db = Arc::downgrade(&db);
            drop((buckets, table, db));
            assert!(wait_for_database_close(weak_db).await);
        }
        {
            let (buckets, db) = placed_buckets_without(&db_path, &paths, &[], &[3]);
            assert!(paths.bucket_dir(3).exists());
            let weak_db = Arc::downgrade(&db);
            drop((buckets, db));
            assert!(wait_for_database_close(weak_db).await);
        }

        let (_buckets, db) = placed_buckets(&db_path, &paths, &[3]);
        assert_eq!(bucket_table(&db, 3).get(&1).unwrap(), Some(2));
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
