// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    borrow::Borrow,
    fmt,
    marker::PhantomData,
    ops::{Bound, RangeBounds},
    path::Path,
    sync::Arc,
    time::Duration,
};

use bincode::Options as _;
use prometheus::{Histogram, HistogramTimer};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::oneshot;
use tracing::{debug, error, instrument, warn};
use typed_store_error::TypedStoreError;

use crate::{
    engine::{StorageEngine, StorageMetrics},
    metrics::SamplingInterval,
    rocks::{
        RocksDB,
        metrics::RocksDBPerfContext,
        safe_iter::{SafeIter, SafeRevIter},
    },
    traits::{Map, TableSummary},
};

// ---------------------------------------------------------------------------
// Serialisation helpers (backend-agnostic)
// ---------------------------------------------------------------------------

/// Serialise `t` using big-endian fixed-int bincode encoding.
///
/// RocksDB stores keys in lexicographic order; big-endian fixed-width encoding
/// preserves the natural order of integer keys.
#[inline]
pub(crate) fn be_fix_int_ser<S>(t: &S) -> Result<Vec<u8>, TypedStoreError>
where
    S: ?Sized + serde::Serialize,
{
    bincode::DefaultOptions::new()
        .with_big_endian()
        .with_fixint_encoding()
        .serialize(t)
        .map_err(typed_store_err_from_bincode_err)
}

pub(crate) fn typed_store_err_from_bincode_err(err: bincode::Error) -> TypedStoreError {
    TypedStoreError::Serialization(format!("{err}"))
}

pub(crate) fn typed_store_err_from_bcs_err(err: bcs::Error) -> TypedStoreError {
    TypedStoreError::Serialization(format!("{err}"))
}

// ---------------------------------------------------------------------------
// Byte-level helpers for big-endian iteration bounds
// ---------------------------------------------------------------------------

/// Increment a big-endian byte vector by one, saturating at all-0xFF.
pub(crate) fn big_endian_saturating_add_one(v: &mut [u8]) {
    if is_max(v) {
        return;
    }
    for i in (0..v.len()).rev() {
        if v[i] == u8::MAX {
            v[i] = 0;
        } else {
            v[i] += 1;
            break;
        }
    }
}

/// Returns `true` if every byte in `v` is `0xFF`.
pub(crate) fn is_max(v: &[u8]) -> bool {
    v.iter().all(|&x| x == u8::MAX)
}

// ---------------------------------------------------------------------------
// MetricConf — per-database metric configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct MetricConf {
    pub db_name: String,
    pub read_sample_interval: SamplingInterval,
    pub write_sample_interval: SamplingInterval,
    pub iter_sample_interval: SamplingInterval,
}

impl MetricConf {
    pub fn new(db_name: &str) -> Self {
        if db_name.is_empty() {
            error!("A meaningful db name should be used for metrics reporting.")
        }
        Self {
            db_name: db_name.to_string(),
            read_sample_interval: SamplingInterval::default(),
            write_sample_interval: SamplingInterval::default(),
            iter_sample_interval: SamplingInterval::default(),
        }
    }

    pub fn with_sampling(self, read_interval: SamplingInterval) -> Self {
        Self {
            db_name: self.db_name,
            read_sample_interval: read_interval,
            write_sample_interval: SamplingInterval::default(),
            iter_sample_interval: SamplingInterval::default(),
        }
    }
}

const CF_METRICS_REPORT_PERIOD_SECS: u64 = 30;

// ---------------------------------------------------------------------------
// Database<S> — owns the storage engine and per-DB metric configuration
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Database<S: StorageEngine = RocksDB> {
    /// The underlying storage engine.
    /// `pub(crate)` so that `DBMap` and `DBBatch` can call trait methods.
    pub(crate) storage: S,
    metric_conf: MetricConf,
}

impl<S: StorageEngine> Drop for Database<S> {
    fn drop(&mut self) {
        S::get_metrics().on_db_closed(&self.metric_conf.db_name);
    }
}

impl<S: StorageEngine> Database<S> {
    pub(crate) fn new(storage: S, metric_conf: MetricConf) -> Self {
        S::get_metrics().on_db_opened(&metric_conf.db_name);
        Self {
            storage,
            metric_conf,
        }
    }

