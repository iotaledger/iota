// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod errors;
pub mod metrics;
pub mod safe_iter;

use std::{
    collections::HashSet,
    env,
    ffi::CStr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use backoff::backoff::Backoff;
use iota_macros::{fail_point, nondeterministic};
use rocksdb::{
    AsColumnFamilyRef, BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBPinnableSlice,
    DBWithThreadMode, LiveFile, MultiThreaded, checkpoint::Checkpoint, properties,
    properties::num_files_at_level,
};
pub use rocksdb::Options;
use tap::TapFallible;
use tracing::{info, instrument, warn};
use typed_store_error::TypedStoreError;

use crate::engine::{RawDbIterator, StorageEngine};
use errors::typed_store_err_from_rocks_err;
use metrics::DBMetrics;

// Write buffer size per RocksDB instance can be set via the env var below.
// If the env var is not set, use the default value in MiB.
const ENV_VAR_DB_WRITE_BUFFER_SIZE: &str = "DB_WRITE_BUFFER_SIZE_MB";
const DEFAULT_DB_WRITE_BUFFER_SIZE: usize = 1024;

// Write ahead log size per RocksDB instance can be set via the env var below.
// If the env var is not set, use the default value in MiB.
const ENV_VAR_DB_WAL_SIZE: &str = "DB_WAL_SIZE_MB";
const DEFAULT_DB_WAL_SIZE: usize = 1024;

// Environment variable to control behavior of write throughput optimized
// tables.
const ENV_VAR_L0_NUM_FILES_COMPACTION_TRIGGER: &str = "L0_NUM_FILES_COMPACTION_TRIGGER";
const DEFAULT_L0_NUM_FILES_COMPACTION_TRIGGER: usize = 4;
const DEFAULT_UNIVERSAL_COMPACTION_L0_NUM_FILES_COMPACTION_TRIGGER: usize = 80;
const ENV_VAR_MAX_WRITE_BUFFER_SIZE_MB: &str = "MAX_WRITE_BUFFER_SIZE_MB";
const DEFAULT_MAX_WRITE_BUFFER_SIZE_MB: usize = 256;
const ENV_VAR_MAX_WRITE_BUFFER_NUMBER: &str = "MAX_WRITE_BUFFER_NUMBER";
const DEFAULT_MAX_WRITE_BUFFER_NUMBER: usize = 6;
const ENV_VAR_TARGET_FILE_SIZE_BASE_MB: &str = "TARGET_FILE_SIZE_BASE_MB";
const DEFAULT_TARGET_FILE_SIZE_BASE_MB: usize = 128;

// Set to 1 to disable blob storage for transactions and effects.
const ENV_VAR_DISABLE_BLOB_STORAGE: &str = "DISABLE_BLOB_STORAGE";

const ENV_VAR_DB_PARALLELISM: &str = "DB_PARALLELISM";

// TODO: remove this after Rust rocksdb has the TOTAL_BLOB_FILES_SIZE property
// built-in. From https://github.com/facebook/rocksdb/blob/bd80433c73691031ba7baa65c16c63a83aef201a/include/rocksdb/db.h#L1169
const ROCKSDB_PROPERTY_TOTAL_BLOB_FILES_SIZE: &CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked("rocksdb.total-blob-file-size\0".as_bytes()) };

const DB_CORRUPTED_KEY: &[u8] = b"db_corrupted";

#[cfg(test)]
mod tests;

use crate::store::{DBMap, Database, MetricConf};

// ---------------------------------------------------------------------------
// ReadWriteOptions — RocksDB-specific semantic read/write configuration
// ---------------------------------------------------------------------------

/// Semantic options for a `DBMap<K, V, RocksDB>` instance.
///
/// These are converted into backend-specific `RocksReadOptions` /
/// `RocksWriteOptions` when a column-family map is opened via `reopen`.
#[derive(Clone, Debug)]
pub struct ReadWriteOptions {
    pub ignore_range_deletions: bool,
    pub(crate) sync_to_disk: bool,
}

impl ReadWriteOptions {
    pub fn set_ignore_range_deletions(mut self, ignore: bool) -> Self {
        self.ignore_range_deletions = ignore;
        self
    }
}

impl Default for ReadWriteOptions {
    fn default() -> Self {
        Self {
            ignore_range_deletions: true,
            sync_to_disk: std::env::var("IOTA_DB_SYNC_TO_DISK").is_ok_and(|v| v != "0"),
        }
    }
}

// ---------------------------------------------------------------------------
// RocksReadOptions / RocksWriteOptions — cloneable wrappers around rocksdb
// option types used as StorageEngine associated types
// ---------------------------------------------------------------------------

