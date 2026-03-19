// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod errors;
pub(crate) mod safe_iter;

use std::{
    collections::{BTreeMap, HashSet},
    env,
    ffi::CStr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use backoff::backoff::Backoff;
use bincode::Options;
use iota_macros::nondeterministic;
use rocksdb::{
    AsColumnFamilyRef, BlockBasedOptions, Cache, ColumnFamilyDescriptor, Error, MultiThreaded,
    properties, properties::num_files_at_level,
};
use tap::TapFallible;
use tracing::{info, instrument, warn};
use typed_store_error::TypedStoreError;

pub use crate::database::{DBBatch, DBMap, MetricConf, ReadWriteOptions};
use crate::{
    database::{Database, Storage},
    metrics::DBMetrics,
    rocks::errors::{typed_store_err_from_bincode_err, typed_store_err_from_rocks_err},
};

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

const METRICS_ERROR: i64 = -1;

const DB_CORRUPTED_KEY: &[u8] = b"db_corrupted";

#[cfg(test)]
mod tests;

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
/// use typed_store::metrics::DBMetrics;
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

#[derive(Debug)]
pub(crate) struct RocksDB {
    pub(crate) underlying: rocksdb::DBWithThreadMode<MultiThreaded>,
}

impl Drop for RocksDB {
    fn drop(&mut self) {
        self.underlying.cancel_all_background_work(/* wait */ true);
    }
}

pub(crate) fn rocks_cf<'a>(
    rocks_db: &'a RocksDB,
    cf_name: &str,
) -> Arc<rocksdb::BoundColumnFamily<'a>> {
    rocks_db
        .underlying
        .cf_handle(cf_name)
        .expect("Map-keying column family should have been checked at DB creation")
}

// Check if the database is corrupted, and if so, panic.
// If the corrupted key is not set, we set it to [1].
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

pub fn unmark_db_corruption(path: &Path) -> Result<(), Error> {
    rocksdb::DB::open_default(path)?.put(DB_CORRUPTED_KEY, [0])
}

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

// TODO: refactor this into a builder pattern, where rocksdb::Options are
// generated after a call to build().
#[derive(Default, Clone)]
pub struct DBOptions {
    pub options: rocksdb::Options,
    pub rw_options: ReadWriteOptions,
}

impl DBOptions {
    // Optimize lookup perf for tables where no scans are performed.
    // If non-trivial number of values can be > 512B in size, it is beneficial to
    // also specify optimize_for_large_values_no_scan().
    pub fn optimize_for_point_lookup(mut self, block_cache_size_mb: usize) -> DBOptions {
        // NOTE: this overwrites the block options.
        self.options
            .optimize_for_point_lookup(block_cache_size_mb as u64);
        self
    }