    // -- Public API --

    /// Returns `Some(())` if the column family exists, `None` otherwise.
    pub fn cf_handle(&self, name: &str) -> Option<()> {
        self.storage.has_cf(name).then_some(())
    }

    pub fn drop_cf(&self, name: &str) -> Result<(), TypedStoreError> {
        self.storage.drop_cf(name)
    }

    /// Flush all memtables to disk.
    pub fn flush(&self) -> Result<(), TypedStoreError> {
        self.storage.flush()
    }

    /// Create a checkpoint at the given path.
    pub fn checkpoint(&self, path: &Path) -> Result<(), TypedStoreError> {
        self.storage.checkpoint(path)
    }

    // -- Crate-internal API --

    pub(crate) fn create_cf(&self, name: &str, opts: &S::CfOptions) -> Result<(), TypedStoreError> {
        self.storage.create_cf(name, opts)
    }

    pub(crate) fn compact_range_cf<K: AsRef<[u8]>>(
        &self,
        cf_name: &str,
        start: Option<K>,
        end: Option<K>,
    ) {
        self.storage.compact_range(
            cf_name,
            start.as_ref().map(|k| k.as_ref()),
            end.as_ref().map(|k| k.as_ref()),
        );
    }

    pub fn get_sampling_interval(&self) -> SamplingInterval {
        self.metric_conf.read_sample_interval.new_from_self()
    }

    pub fn multiget_sampling_interval(&self) -> SamplingInterval {
        self.metric_conf.read_sample_interval.new_from_self()
    }

    pub fn write_sampling_interval(&self) -> SamplingInterval {
        self.metric_conf.write_sample_interval.new_from_self()
    }

    pub fn iter_sampling_interval(&self) -> SamplingInterval {
        self.metric_conf.iter_sample_interval.new_from_self()
    }

    pub(crate) fn db_name(&self) -> String {
        let name = &self.metric_conf.db_name;
        if name.is_empty() {
            "default".to_string()
        } else {
            name.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// DBMap<K, V, S> — typed map over a column family
// ---------------------------------------------------------------------------

/// An interface to a storage database, keyed by a column family.
pub struct DBMap<K, V, S: StorageEngine = RocksDB> {
    pub db: Arc<Database<S>>,
    _phantom: PhantomData<fn(K) -> V>,
    /// Column-family name under which this map is stored.
    cf: String,
    /// Read options applied to every read/iterator on this map.
    pub read_opts: S::ReadOptions,
    /// Write options applied to every write on this map.
    pub(crate) write_opts: S::WriteOptions,
    db_metrics: Arc<S::Metrics>,
    get_sample_interval: SamplingInterval,
    multiget_sample_interval: SamplingInterval,
    write_sample_interval: SamplingInterval,
    iter_sample_interval: SamplingInterval,
    _metrics_task_cancel_handle: Arc<oneshot::Sender<()>>,
}

// Manual impls to avoid spurious `S: Clone` / `S: Debug` bounds from `derive`.
impl<K, V, S: StorageEngine> Clone for DBMap<K, V, S> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            _phantom: PhantomData,
            cf: self.cf.clone(),
            read_opts: self.read_opts.clone(),
            write_opts: self.write_opts.clone(),
            db_metrics: self.db_metrics.clone(),
            get_sample_interval: self.get_sample_interval.clone(),
            multiget_sample_interval: self.multiget_sample_interval.clone(),
            write_sample_interval: self.write_sample_interval.clone(),
            iter_sample_interval: self.iter_sample_interval.clone(),
            _metrics_task_cancel_handle: self._metrics_task_cancel_handle.clone(),
        }
    }
}

impl<K, V, S: StorageEngine> fmt::Debug for DBMap<K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DBMap")
            .field("db", &self.db)
            .field("cf", &self.cf)
            .finish()
    }
}

unsafe impl<K: Send, V: Send, S: StorageEngine> Send for DBMap<K, V, S> {}