/// Cloneable read-option state for the RocksDB backend.
///
/// Stores the semantic flags and iteration bounds that are translated into a
/// `rocksdb::ReadOptions` just-in-time inside each `StorageEngine` method.
#[derive(Clone, Debug)]
pub struct RocksReadOptions {
    pub(crate) ignore_range_deletions: bool,
    pub(crate) lower_bound: Option<Vec<u8>>,
    pub(crate) upper_bound: Option<Vec<u8>>,
}

impl Default for RocksReadOptions {
    fn default() -> Self {
        Self {
            ignore_range_deletions: true,
            lower_bound: None,
            upper_bound: None,
        }
    }
}

impl RocksReadOptions {
    fn to_rocksdb(&self) -> rocksdb::ReadOptions {
        let mut opts = rocksdb::ReadOptions::default();
        opts.set_ignore_range_deletions(self.ignore_range_deletions);
        if let Some(lb) = self.lower_bound.clone() {
            opts.set_iterate_lower_bound(lb);
        }
        if let Some(ub) = self.upper_bound.clone() {
            opts.set_iterate_upper_bound(ub);
        }
        opts
    }
}

/// Cloneable write-option state for the RocksDB backend.
#[derive(Clone, Debug)]
pub struct RocksWriteOptions {
    pub(crate) sync_to_disk: bool,
}

impl Default for RocksWriteOptions {
    fn default() -> Self {
        Self {
            sync_to_disk: std::env::var("IOTA_DB_SYNC_TO_DISK").is_ok_and(|v| v != "0"),
        }
    }
}

impl RocksWriteOptions {
    fn to_rocksdb(&self) -> rocksdb::WriteOptions {
        let mut opts = rocksdb::WriteOptions::default();
        opts.set_sync(self.sync_to_disk);
        opts
    }
}

/// A helper macro to reopen multiple column families. The macro returns
/// a tuple of DBMap structs in the same order that the column families
/// are defined.
///
/// # Arguments
///
/// * `db` - a reference to a rocks DB object
/// * `cf;<ty,ty>` - a comma separated list of column families to open. For each
///   column family a concatenation of column family name (cf) and Key-Value
///   <ty, ty> should be provided.
///
/// # Examples
///
/// We successfully open two different column families.
/// ```
/// use typed_store::reopen;
/// use typed_store::rocks::*;
/// use tempfile::tempdir;
/// use prometheus::Registry;
/// use std::sync::Arc;
/// use core::fmt::Error;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
/// const FIRST_CF: &str = "First_CF";
/// const SECOND_CF: &str = "Second_CF";
///
///
/// /// Create the rocks database reference for the desired column families
/// let rocks = open_cf(tempdir().unwrap(), None, MetricConf::default(), &[FIRST_CF, SECOND_CF]).unwrap();
///
/// /// Now simply open all the column families for their expected Key-Value types
/// let (db_map_1, db_map_2) = reopen!(&rocks, FIRST_CF;<i32, String>, SECOND_CF;<i32, String>);
/// Ok(())
/// }
/// ```
#[macro_export]
macro_rules! reopen {
    ( $db:expr, $($cf:expr;<$K:ty, $V:ty>),*) => {
        (
            $(
                DBMap::<$K, $V>::reopen($db, Some($cf), &ReadWriteOptions::default(), false).expect(&format!("Cannot open {} CF.", $cf)[..])
            ),*
        )
    };
}

// ---------------------------------------------------------------------------
// RocksDB backend
// ---------------------------------------------------------------------------

pub type RawIter<'a> =
    rocksdb::DBRawIteratorWithThreadMode<'a, DBWithThreadMode<MultiThreaded>>;

impl RawDbIterator for RawIter<'_> {
    fn seek_to_first(&mut self) {
        self.seek_to_first();
    }

    fn seek_to_last(&mut self) {
        self.seek_to_last();
    }

    fn seek(&mut self, key: &[u8]) {
        self.seek(key);
    }

    fn seek_for_prev(&mut self, key: &[u8]) {
        self.seek_for_prev(key);
    }

    fn valid(&self) -> bool {
        self.valid()
    }

    fn key(&self) -> Option<&[u8]> {
        self.key()
    }

    fn value(&self) -> Option<&[u8]> {
        self.value()
    }

    fn next(&mut self) {
        self.next();
    }

    fn prev(&mut self) {
        self.prev();
    }

    fn status(&self) -> Result<(), TypedStoreError> {
        self.status()
            .map_err(|e| TypedStoreError::RocksDB(format!("{e}")))
    }
}

#[derive(Debug)]
pub struct RocksDB {
    pub(crate) underlying: rocksdb::DBWithThreadMode<MultiThreaded>,
}

impl Drop for RocksDB {
    fn drop(&mut self) {
        self.underlying.cancel_all_background_work(/* wait */ true);
    }
}

