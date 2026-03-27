// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod errors;
pub(crate) mod options;
pub(crate) mod rocks_util;
pub(crate) mod safe_iter;

use std::{
    collections::HashSet,
    ffi::CStr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use backoff::backoff::Backoff;
use iota_macros::nondeterministic;
use rocksdb::{
    AsColumnFamilyRef, ColumnFamilyDescriptor, Error, MultiThreaded, properties,
    properties::num_files_at_level,
};
use tracing::warn;
use typed_store_error::TypedStoreError;

pub use crate::{
    database::{DBBatch, DBMap, MetricConf},
    rocks::options::{
        DBMapTableConfigMap, DBOptions, ReadWriteOptions, default_db_options, list_tables,
        read_size_from_env,
    },
};
use crate::{
    database::{Database, Storage},
    metrics::DBMetrics,
    rocks::errors::typed_store_err_from_rocks_err,
};

// TODO: remove this after Rust rocksdb has the TOTAL_BLOB_FILES_SIZE property
// built-in. From https://github.com/facebook/rocksdb/blob/bd80433c73691031ba7baa65c16c63a83aef201a/include/rocksdb/db.h#L1169
const ROCKSDB_PROPERTY_TOTAL_BLOB_FILES_SIZE: &CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked("rocksdb.total-blob-file-size\0".as_bytes()) };

const METRICS_ERROR: i64 = -1;

const DB_CORRUPTED_KEY: &[u8] = b"db_corrupted";

#[cfg(test)]
mod tests;

// TODO: deprecate macros use
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
        let mut options = db_options.unwrap_or_else(|| default_db_options().options);
        options.create_if_missing(true);
        options.create_missing_column_families(true);
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