impl<K, V, S: StorageEngine> DBMap<K, V, S> {
    pub(crate) fn new(
        db: Arc<Database<S>>,
        read_opts: S::ReadOptions,
        write_opts: S::WriteOptions,
        opt_cf: &str,
        is_deprecated: bool,
    ) -> Self {
        let db_cloned = db.clone();
        let db_metrics = S::get_metrics();
        let db_metrics_cloned = db_metrics.clone();
        let cf = opt_cf.to_string();

        let (sender, mut recv) = tokio::sync::oneshot::channel();
        if !is_deprecated {
            tokio::task::spawn(async move {
                let mut interval =
                    tokio::time::interval(Duration::from_secs(CF_METRICS_REPORT_PERIOD_SECS));
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            let db = db_cloned.clone();
                            let cf = cf.clone();
                            let db_metrics = db_metrics.clone();
                            if let Err(e) = tokio::task::spawn_blocking(move || {
                                let db_name = db.db_name();
                                db.storage.report_cf_metrics(&cf, &db_name, &db_metrics);
                            }).await {
                                error!("Failed to log metrics with error: {}", e);
                            }
                        }
                        _ = &mut recv => break,
                    }
                }
                debug!("Returning the cf metric logging task for DBMap: {}", &cf);
            });
        }
        DBMap {
            db: db.clone(),
            read_opts,
            write_opts,
            _phantom: PhantomData,
            cf: opt_cf.to_string(),
            db_metrics: db_metrics_cloned,
            _metrics_task_cancel_handle: Arc::new(sender),
            get_sample_interval: db.get_sampling_interval(),
            multiget_sample_interval: db.multiget_sampling_interval(),
            write_sample_interval: db.write_sampling_interval(),
            iter_sample_interval: db.iter_sampling_interval(),
        }
    }

    pub fn cf_name(&self) -> &str {
        &self.cf
    }

    pub fn batch(&self) -> DBBatch<S> {
        let batch = self.db.storage.new_batch();
        DBBatch::new(
            &self.db,
            batch,
            self.write_opts.clone(),
            &self.db_metrics,
            &self.write_sample_interval,
        )
    }

    pub fn compact_range<J: Serialize>(&self, start: &J, end: &J) -> Result<(), TypedStoreError> {
        let from_buf = be_fix_int_ser(start)?;
        let to_buf = be_fix_int_ser(end)?;
        self.db
            .compact_range_cf(&self.cf, Some(from_buf), Some(to_buf));
        Ok(())
    }

    pub fn compact_range_raw(
        &self,
        cf_name: &str,
        start: Vec<u8>,
        end: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        self.db.compact_range_cf(cf_name, Some(start), Some(end));
        Ok(())
    }

    pub fn checkpoint_db(&self, path: &Path) -> Result<(), TypedStoreError> {
        self.db.checkpoint(path)
    }

    /// Build read options with lower and upper iteration bounds.
    /// Lower bound is inclusive, upper bound is exclusive.
    fn create_read_options_with_bounds(
        &self,
        lower_bound: Option<K>,
        upper_bound: Option<K>,
    ) -> S::ReadOptions
    where
        K: Serialize,
    {
        let mut readopts = self.read_opts.clone();
        if let Some(lower_bound) = lower_bound {
            let key_buf = be_fix_int_ser(&lower_bound).unwrap();
            S::set_iter_lower_bound(&mut readopts, key_buf);
        }
        if let Some(upper_bound) = upper_bound {
            let key_buf = be_fix_int_ser(&upper_bound).unwrap();
            S::set_iter_upper_bound(&mut readopts, key_buf);
        }
        readopts
    }

    /// Build read options with lower and upper bounds corresponding to `range`.
    fn create_read_options_with_range(&self, range: impl RangeBounds<K>) -> S::ReadOptions
    where
        K: Serialize,
    {
        let mut readopts = self.read_opts.clone();

        match range.start_bound() {
            Bound::Included(lower_bound) => {
                let key_buf = be_fix_int_ser(lower_bound).expect("Serialization must not fail");
                S::set_iter_lower_bound(&mut readopts, key_buf);
            }
            Bound::Excluded(lower_bound) => {
                let mut key_buf =
                    be_fix_int_ser(lower_bound).expect("Serialization must not fail");
                big_endian_saturating_add_one(&mut key_buf);
                S::set_iter_lower_bound(&mut readopts, key_buf);
            }
            Bound::Unbounded => (),
        }

        match range.end_bound() {
            Bound::Included(upper_bound) => {
                let mut key_buf =
                    be_fix_int_ser(upper_bound).expect("Serialization must not fail");
                if !is_max(&key_buf) {
                    big_endian_saturating_add_one(&mut key_buf);
                    S::set_iter_upper_bound(&mut readopts, key_buf);
                }
            }
            Bound::Excluded(upper_bound) => {
                let key_buf = be_fix_int_ser(upper_bound).expect("Serialization must not fail");
                S::set_iter_upper_bound(&mut readopts, key_buf);
            }
            Bound::Unbounded => (),
        }

        readopts
    }
}