impl RocksDB {
    /// Retrieve a column family handle, panicking if it does not exist.
    fn rocks_cf(&self, cf_name: &str) -> Arc<rocksdb::BoundColumnFamily<'_>> {
        self.underlying
            .cf_handle(cf_name)
            .expect("Map-keying column family should have been checked at DB creation")
    }

    fn get_int_property(
        &self,
        cf: &impl AsColumnFamilyRef,
        property_name: &std::ffi::CStr,
    ) -> Result<i64, TypedStoreError> {
        match self.underlying.property_int_value_cf(cf, property_name) {
            Ok(Some(value)) => Ok(value.min(i64::MAX as u64).try_into().unwrap_or_default()),
            Ok(None) => Ok(0),
            Err(e) => Err(TypedStoreError::RocksDB(e.into_string())),
        }
    }
}

impl StorageEngine for RocksDB {
    type Batch = rocksdb::WriteBatch;
    type GetValue<'a>
        = DBPinnableSlice<'a>
    where
        Self: 'a;
    type ReadOptions = RocksReadOptions;
    type WriteOptions = RocksWriteOptions;
    type CfOptions = rocksdb::Options;
    type RawIter<'a>
        = RawIter<'a>
    where
        Self: 'a;
    type Metrics = DBMetrics;

    fn get_metrics() -> Arc<DBMetrics> {
        DBMetrics::get().clone()
    }

    fn set_iter_lower_bound(opts: &mut RocksReadOptions, bound: Vec<u8>) {
        opts.lower_bound = Some(bound);
    }

    fn set_iter_upper_bound(opts: &mut RocksReadOptions, bound: Vec<u8>) {
        opts.upper_bound = Some(bound);
    }

    fn default_cf_options() -> rocksdb::Options {
        default_db_options().options
    }