    // Optimize write and lookup perf for tables which are rarely scanned, and have
    // large values. https://rocksdb.org/blog/2021/05/26/integrated-blob-db.html
    pub fn optimize_for_large_values_no_scan(mut self, min_blob_size: u64) -> DBOptions {
        if env::var(ENV_VAR_DISABLE_BLOB_STORAGE).is_ok() {
            info!("Large value blob storage optimization is disabled via env var.");
            return self;
        }

        // Blob settings.
        self.options.set_enable_blob_files(true);
        self.options
            .set_blob_compression_type(rocksdb::DBCompressionType::Lz4);
        self.options.set_enable_blob_gc(true);
        // Since each blob can have non-trivial size overhead, and compression does not
        // work across blobs, set a min blob size in bytes to so small
        // transactions and effects are kept in sst files.
        self.options.set_min_blob_size(min_blob_size);

        // Increase write buffer size to 256MiB.
        let write_buffer_size = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_SIZE_MB)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_SIZE_MB)
            * 1024
            * 1024;
        self.options.set_write_buffer_size(write_buffer_size);
        // Since large blobs are not in sst files, reduce the target file size and base
        // level target size.
        let target_file_size_base = 64 << 20;
        self.options
            .set_target_file_size_base(target_file_size_base);
        // Level 1 default to 64MiB * 4 ~ 256MiB.
        let max_level_zero_file_num = read_size_from_env(ENV_VAR_L0_NUM_FILES_COMPACTION_TRIGGER)
            .unwrap_or(DEFAULT_L0_NUM_FILES_COMPACTION_TRIGGER);
        self.options
            .set_max_bytes_for_level_base(target_file_size_base * max_level_zero_file_num as u64);

        self
    }

    // Optimize tables with a mix of lookup and scan workloads.
    pub fn optimize_for_read(mut self, block_cache_size_mb: usize) -> DBOptions {
        self.options
            .set_block_based_table_factory(&get_block_options(block_cache_size_mb, 16 << 10));
        self
    }

    // Optimize DB receiving significant insertions.
    pub fn optimize_db_for_write_throughput(mut self, db_max_write_buffer_gb: u64) -> DBOptions {
        self.options
            .set_db_write_buffer_size(db_max_write_buffer_gb as usize * 1024 * 1024 * 1024);
        self.options
            .set_max_total_wal_size(db_max_write_buffer_gb * 1024 * 1024 * 1024);
        self
    }

    // Optimize tables receiving significant insertions.
    pub fn optimize_for_write_throughput(mut self) -> DBOptions {
        // Increase write buffer size to 256MiB.
        let write_buffer_size = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_SIZE_MB)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_SIZE_MB)
            * 1024
            * 1024;
        self.options.set_write_buffer_size(write_buffer_size);
        // Increase write buffers to keep to 6 before slowing down writes.
        let max_write_buffer_number = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_NUMBER)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_NUMBER);
        self.options
            .set_max_write_buffer_number(max_write_buffer_number.try_into().unwrap());
        // Keep 1 write buffer so recent writes can be read from memory.
        self.options
            .set_max_write_buffer_size_to_maintain((write_buffer_size).try_into().unwrap());

        // Increase compaction trigger for level 0 to 6.
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

        // Increase sst file size to 128MiB.
        self.options.set_target_file_size_base(
            read_size_from_env(ENV_VAR_TARGET_FILE_SIZE_BASE_MB)
                .unwrap_or(DEFAULT_TARGET_FILE_SIZE_BASE_MB) as u64
                * 1024
                * 1024,
        );

        // Increase level 1 target size to 256MiB * 6 ~ 1.5GiB.
        self.options
            .set_max_bytes_for_level_base((write_buffer_size * max_level_zero_file_num) as u64);

        self
    }

    // Optimize tables receiving significant insertions, without any deletions.
    // TODO: merge this function with optimize_for_write_throughput(), and use a
    // flag to indicate if deletion is received.
    pub fn optimize_for_write_throughput_no_deletion(mut self) -> DBOptions {
        // Increase write buffer size to 256MiB.
        let write_buffer_size = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_SIZE_MB)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_SIZE_MB)
            * 1024
            * 1024;
        self.options.set_write_buffer_size(write_buffer_size);
        // Increase write buffers to keep to 6 before slowing down writes.
        let max_write_buffer_number = read_size_from_env(ENV_VAR_MAX_WRITE_BUFFER_NUMBER)
            .unwrap_or(DEFAULT_MAX_WRITE_BUFFER_NUMBER);
        self.options
            .set_max_write_buffer_number(max_write_buffer_number.try_into().unwrap());
        // Keep 1 write buffer so recent writes can be read from memory.
        self.options
            .set_max_write_buffer_size_to_maintain((write_buffer_size).try_into().unwrap());

        // Switch to universal compactions.
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

        // Increase sst file size to 128MiB.
        self.options.set_target_file_size_base(
            read_size_from_env(ENV_VAR_TARGET_FILE_SIZE_BASE_MB)
                .unwrap_or(DEFAULT_TARGET_FILE_SIZE_BASE_MB) as u64
                * 1024
                * 1024,
        );

        // This should be a no-op for universal compaction but increasing it to be safe.
        self.options
            .set_max_bytes_for_level_base((write_buffer_size * max_level_zero_file_num) as u64);

        self
    }

    // Overrides the block options with different block cache size and block size.
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

    // Disables write stalling and stopping based on pending compaction bytes.
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

    // One common issue when running tests on Mac is that the default ulimit is too
    // low, leading to I/O errors such as "Too many open files". Raising fdlimit
    // to bypass it.
    if let Some(limit) = fdlimit::raise_fd_limit() {
        // on windows raise_fd_limit return None
        opt.set_max_open_files((limit / 8) as i32);
    }

    // The table cache is locked for updates and this determines the number
    // of shards, ie 2^10. Increase in case of lock contentions.
    opt.set_table_cache_num_shard_bits(10);

    // LSM compression settings
    opt.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opt.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
    opt.set_bottommost_zstd_max_train_bytes(1024 * 1024, true);

    // IOTA uses multiple RocksDB in a node, so total sizes of write buffers and WAL
    // can be higher than the limits below.
    //
    // RocksDB also exposes the option to configure total write buffer size across
    // multiple instances via `write_buffer_manager`. But the write buffer flush
    // policy (flushing the buffer receiving the next write) may not work well.
    // So sticking to per-db write buffer size limit for now.
    //
    // The environment variables are only meant to be emergency overrides. They may
    // go away in future. It is preferable to update the default value, or
    // override the option in code.
    opt.set_db_write_buffer_size(
        read_size_from_env(ENV_VAR_DB_WRITE_BUFFER_SIZE).unwrap_or(DEFAULT_DB_WRITE_BUFFER_SIZE)
            * 1024
            * 1024,
    );
    opt.set_max_total_wal_size(
        read_size_from_env(ENV_VAR_DB_WAL_SIZE).unwrap_or(DEFAULT_DB_WAL_SIZE) as u64 * 1024 * 1024,
    );

    // Num threads for compactions and memtable flushes.
    opt.increase_parallelism(read_size_from_env(ENV_VAR_DB_PARALLELISM).unwrap_or(8) as i32);

    opt.set_enable_pipelined_write(true);

    // Increase block size to 16KiB.
    // https://github.com/EighteenZi/rocksdb_wiki/blob/master/Memory-usage-in-RocksDB.md#indexes-and-filter-blocks
    opt.set_block_based_table_factory(&get_block_options(128, 16 << 10));

    // Set memtable bloomfilter.
    opt.set_memtable_prefix_bloom_ratio(0.02);

    DBOptions {
        options: opt,
        rw_options: ReadWriteOptions::default(),
    }
}