// ---------------------------------------------------------------------------
// RocksDB-specific helpers on DBMap<K, V, RocksDB>
// ---------------------------------------------------------------------------

impl<K, V> DBMap<K, V, RocksDB> {
    /// Returns a vector of raw pinned values corresponding to the keys provided.
    fn multi_get_pinned<J>(
        &self,
        keys: impl IntoIterator<Item = J>,
    ) -> Result<Vec<Option<<RocksDB as StorageEngine>::GetValue<'_>>>, TypedStoreError>
    where
        J: Borrow<K>,
        K: Serialize,
    {
        let _timer = self.db_metrics.start_multiget_timer(&self.cf);
        let perf_ctx = if self.multiget_sample_interval.sample() {
            Some(RocksDBPerfContext::default())
        } else {
            None
        };
        let keys_bytes: Result<Vec<_>, TypedStoreError> = keys
            .into_iter()
            .map(|k| be_fix_int_ser(k.borrow()))
            .collect();
        let readopts = self.read_opts.clone();
        let results: Result<Vec<_>, TypedStoreError> = self
            .db
            .storage
            .multi_get(&self.cf, keys_bytes?, &readopts)
            .into_iter()
            .collect();
        let entries = results?;
        let entry_size = entries
            .iter()
            .flatten()
            .map(|entry| entry.len())
            .sum::<usize>();
        self.db_metrics.observe_multiget_bytes(&self.cf, entry_size as f64);
        if perf_ctx.is_some() {
            self.db_metrics.report_read_perf_ctx(&self.cf);
        }
        Ok(entries)
    }

    pub fn table_summary(&self) -> eyre::Result<TableSummary>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        use crate::traits::Map;
        let mut num_keys = 0;
        let mut key_bytes_total = 0;
        let mut value_bytes_total = 0;
        let mut key_hist = hdrhistogram::Histogram::<u64>::new_with_max(100000, 2).unwrap();
        let mut value_hist = hdrhistogram::Histogram::<u64>::new_with_max(100000, 2).unwrap();
        for item in self.safe_iter() {
            let (key, value) = item?;
            num_keys += 1;
            let key_len = be_fix_int_ser(key.borrow())?.len();
            let value_len = bcs::to_bytes(value.borrow())?.len();
            key_bytes_total += key_len;
            value_bytes_total += value_len;
            key_hist.record(key_len as u64)?;
            value_hist.record(value_len as u64)?;
        }
        Ok(TableSummary {
            num_keys,
            key_bytes_total,
            value_bytes_total,
            key_hist,
            value_hist,
        })
    }

    /// Creates metrics and context for tracking iterator usage and performance.
    fn create_iter_context(
        &self,
    ) -> (
        Option<HistogramTimer>,
        Option<Histogram>,
        Option<Histogram>,
        Option<RocksDBPerfContext>,
    ) {
        let timer = self.db_metrics.start_iter_timer(&self.cf);
        let bytes_scanned = self.db_metrics.iter_bytes_histogram(&self.cf);
        let keys_scanned = self.db_metrics.iter_keys_histogram(&self.cf);
        let perf_ctx = if self.iter_sample_interval.sample() {
            Some(RocksDBPerfContext::default())
        } else {
            None
        };
        (
            Some(timer),
            Some(bytes_scanned),
            Some(keys_scanned),
            perf_ctx,
        )
    }

    /// Creates a safe reversed iterator with optional bounds.
    /// Upper bound is included.
    pub fn reversed_safe_iter_with_bounds(
        &self,
        lower_bound: Option<K>,
        upper_bound: Option<K>,
    ) -> Result<SafeRevIter<'_, K, V>, TypedStoreError>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
        let upper_bound_key = upper_bound.as_ref().map(|k| be_fix_int_ser(k));
        let readopts = self.create_read_options_with_range((
            lower_bound
                .as_ref()
                .map(Bound::Included)
                .unwrap_or(Bound::Unbounded),
            upper_bound
                .as_ref()
                .map(Bound::Included)
                .unwrap_or(Bound::Unbounded),
        ));

        let db_iter = self.db.storage.raw_iterator(&self.cf, readopts);
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        let iter = SafeIter::new(
            self.cf.clone(),
            db_iter,
            _timer,
            _perf_ctx,
            bytes_scanned,
            keys_scanned,
            Some(self.db_metrics.clone()),
        );
        Ok(SafeRevIter::new(iter, upper_bound_key.transpose()?))
    }
}