    fn get<K: AsRef<[u8]>>(
        &self,
        cf_name: &str,
        key: K,
        readopts: &RocksReadOptions,
    ) -> Result<Option<DBPinnableSlice<'_>>, TypedStoreError> {
        self.underlying
            .get_pinned_cf_opt(&self.rocks_cf(cf_name), key, &readopts.to_rocksdb())
            .map_err(typed_store_err_from_rocks_err)
    }

    fn multi_get<I, K>(
        &self,
        cf_name: &str,
        keys: I,
        readopts: &RocksReadOptions,
    ) -> Vec<Result<Option<DBPinnableSlice<'_>>, TypedStoreError>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        self.underlying
            .batched_multi_get_cf_opt(&self.rocks_cf(cf_name), keys, false, &readopts.to_rocksdb())
            .into_iter()
            .map(|r| r.map_err(typed_store_err_from_rocks_err))
            .collect()
    }

    fn put(
        &self,
        cf_name: &str,
        key: Vec<u8>,
        value: Vec<u8>,
        writeopts: &RocksWriteOptions,
    ) -> Result<(), TypedStoreError> {
        fail_point!("put-cf-before");
        let ret = self
            .underlying
            .put_cf_opt(&self.rocks_cf(cf_name), key, value, &writeopts.to_rocksdb())
            .map_err(typed_store_err_from_rocks_err);
        fail_point!("put-cf-after");
        #[allow(clippy::let_and_return)]
        ret
    }

    fn delete(
        &self,
        cf_name: &str,
        key: &[u8],
        writeopts: &RocksWriteOptions,
    ) -> Result<(), TypedStoreError> {
        fail_point!("delete-cf-before");
        let ret = self
            .underlying
            .delete_cf_opt(&self.rocks_cf(cf_name), key, &writeopts.to_rocksdb())
            .map_err(typed_store_err_from_rocks_err);
        fail_point!("delete-cf-after");
        #[allow(clippy::let_and_return)]
        ret
    }

    fn new_batch(&self) -> rocksdb::WriteBatch {
        rocksdb::WriteBatch::default()
    }

    fn batch_put(
        &self,
        batch: &mut rocksdb::WriteBatch,
        cf_name: &str,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        batch.put_cf(&self.rocks_cf(cf_name), key, value);
        Ok(())
    }

    fn batch_delete(
        &self,
        batch: &mut rocksdb::WriteBatch,
        cf_name: &str,
        key: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        batch.delete_cf(&self.rocks_cf(cf_name), key);
        Ok(())
    }

    fn batch_delete_range(
        &self,
        batch: &mut rocksdb::WriteBatch,
        cf_name: &str,
        from: Vec<u8>,
        to: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        batch.delete_range_cf(&self.rocks_cf(cf_name), from, to);
        Ok(())
    }

    fn batch_size_in_bytes(&self, batch: &rocksdb::WriteBatch) -> usize {
        batch.size_in_bytes()
    }

    fn write_batch(
        &self,
        batch: rocksdb::WriteBatch,
        writeopts: &RocksWriteOptions,
    ) -> Result<(), TypedStoreError> {
        fail_point!("batch-write-before");
        let ret = self
            .underlying
            .write_opt(batch, &writeopts.to_rocksdb())
            .map_err(typed_store_err_from_rocks_err);
        fail_point!("batch-write-after");
        #[allow(clippy::let_and_return)]
        ret
    }

    fn create_cf(&self, name: &str, opts: &rocksdb::Options) -> Result<(), TypedStoreError> {
        self.underlying
            .create_cf(name, opts)
            .map_err(typed_store_err_from_rocks_err)
    }

    fn has_cf(&self, name: &str) -> bool {
        self.underlying.cf_handle(name).is_some()
    }

    fn drop_cf(&self, name: &str) -> Result<(), TypedStoreError> {
        self.underlying
            .drop_cf(name)
            .map_err(typed_store_err_from_rocks_err)
    }

    fn flush(&self) -> Result<(), TypedStoreError> {
        self.underlying
            .flush()
            .map_err(typed_store_err_from_rocks_err)
    }

    fn checkpoint(&self, path: &Path) -> Result<(), TypedStoreError> {
        let checkpoint =
            Checkpoint::new(&self.underlying).map_err(typed_store_err_from_rocks_err)?;
        checkpoint
            .create_checkpoint(path)
            .map_err(|e| TypedStoreError::RocksDB(e.to_string()))
    }

    fn compact_range(&self, cf_name: &str, start: Option<&[u8]>, end: Option<&[u8]>) {
        self.underlying
            .compact_range_cf(&self.rocks_cf(cf_name), start, end);
    }

    fn raw_iterator<'a>(&'a self, cf_name: &str, readopts: RocksReadOptions) -> RawIter<'a> {
        self.underlying
            .raw_iterator_cf_opt(&self.rocks_cf(cf_name), readopts.to_rocksdb())
    }

    fn key_may_exist(&self, cf_name: &str, key: &[u8], readopts: &RocksReadOptions) -> bool {
        self.underlying
            .key_may_exist_cf_opt(&self.rocks_cf(cf_name), key, &readopts.to_rocksdb())
    }

    fn try_catch_up_with_primary(&self) -> Result<(), TypedStoreError> {
        self.underlying
            .try_catch_up_with_primary()
            .map_err(typed_store_err_from_rocks_err)
    }

    fn report_cf_metrics(&self, cf_name: &str, db_name: &str, db_metrics: &Arc<Self::Metrics>) {
        let Some(cf) = self.underlying.cf_handle(cf_name) else {
            tracing::warn!(
                "unable to report metrics for cf {cf_name:?} in db {db_name:?}",
            );
            return;
        };

        const METRICS_ERROR: i64 = -1;

        db_metrics
            .cf_metrics
            .rocksdb_total_sst_files_size
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::TOTAL_SST_FILES_SIZE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_total_blob_files_size
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, ROCKSDB_PROPERTY_TOTAL_BLOB_FILES_SIZE)
                    .unwrap_or(METRICS_ERROR),
            );
        let total_num_files: i64 = (0..=6)
            .map(|level| {
                self.get_int_property(&cf, &num_files_at_level(level))
                    .unwrap_or(METRICS_ERROR)
            })
            .sum();
        db_metrics
            .cf_metrics
            .rocksdb_total_num_files
            .with_label_values(&[cf_name])
            .set(total_num_files);
        db_metrics
            .cf_metrics
            .rocksdb_num_level0_files
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, &num_files_at_level(0))
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_current_size_active_mem_tables
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::CUR_SIZE_ACTIVE_MEM_TABLE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_size_all_mem_tables
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::SIZE_ALL_MEM_TABLES)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_snapshots
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::NUM_SNAPSHOTS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_oldest_snapshot_time
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::OLDEST_SNAPSHOT_TIME)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_actual_delayed_write_rate
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::ACTUAL_DELAYED_WRITE_RATE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_is_write_stopped
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::IS_WRITE_STOPPED)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_block_cache_capacity
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::BLOCK_CACHE_CAPACITY)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_block_cache_usage
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::BLOCK_CACHE_USAGE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_block_cache_pinned_usage
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::BLOCK_CACHE_PINNED_USAGE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimate_table_readers_mem
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::ESTIMATE_TABLE_READERS_MEM)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimated_num_keys
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::ESTIMATE_NUM_KEYS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_immutable_mem_tables
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::NUM_IMMUTABLE_MEM_TABLE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_mem_table_flush_pending
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::MEM_TABLE_FLUSH_PENDING)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_compaction_pending
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::COMPACTION_PENDING)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimate_pending_compaction_bytes
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::ESTIMATE_PENDING_COMPACTION_BYTES)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_running_compactions
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::NUM_RUNNING_COMPACTIONS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_running_flushes
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::NUM_RUNNING_FLUSHES)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimate_oldest_key_time
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::ESTIMATE_OLDEST_KEY_TIME)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_background_errors
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::BACKGROUND_ERRORS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_base_level
            .with_label_values(&[cf_name])
            .set(
                self.get_int_property(&cf, properties::BASE_LEVEL)
                    .unwrap_or(METRICS_ERROR),
            );
    }
}