fn get_block_options(block_cache_size_mb: usize, block_size_bytes: usize) -> BlockBasedOptions {
    // Set options mostly similar to those used in optimize_for_point_lookup(),
    // except non-default binary and hash index, to hopefully reduce lookup
    // latencies without causing any regression for scanning, with slightly more
    // memory usages. https://github.com/facebook/rocksdb/blob/11cb6af6e5009c51794641905ca40ce5beec7fee/options/options.cc#L611-L621
    let mut block_options = BlockBasedOptions::default();
    // Overrides block size.
    block_options.set_block_size(block_size_bytes);
    // Configure a block cache.
    block_options.set_block_cache(&Cache::new_lru_cache(block_cache_size_mb << 20));
    // Set a bloomfilter with 1% false positive rate.
    block_options.set_bloom_filter(10.0, false);
    // From https://github.com/EighteenZi/rocksdb_wiki/blob/master/Block-Cache.md#caching-index-and-filter-blocks
    block_options.set_pin_l0_filter_and_index_blocks_in_cache(true);
    block_options
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
    // Customize database options
    let mut options = db_options.unwrap_or_else(|| default_db_options().options);
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    options
}

/// Opens a database with options, and a number of column families with
/// individual options that are created if they do not exist.
#[tracing::instrument(level="debug", skip_all, fields(path = ?path.as_ref()), err)]
pub fn open_cf_opts<P: AsRef<Path>>(
    path: P,
    db_options: Option<rocksdb::Options>,
    metric_conf: MetricConf,
    opt_cfs: &[(&str, rocksdb::Options)],
) -> Result<Arc<Database>, TypedStoreError> {
    let path = path.as_ref();
    // In the simulator, we intercept the wall clock in the test thread only. This
    // causes problems because rocksdb uses the simulated clock when creating
    // its background threads, but then those threads see the real wall clock
    // (because they are not the test thread), which causes rocksdb to panic.
    // The `nondeterministic` macro evaluates expressions in new threads, which
    // resolves the issue.
    //
    // This is a no-op in non-simulator builds.

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
            Storage::Rocks(RocksDB {
                underlying: rocksdb,
            }),
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
    // See comment above for explanation of why nondeterministic is necessary here.
    nondeterministic!({
        // Customize database options
        let mut options = db_options.unwrap_or_else(|| default_db_options().options);

        fdlimit::raise_fd_limit();
        // This is a requirement by RocksDB when opening as secondary
        options.set_max_open_files(-1);

        let mut opt_cfs: std::collections::HashMap<_, _> = opt_cfs.iter().cloned().collect();
        let cfs = rocksdb::DBWithThreadMode::<MultiThreaded>::list_cf(&options, primary_path)
            .ok()
            .unwrap_or_default();

        let default_db_options = default_db_options();
        // Add CFs not explicitly listed
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
            Storage::Rocks(RocksDB {
                underlying: rocksdb,
            }),
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
                    // The `default` table is not used
                    if s != DB_DEFAULT_CF_NAME {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect()
        })
}

/// Serializes keys using big-endian byte order with fixed-int encoding.
/// RocksDB stores keys in big-endian and uses a byte-wise seek operator on
/// iterators, see `https://github.com/facebook/rocksdb/wiki/Iterator#introduction`
#[inline]
pub fn be_fix_int_ser<S>(t: &S) -> Result<Vec<u8>, TypedStoreError>
where
    S: ?Sized + serde::Serialize,
{
    bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding()
        .serialize(t)
        .map_err(typed_store_err_from_bincode_err)
}

#[derive(Clone)]
pub struct DBMapTableConfigMap(BTreeMap<String, DBOptions>);
impl DBMapTableConfigMap {
    pub fn new(map: BTreeMap<String, DBOptions>) -> Self {
        Self(map)
    }

    pub fn to_map(&self) -> BTreeMap<String, DBOptions> {
        self.0.clone()
    }
}

pub enum RocksDBAccessType {
    Primary,
    Secondary(Option<PathBuf>),
}

// Drops a database if there is no other handle to it, with retries and timeout.
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

/// RocksDB-specific methods on `DBMap`. These are kept separate from the
/// generic impl in `database.rs` because they directly access RocksDB
/// internals and have no meaning for other storage backends.
impl<K, V> DBMap<K, V> {
    fn get_rocksdb_int_property(
        rocksdb: &RocksDB,
        cf: &impl AsColumnFamilyRef,
        property_name: &CStr,
    ) -> Result<i64, TypedStoreError> {
        match rocksdb.underlying.property_int_value_cf(cf, property_name) {
            Ok(Some(value)) => Ok(value.min(i64::MAX as u64).try_into().unwrap_or_default()),
            Ok(None) => Ok(0),
            Err(e) => Err(TypedStoreError::RocksDB(e.into_string())),
        }
    }

    pub(crate) fn report_rocksdb_metrics(
        database: &Arc<Database>,
        cf_name: &str,
        db_metrics: &Arc<DBMetrics>,
    ) {
        let Storage::Rocks(rocksdb) = &database.storage else {
            return;
        };

        let Some(cf) = rocksdb.underlying.cf_handle(cf_name) else {
            warn!(
                "unable to report metrics for cf {cf_name:?} in db {:?}",
                database.db_name()
            );
            return;
        };

        db_metrics
            .cf_metrics
            .rocksdb_total_sst_files_size
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::TOTAL_SST_FILES_SIZE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_total_blob_files_size
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(
                    rocksdb,
                    &cf,
                    ROCKSDB_PROPERTY_TOTAL_BLOB_FILES_SIZE,
                )
                .unwrap_or(METRICS_ERROR),
            );
        // 7 is the default number of levels in RocksDB. If we ever change the number of
        // levels using `set_num_levels`, we need to update here as well. Note
        // that there isn't an API to query the DB to get the number of levels (yet).
        let total_num_files: i64 = (0..=6)
            .map(|level| {
                Self::get_rocksdb_int_property(rocksdb, &cf, &num_files_at_level(level))
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
                Self::get_rocksdb_int_property(rocksdb, &cf, &num_files_at_level(0))
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_current_size_active_mem_tables
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::CUR_SIZE_ACTIVE_MEM_TABLE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_size_all_mem_tables
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::SIZE_ALL_MEM_TABLES)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_snapshots
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::NUM_SNAPSHOTS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_oldest_snapshot_time
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::OLDEST_SNAPSHOT_TIME)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_actual_delayed_write_rate
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::ACTUAL_DELAYED_WRITE_RATE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_is_write_stopped
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::IS_WRITE_STOPPED)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_block_cache_capacity
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::BLOCK_CACHE_CAPACITY)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_block_cache_usage
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::BLOCK_CACHE_USAGE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_block_cache_pinned_usage
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::BLOCK_CACHE_PINNED_USAGE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimate_table_readers_mem
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(
                    rocksdb,
                    &cf,
                    properties::ESTIMATE_TABLE_READERS_MEM,
                )
                .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimated_num_keys
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::ESTIMATE_NUM_KEYS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_immutable_mem_tables
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::NUM_IMMUTABLE_MEM_TABLE)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_mem_table_flush_pending
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::MEM_TABLE_FLUSH_PENDING)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_compaction_pending
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::COMPACTION_PENDING)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimate_pending_compaction_bytes
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(
                    rocksdb,
                    &cf,
                    properties::ESTIMATE_PENDING_COMPACTION_BYTES,
                )
                .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_running_compactions
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::NUM_RUNNING_COMPACTIONS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_num_running_flushes
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::NUM_RUNNING_FLUSHES)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_estimate_oldest_key_time
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::ESTIMATE_OLDEST_KEY_TIME)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_background_errors
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::BACKGROUND_ERRORS)
                    .unwrap_or(METRICS_ERROR),
            );
        db_metrics
            .cf_metrics
            .rocksdb_base_level
            .with_label_values(&[cf_name])
            .set(
                Self::get_rocksdb_int_property(rocksdb, &cf, properties::BASE_LEVEL)
                    .unwrap_or(METRICS_ERROR),
            );
    }
}