// ---------------------------------------------------------------------------
// DBBatch<S> — accumulated write batch
// ---------------------------------------------------------------------------

/// Provides a mutable struct to form a collection of database write
/// operations, and execute them.
///
/// Batching write and delete operations is faster than performing them one by
/// one and ensures their atomicity, i.e. they are all written or none is.
/// This is also true of operations across column families in the same database.
///
/// Serialisation / deserialisation, and naming of column families is performed
/// by passing a DBMap<K,V> with each operation.
///
/// ```
/// use core::fmt::Error;
/// use std::sync::Arc;
///
/// use prometheus::Registry;
/// use tempfile::tempdir;
/// use typed_store::{Map, rocks::*};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Error> {
///     let rocks = open_cf(
///         tempfile::tempdir().unwrap(),
///         None,
///         MetricConf::default(),
///         &["First_CF", "Second_CF"],
///     )
///     .unwrap();
///
///     let db_cf_1 = DBMap::reopen(
///         &rocks,
///         Some("First_CF"),
///         &ReadWriteOptions::default(),
///         false,
///     )
///     .expect("Failed to open storage");
///     let keys_vals_1 = (1..100).map(|i| (i, i.to_string()));
///
///     let db_cf_2 = DBMap::reopen(
///         &rocks,
///         Some("Second_CF"),
///         &ReadWriteOptions::default(),
///         false,
///     )
///     .expect("Failed to open storage");
///     let keys_vals_2 = (1000..1100).map(|i| (i, i.to_string()));
///
///     let mut batch = db_cf_1.batch();
///     batch
///         .insert_batch(&db_cf_1, keys_vals_1.clone())
///         .expect("Failed to batch insert")
///         .insert_batch(&db_cf_2, keys_vals_2.clone())
///         .expect("Failed to batch insert");
///
///     let _ = batch.write().expect("Failed to execute batch");
///     Ok(())
/// }
/// ```
pub struct DBBatch<S: StorageEngine = RocksDB> {
    database: Arc<Database<S>>,
    batch: S::Batch,
    write_opts: S::WriteOptions,
    db_metrics: Arc<S::Metrics>,
    write_sample_interval: SamplingInterval,
}

impl<S: StorageEngine> DBBatch<S> {
    /// Create a new batch associated with a DB reference.
    pub fn new(
        dbref: &Arc<Database<S>>,
        batch: S::Batch,
        write_opts: S::WriteOptions,
        db_metrics: &Arc<S::Metrics>,
        write_sample_interval: &SamplingInterval,
    ) -> Self {
        DBBatch {
            database: dbref.clone(),
            batch,
            write_opts,
            db_metrics: db_metrics.clone(),
            write_sample_interval: write_sample_interval.clone(),
        }
    }