// ---------------------------------------------------------------------------
// RocksDB-specific extensions on Database<RocksDB>
// ---------------------------------------------------------------------------

impl Database<RocksDB> {
    /// Return the filesystem path (used for SST file pruning).
    pub fn path_for_pruning(&self) -> &Path {
        self.storage.underlying.path()
    }

    /// Return a list of live SST files.
    pub fn live_files(&self) -> Result<Vec<LiveFile>, rocksdb::Error> {
        self.storage.underlying.live_files()
    }
}

// ---------------------------------------------------------------------------
// RocksDB-specific DBMap impl — reopen using ReadWriteOptions
// ---------------------------------------------------------------------------

impl<K, V> DBMap<K, V, RocksDB> {
    /// Reopens an open database as a typed map operating under a specific
    /// column family. If no column family is passed, the default column
    /// family is used.
    ///
    /// ```
    /// use core::fmt::Error;
    /// use std::sync::Arc;
    ///
    /// use prometheus::Registry;
    /// use tempfile::tempdir;
    /// use typed_store::rocks::{metrics::DBMetrics, *};
    /// #[tokio::main]
    /// async fn main() -> Result<(), Error> {
    ///     /// Open the DB with all needed column families first.
    ///     let rocks = open_cf(
    ///         tempdir().unwrap(),
    ///         None,
    ///         MetricConf::default(),
    ///         &["First_CF", "Second_CF"],
    ///     )
    ///     .unwrap();
    ///     /// Attach the column families to specific maps.
    ///     let db_cf_1 = DBMap::<u32, u32>::reopen(
    ///         &rocks,
    ///         Some("First_CF"),
    ///         &ReadWriteOptions::default(),
    ///         false,
    ///     )
    ///     .expect("Failed to open storage");
    ///     Ok(())
    /// }
    /// ```
    #[instrument(level = "debug", skip(db), err)]
    pub fn reopen(
        db: &Arc<Database<RocksDB>>,
        opt_cf: Option<&str>,
        rw_options: &ReadWriteOptions,
        is_deprecated: bool,
    ) -> Result<Self, TypedStoreError> {
        let cf_key = opt_cf.unwrap_or("default").to_owned();
        let read_opts = RocksReadOptions {
            ignore_range_deletions: rw_options.ignore_range_deletions,
            lower_bound: None,
            upper_bound: None,
        };
        let write_opts = RocksWriteOptions {
            sync_to_disk: rw_options.sync_to_disk,
        };
        Ok(DBMap::new(db.clone(), read_opts, write_opts, &cf_key, is_deprecated))
    }
}

// ---------------------------------------------------------------------------
// Corruption helpers (RocksDB-specific, not part of StorageEngine)
// ---------------------------------------------------------------------------

pub fn check_and_mark_db_corruption(path: &Path) -> Result<(), String> {
    let db = rocksdb::DB::open_default(path).map_err(|e| e.to_string())?;

    db.get(DB_CORRUPTED_KEY)
        .map_err(|e| format!("Failed to open database: {e}"))
        .and_then(|value| match value {
            Some(v) if v[0] == 1 => Err(
                "Database is corrupted, please remove the current database and start clean!"
                    .to_string(),
            ),
            Some(_) => Ok(()),
            None => db
                .put(DB_CORRUPTED_KEY, [1])
                .map_err(|e| format!("Failed to set corrupted key in database: {e}")),
        })?;

    Ok(())
}

pub fn unmark_db_corruption(path: &Path) -> Result<(), rocksdb::Error> {
    rocksdb::DB::open_default(path)?.put(DB_CORRUPTED_KEY, [0])
}

// ---------------------------------------------------------------------------
// DBOptions
// ---------------------------------------------------------------------------

// TODO: refactor this into a builder pattern, where rocksdb::Options are
// generated after a call to build().
#[derive(Default, Clone)]
pub struct DBOptions {
    pub options: rocksdb::Options,
    pub rw_options: ReadWriteOptions,
}

impl DBOptions {
    pub fn optimize_for_point_lookup(mut self, block_cache_size_mb: usize) -> DBOptions {
        self.options
            .optimize_for_point_lookup(block_cache_size_mb as u64);
        self
    }