    /// Consume the batch and write its operations to the database.
    #[instrument(level = "trace", skip_all, err)]
    pub fn write(self) -> Result<(), TypedStoreError> {
        let db_name = self.database.db_name();
        let timer = self.db_metrics.start_batch_commit_timer(&db_name);
        let batch_size = self.size_in_bytes();
        let sample = self.write_sample_interval.sample();
        self.db_metrics.begin_write_perf_ctx(sample);
        self.database
            .storage
            .write_batch(self.batch, &self.write_opts)?;
        self.db_metrics.observe_batch_commit_bytes(&db_name, batch_size as f64);
        self.db_metrics.end_write_perf_ctx(sample, &db_name);
        let elapsed = timer.stop_and_record();
        if elapsed > 1.0 {
            warn!(?elapsed, ?db_name, "very slow batch write");
            self.db_metrics.inc_very_slow_batch_writes(&db_name, (elapsed * 1000.0) as u64);
        }
        Ok(())
    }

    pub fn size_in_bytes(&self) -> usize {
        self.database.storage.batch_size_in_bytes(&self.batch)
    }

    pub fn delete_batch<J: Borrow<K>, K: Serialize, V>(
        &mut self,
        db: &DBMap<K, V, S>,
        purged_vals: impl IntoIterator<Item = J>,
    ) -> Result<(), TypedStoreError> {
        if !Arc::ptr_eq(&db.db, &self.database) {
            return Err(TypedStoreError::CrossDBBatch);
        }

        purged_vals
            .into_iter()
            .try_for_each::<_, Result<_, TypedStoreError>>(|k| {
                let k_buf = be_fix_int_ser(k.borrow())?;
                self.database
                    .storage
                    .batch_delete(&mut self.batch, &db.cf, k_buf)
            })
    }

    /// Deletes a range of keys between `from` (inclusive) and `to`
    /// (non-inclusive) by writing a range delete tombstone.
    pub fn schedule_delete_range<K: Serialize, V>(
        &mut self,
        db: &DBMap<K, V, S>,
        from: &K,
        to: &K,
    ) -> Result<(), TypedStoreError> {
        if !Arc::ptr_eq(&db.db, &self.database) {
            return Err(TypedStoreError::CrossDBBatch);
        }

        let from_buf = be_fix_int_ser(from)?;
        let to_buf = be_fix_int_ser(to)?;
        self.database
            .storage
            .batch_delete_range(&mut self.batch, &db.cf, from_buf, to_buf)
    }

    /// Inserts a range of (key, value) pairs given as an iterator.
    pub fn insert_batch<J: Borrow<K>, K: Serialize, U: Borrow<V>, V: Serialize>(
        &mut self,
        db: &DBMap<K, V, S>,
        new_vals: impl IntoIterator<Item = (J, U)>,
    ) -> Result<&mut Self, TypedStoreError> {
        if !Arc::ptr_eq(&db.db, &self.database) {
            return Err(TypedStoreError::CrossDBBatch);
        }
        let mut total = 0usize;
        new_vals
            .into_iter()
            .try_for_each::<_, Result<_, TypedStoreError>>(|(k, v)| {
                let k_buf = be_fix_int_ser(k.borrow())?;
                let v_buf = bcs::to_bytes(v.borrow()).map_err(typed_store_err_from_bcs_err)?;
                total += k_buf.len() + v_buf.len();
                self.database
                    .storage
                    .batch_put(&mut self.batch, &db.cf, k_buf, v_buf)
            })?;
        self.db_metrics.observe_batch_put_bytes(&db.cf, total as f64);
        Ok(self)
    }
}

// ---------------------------------------------------------------------------
// impl Map for DBMap
// ---------------------------------------------------------------------------

impl<'a, K, V> Map<'a, K, V> for DBMap<K, V, RocksDB>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    type Error = TypedStoreError;
    type SafeIterator = SafeIter<'a, K, V>;

    #[instrument(level = "trace", skip_all, err)]
    fn contains_key(&self, key: &K) -> Result<bool, TypedStoreError> {
        let key_buf = be_fix_int_ser(key)?;
        let readopts = self.read_opts.clone();
        Ok(self
            .db
            .storage
            .key_may_exist(&self.cf, &key_buf, &readopts)
            && self
                .db
                .storage
                .get(&self.cf, &key_buf, &readopts)?
                .is_some())
    }

    #[instrument(level = "trace", skip_all, err)]
    fn multi_contains_keys<J>(
        &self,
        keys: impl IntoIterator<Item = J>,
    ) -> Result<Vec<bool>, Self::Error>
    where
        J: Borrow<K>,
    {
        let values = self.multi_get_pinned(keys)?;
        Ok(values.into_iter().map(|v| v.is_some()).collect())
    }

    #[instrument(level = "trace", skip_all, err)]
    fn get(&self, key: &K) -> Result<Option<V>, TypedStoreError> {
        let _timer = self.db_metrics.start_get_timer(&self.cf);
        let perf_ctx = if self.get_sample_interval.sample() {
            Some(RocksDBPerfContext::default())
        } else {
            None
        };
        let key_buf = be_fix_int_ser(key)?;
        let readopts = self.read_opts.clone();
        let res = self
            .db
            .storage
            .get(&self.cf, &key_buf, &readopts)?;
        self.db_metrics.observe_get_bytes(&self.cf, res.as_ref().map_or(0.0, |v| v.len() as f64));
        if perf_ctx.is_some() {
            self.db_metrics.report_read_perf_ctx(&self.cf);
        }
        match res {
            Some(data) => Ok(Some(
                bcs::from_bytes(&data).map_err(typed_store_err_from_bcs_err)?,
            )),
            None => Ok(None),
        }
    }

    #[instrument(level = "trace", skip_all, err)]
    fn insert(&self, key: &K, value: &V) -> Result<(), TypedStoreError> {
        let timer = self.db_metrics.start_put_timer(&self.cf);
        let perf_ctx = if self.write_sample_interval.sample() {
            Some(RocksDBPerfContext::default())
        } else {
            None
        };
        let key_buf = be_fix_int_ser(key)?;
        let value_buf = bcs::to_bytes(value).map_err(typed_store_err_from_bcs_err)?;
        self.db_metrics.observe_put_bytes(&self.cf, (key_buf.len() + value_buf.len()) as f64);
        if perf_ctx.is_some() {
            self.db_metrics.report_write_perf_ctx_cf(&self.cf);
        }
        let writeopts = self.write_opts.clone();
        self.db
            .storage
            .put(&self.cf, key_buf, value_buf, &writeopts)?;

        let elapsed = timer.stop_and_record();
        if elapsed > 1.0 {
            warn!(?elapsed, cf = ?self.cf, "very slow insert");
            self.db_metrics.inc_very_slow_puts(&self.cf, (elapsed * 1000.0) as u64);
        }

        Ok(())
    }

    #[instrument(level = "trace", skip_all, err)]
    fn remove(&self, key: &K) -> Result<(), TypedStoreError> {
        let _timer = self.db_metrics.start_delete_timer(&self.cf);
        let perf_ctx = if self.write_sample_interval.sample() {
            Some(RocksDBPerfContext::default())
        } else {
            None
        };
        let key_buf = be_fix_int_ser(key)?;
        let writeopts = self.write_opts.clone();
        self.db
            .storage
            .delete(&self.cf, &key_buf, &writeopts)?;
        self.db_metrics.inc_deletes(&self.cf);
        if perf_ctx.is_some() {
            self.db_metrics.report_write_perf_ctx_cf(&self.cf);
        }
        Ok(())
    }

    /// Drops and recreates the column family (non-atomic).
    #[instrument(level = "trace", skip_all, err)]
    fn unsafe_clear(&self) -> Result<(), TypedStoreError> {
        let _ = self.db.storage.drop_cf(&self.cf);
        self.db
            .create_cf(&self.cf, &RocksDB::default_cf_options())?;
        Ok(())
    }

    /// Writes a range delete tombstone to delete all entries.
    #[instrument(level = "trace", skip_all, err)]
    fn schedule_delete_all(&self) -> Result<(), TypedStoreError> {
        let first_key = self.safe_iter().next().transpose()?.map(|(k, _v)| k);
        let last_key = self
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
            .transpose()?
            .map(|(k, _v)| k);
        if let Some((first_key, last_key)) = first_key.zip(last_key) {
            let mut batch = self.batch();
            batch.schedule_delete_range(self, &first_key, &last_key)?;
            batch.write()?;
        }
        Ok(())
    }

    fn is_empty(&self) -> bool {
        self.safe_iter().next().is_none()
    }

    fn safe_iter(&'a self) -> Self::SafeIterator {
        let readopts = self.read_opts.clone();
        let db_iter = self.db.storage.raw_iterator(&self.cf, readopts);
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        SafeIter::new(
            self.cf.clone(),
            db_iter,
            _timer,
            _perf_ctx,
            bytes_scanned,
            keys_scanned,
            Some(self.db_metrics.clone()),
        )
    }

    fn safe_iter_with_bounds(
        &'a self,
        lower_bound: Option<K>,
        upper_bound: Option<K>,
    ) -> Self::SafeIterator {
        let readopts = self.create_read_options_with_bounds(lower_bound, upper_bound);
        let db_iter = self.db.storage.raw_iterator(&self.cf, readopts);
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        SafeIter::new(
            self.cf.clone(),
            db_iter,
            _timer,
            _perf_ctx,
            bytes_scanned,
            keys_scanned,
            Some(self.db_metrics.clone()),
        )
    }

    fn safe_range_iter(&'a self, range: impl RangeBounds<K>) -> Self::SafeIterator {
        let readopts = self.create_read_options_with_range(range);
        let db_iter = self.db.storage.raw_iterator(&self.cf, readopts);
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        SafeIter::new(
            self.cf.clone(),
            db_iter,
            _timer,
            _perf_ctx,
            bytes_scanned,
            keys_scanned,
            Some(self.db_metrics.clone()),
        )
    }

    /// Returns a vector of values corresponding to the keys provided.
    #[instrument(level = "trace", skip_all, err)]
    fn multi_get<J>(
        &self,
        keys: impl IntoIterator<Item = J>,
    ) -> Result<Vec<Option<V>>, TypedStoreError>
    where
        J: Borrow<K>,
    {
        let results = self.multi_get_pinned(keys)?;
        results
            .into_iter()
            .map(|value_byte| match value_byte {
                Some(data) => Ok(Some(
                    bcs::from_bytes(&data).map_err(typed_store_err_from_bcs_err)?,
                )),
                None => Ok(None),
            })
            .collect()
    }

    #[instrument(level = "trace", skip_all, err)]
    fn multi_insert<J, U>(
        &self,
        key_val_pairs: impl IntoIterator<Item = (J, U)>,
    ) -> Result<(), Self::Error>
    where
        J: Borrow<K>,
        U: Borrow<V>,
    {
        let mut batch = self.batch();
        batch.insert_batch(self, key_val_pairs)?;
        batch.write()
    }

    #[instrument(level = "trace", skip_all, err)]
    fn multi_remove<J>(&self, keys: impl IntoIterator<Item = J>) -> Result<(), Self::Error>
    where
        J: Borrow<K>,
    {
        let mut batch = self.batch();
        batch.delete_batch(self, keys)?;
        batch.write()
    }

    #[instrument(level = "trace", skip_all, err)]
    fn try_catch_up_with_primary(&self) -> Result<(), Self::Error> {
        self.db.storage.try_catch_up_with_primary()
    }
}

// `clippy::manual_div_ceil` is raised by code expanded by the
// `uint::construct_uint!` macro so it needs to be fixed by `uint`
#[expect(clippy::assign_op_pattern, clippy::manual_div_ceil)]
#[test]
fn test_helpers() {
    let v = vec![];
    assert!(is_max(&v));

    fn check_add(v: Vec<u8>) {
        let mut v = v;
        let num = Num32::from_big_endian(&v);
        big_endian_saturating_add_one(&mut v);
        assert!(num + 1 == Num32::from_big_endian(&v));
    }

    uint::construct_uint! {
        // 32 byte number
        struct Num32(4);
    }

    let mut v = vec![255; 32];
    big_endian_saturating_add_one(&mut v);
    assert!(Num32::MAX == Num32::from_big_endian(&v));

    check_add(vec![1; 32]);
    check_add(vec![6; 32]);
    check_add(vec![254; 32]);
}