    pub fn optimize_for_large_values_no_scan(mut self, min_blob_size: u64) -> DBOptions {
        if env::var(ENV_VAR_DISABLE_BLOB_STORAGE).is_ok() {
            info!("Large value blob storage optimization is disabled via env var.");
            return self;
        }

        self.options.set_enable_blob_files(true);
        self.options
            .set_blob_compression_type(rocksdb::DBCompressionType::Lz4);
        self.options.set_enable_blob_gc(true);
        self.options.set_min_blob_size(min_blob_size);

        let write_buffer_size = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_SIZE_MB)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_SIZE_MB)
            * 1024
            * 1024;
        self.options.set_write_buffer_size(write_buffer_size);
        let target_file_size_base = 64 << 20;
        self.options
            .set_target_file_size_base(target_file_size_base);
        let max_level_zero_file_num = read_size_from_env(ENV_VAR_L0_NUM_FILES_COMPACTION_TRIGGER)
            .unwrap_or(DEFAULT_L0_NUM_FILES_COMPACTION_TRIGGER);
        self.options
            .set_max_bytes_for_level_base(target_file_size_base * max_level_zero_file_num as u64);

        self
    }

    pub fn optimize_for_read(mut self, block_cache_size_mb: usize) -> DBOptions {
        self.options
            .set_block_based_table_factory(&get_block_options(block_cache_size_mb, 16 << 10));
        self
    }

    pub fn optimize_db_for_write_throughput(mut self, db_max_write_buffer_gb: u64) -> DBOptions {
        self.options
            .set_db_write_buffer_size(db_max_write_buffer_gb as usize * 1024 * 1024 * 1024);
        self.options
            .set_max_total_wal_size(db_max_write_buffer_gb * 1024 * 1024 * 1024);
        self
    }

    pub fn optimize_for_write_throughput(mut self) -> DBOptions {
        let write_buffer_size = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_SIZE_MB)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_SIZE_MB)
            * 1024
            * 1024;
        self.options.set_write_buffer_size(write_buffer_size);
        let max_write_buffer_number = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_NUMBER)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_NUMBER);
        self.options
            .set_max_write_buffer_number(max_write_buffer_number.try_into().unwrap());
        self.options
            .set_max_write_buffer_size_to_maintain((write_buffer_size).try_into().unwrap());

        let max_level_zero_file_num = read_size_from_env(ENV_VAR_L0_NUM_FILES_COMPACTION_TRIGGER)
            .unwrap_or(DEFAULT_L0_NUM_FILES_COMPACTION_TRIGGER);
        self.options.set_level_zero_file_num_compaction_trigger(
            max_level_zero_file_num.try_into().unwrap(),
        );
        self.options.set_level_zero_slowdown_writes_trigger(
            (max_level_zero_file_num * 12).try_into().unwrap(),
        );
        self.options
            .set_level_zero_stop_writes_trigger((max_level_zero_file_num * 16).try_into().unwrap());
        self.options.set_target_file_size_base(
            read_size_from_env(ENV_VAR_TARGET_FILE_SIZE_BASE_MB)
                .unwrap_or(DEFAULT_TARGET_FILE_SIZE_BASE_MB) as u64
                * 1024
                * 1024,
        );
        self.options
            .set_max_bytes_for_level_base((write_buffer_size * max_level_zero_file_num) as u64);

        self
    }

    pub fn optimize_for_write_throughput_no_deletion(mut self) -> DBOptions {
        let write_buffer_size = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_SIZE_MB)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_SIZE_MB)
            * 1024
            * 1024;
        self.options.set_write_buffer_size(write_buffer_size);
        let max_write_buffer_number = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_NUMBER)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_NUMBER);
        self.options
            .set_max_write_buffer_number(max_write_buffer_number.try_into().unwrap());
        self.options
            .set_max_write_buffer_size_to_maintain((write_buffer_size).try_into().unwrap());

        self.options
            .set_compaction_style(rocksdb::DBCompactionStyle::Universal);
        let mut compaction_options = rocksdb::UniversalCompactOptions::default();
        compaction_options.set_max_size_amplification_percent(10000);
        compaction_options.set_stop_style(rocksdb::UniversalCompactionStopStyle::Similar);
        self.options
            .set_universal_compaction_options(&compaction_options);

        let max_level_zero_file_num = read_size_from_env(ENV_VAR_L0_NUM_FILES_COMPACTION_TRIGGER)
            .unwrap_or(DEFAULT_UNIVERSAL_COMPACTION_L0_NUM_FILES_COMPACTION_TRIGGER);
        self.options.set_level_zero_file_num_compaction_trigger(
            max_level_zero_file_num.try_into().unwrap(),
        );
        self.options.set_level_zero_slowdown_writes_trigger(
            (max_level_zero_file_num * 12).try_into().unwrap(),
        );
        self.options
            .set_level_zero_stop_writes_trigger((max_level_zero_file_num * 16).try_into().unwrap());
        self.options.set_target_file_size_base(
            read_size_from_env(ENV_VAR_TARGET_FILE_SIZE_BASE_MB)
                .unwrap_or(DEFAULT_TARGET_FILE_SIZE_BASE_MB) as u64
                * 1024
                * 1024,
        );
        self.options
            .set_max_bytes_for_level_base((write_buffer_size * max_level_zero_file_num) as u64);

        self
    }

    pub fn set_block_options(
        mut self,
        block_cache_size_mb: usize,
        block_size_bytes: usize,
    ) -> DBOptions {
        self.options
            .set_block_based_table_factory(&get_block_options(
                block_cache_size_mb,
                block_size_bytes,
            ));
        self
    }

    pub fn disable_write_throttling(mut self) -> DBOptions {
        self.options.set_soft_pending_compaction_bytes_limit(0);
        self.options.set_hard_pending_compaction_bytes_limit(0);
        self
    }
}

/// Creates a default RocksDB option, to be used when RocksDB option is
/// unspecified.
pub fn default_db_options() -> DBOptions {
    let mut opt = rocksdb::Options::default();

    if let Some(limit) = fdlimit::raise_fd_limit() {
        opt.set_max_open_files((limit / 8) as i32);
    }

    opt.set_table_cache_num_shard_bits(10);

    opt.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opt.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
    opt.set_bottommost_zstd_max_train_bytes(1024 * 1024, true);

    opt.set_db_write_buffer_size(
        read_size_from_env(ENV_VAR_DB_WRITE_BUFFER_SIZE).unwrap_or(DEFAULT_DB_WRITE_BUFFER_SIZE)
            * 1024
            * 1024,
    );
    opt.set_max_total_wal_size(
        read_size_from_env(ENV_VAR_DB_WAL_SIZE).unwrap_or(DEFAULT_DB_WAL_SIZE) as u64 * 1024 * 1024,
    );

    opt.increase_parallelism(read_size_from_env(ENV_VAR_DB_PARALLELISM).unwrap_or(8) as i32);

    opt.set_enable_pipelined_write(true);

    opt.set_block_based_table_factory(&get_block_options(128, 16 << 10));

    opt.set_memtable_prefix_bloom_ratio(0.02);

    DBOptions {
        options: opt,
        rw_options: ReadWriteOptions::default(),
    }
}

fn get_block_options(block_cache_size_mb: usize, block_size_bytes: usize) -> BlockBasedOptions {
    let mut block_options = BlockBasedOptions::default();
    block_options.set_block_size(block_size_bytes);
    block_options.set_block_cache(&Cache::new_lru_cache(block_cache_size_mb << 20));
    block_options.set_bloom_filter(10.0, false);
    block_options.set_pin_l0_filter_and_index_blocks_in_cache(true);
    block_options
}

// ---------------------------------------------------------------------------
// DB open helpers
// ---------------------------------------------------------------------------

pub fn read_size_from_env(var_name: &str) -> Option<usize> {
    env::var(var_name)
        .ok()?
        .parse::<usize>()
        .tap_err(|e| {
            warn!(
                "Env var {} does not contain valid usize integer: {}",
                var_name, e
            )
        })
        .ok()
}

/// Opens a database with options, and a number of column families that are
/// created if they do not exist.
#[instrument(level="debug", skip_all, fields(path = ?path.as_ref(), cf = ?opt_cfs), err)]
pub fn open_cf<P: AsRef<Path>>(
    path: P,
    db_options: Option<rocksdb::Options>,
    metric_conf: MetricConf,
    opt_cfs: &[&str],
) -> Result<Arc<Database>, TypedStoreError> {
    let options = db_options.unwrap_or_else(|| default_db_options().options);
    let column_descriptors: Vec<_> = opt_cfs
        .iter()
        .map(|name| (*name, options.clone()))
        .collect();
    open_cf_opts(
        path,
        Some(options.clone()),
        metric_conf,
        &column_descriptors[..],
    )
}

fn prepare_db_options(db_options: Option<rocksdb::Options>) -> rocksdb::Options {
    let mut options = db_options.unwrap_or_else(|| default_db_options().options);
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    options
}

/// Opens a database with options, and a number of column families with
/// individual options that are created if they do not exist.
#[instrument(level="debug", skip_all, fields(path = ?path.as_ref()), err)]
pub fn open_cf_opts<P: AsRef<Path>>(
    path: P,
    db_options: Option<rocksdb::Options>,
    metric_conf: MetricConf,
    opt_cfs: &[(&str, rocksdb::Options)],
) -> Result<Arc<Database>, TypedStoreError> {
    let path = path.as_ref();
    let cfs = populate_missing_cfs(opt_cfs, path).map_err(typed_store_err_from_rocks_err)?;
    nondeterministic!({
        let options = prepare_db_options(db_options);
        let rocksdb = {
            rocksdb::DBWithThreadMode::<MultiThreaded>::open_cf_descriptors(
                &options,
                path,
                cfs.into_iter()
                    .map(|(name, opts)| ColumnFamilyDescriptor::new(name, opts)),
            )
            .map_err(typed_store_err_from_rocks_err)?
        };
        Ok(Arc::new(Database::new(
            RocksDB {
                underlying: rocksdb,
            },
            metric_conf,
        )))
    })
}

/// Opens a database with options, and a number of column families with
/// individual options that are created if they do not exist.
pub fn open_cf_opts_secondary<P: AsRef<Path>>(
    primary_path: P,
    secondary_path: Option<P>,
    db_options: Option<rocksdb::Options>,
    metric_conf: MetricConf,
    opt_cfs: &[(&str, rocksdb::Options)],
) -> Result<Arc<Database>, TypedStoreError> {
    let primary_path = primary_path.as_ref();
    let secondary_path = secondary_path.as_ref().map(|p| p.as_ref());
    nondeterministic!({
        let mut options = db_options.unwrap_or_else(|| default_db_options().options);

        fdlimit::raise_fd_limit();
        options.set_max_open_files(-1);

        let mut opt_cfs: std::collections::HashMap<_, _> = opt_cfs.iter().cloned().collect();
        let cfs = rocksdb::DBWithThreadMode::<MultiThreaded>::list_cf(&options, primary_path)
            .ok()
            .unwrap_or_default();

        let default_db_options = default_db_options();
        for cf_key in cfs.iter() {
            if !opt_cfs.contains_key(&cf_key[..]) {
                opt_cfs.insert(cf_key, default_db_options.options.clone());
            }
        }

        let primary_path = primary_path.to_path_buf();
        let secondary_path = secondary_path.map(|q| q.to_path_buf()).unwrap_or_else(|| {
            let mut s = primary_path.clone();
            s.pop();
            s.push("SECONDARY");
            s.as_path().to_path_buf()
        });

        let rocksdb = {
            options.create_if_missing(true);
            options.create_missing_column_families(true);
            let db = rocksdb::DBWithThreadMode::<MultiThreaded>::open_cf_descriptors_as_secondary(
                &options,
                &primary_path,
                &secondary_path,
                opt_cfs
                    .iter()
                    .map(|(name, opts)| ColumnFamilyDescriptor::new(*name, (*opts).clone())),
            )
            .map_err(typed_store_err_from_rocks_err)?;
            db.try_catch_up_with_primary()
                .map_err(typed_store_err_from_rocks_err)?;
            db
        };
        Ok(Arc::new(Database::new(
            RocksDB {
                underlying: rocksdb,
            },
            metric_conf,
        )))
    })
}

pub fn list_tables(path: std::path::PathBuf) -> eyre::Result<Vec<String>> {
    const DB_DEFAULT_CF_NAME: &str = "default";

    let opts = rocksdb::Options::default();
    rocksdb::DBWithThreadMode::<rocksdb::MultiThreaded>::list_cf(&opts, path)
        .map_err(|e| e.into())
        .map(|q| {
            q.iter()
                .filter_map(|s| {
                    if s != DB_DEFAULT_CF_NAME {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
}

#[derive(Clone)]
pub struct DBMapTableConfigMap(std::collections::BTreeMap<String, DBOptions>);
impl DBMapTableConfigMap {
    pub fn new(map: std::collections::BTreeMap<String, DBOptions>) -> Self {
        Self(map)
    }

    pub fn to_map(&self) -> std::collections::BTreeMap<String, DBOptions> {
        self.0.clone()
    }
}

pub enum RocksDBAccessType {
    Primary,
    Secondary(Option<PathBuf>),
}

pub async fn safe_drop_db(path: PathBuf, timeout: Duration) -> Result<(), rocksdb::Error> {
    let mut backoff = backoff::ExponentialBackoff {
        max_elapsed_time: Some(timeout),
        ..Default::default()
    };
    loop {
        match rocksdb::DB::destroy(&rocksdb::Options::default(), path.clone()) {
            Ok(()) => return Ok(()),
            Err(err) => match backoff.next_backoff() {
                Some(duration) => tokio::time::sleep(duration).await,
                None => return Err(err),
            },
        }
    }
}

fn populate_missing_cfs(
    input_cfs: &[(&str, rocksdb::Options)],
    path: &Path,
) -> Result<Vec<(String, rocksdb::Options)>, rocksdb::Error> {
    let mut cfs = vec![];
    let input_cf_index: HashSet<_> = input_cfs.iter().map(|(name, _)| *name).collect();
    let existing_cfs =
        rocksdb::DBWithThreadMode::<MultiThreaded>::list_cf(&rocksdb::Options::default(), path)
            .ok()
            .unwrap_or_default();

    for cf_name in existing_cfs {
        if !input_cf_index.contains(&cf_name[..]) {
            cfs.push((cf_name, rocksdb::Options::default()));
        }
    }
    cfs.extend(
        input_cfs
            .iter()
            .map(|(name, opts)| (name.to_string(), (*opts).clone())),
    );
    Ok(cfs)
}
