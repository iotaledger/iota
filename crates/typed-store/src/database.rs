// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    borrow::Borrow,
    marker::PhantomData,
    ops::{Bound, Deref, RangeBounds},
    path::Path,
    sync::Arc,
    time::Duration,
};

use iota_macros::fail_point;
use prometheus::{Histogram, HistogramTimer};
use rocksdb::{
    DBPinnableSlice, DBWithThreadMode, Error, LiveFile, MultiThreaded, ReadOptions, WriteBatch,
    WriteOptions, checkpoint::Checkpoint,
};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::oneshot;
use tracing::{debug, error, instrument, warn};
use typed_store_error::TypedStoreError;

use crate::{
    memstore::{InMemoryBatch, InMemoryDB},
    metrics::{DBMetrics, RocksDBPerfContext, SamplingInterval},
    rocks::{
        RocksDB, be_fix_int_ser,
        errors::{typed_store_err_from_bcs_err, typed_store_err_from_rocks_err},
        rocks_cf,
        safe_iter::{SafeIter, SafeRevIter},
    },
    traits::{Map, TableSummary},
};

#[derive(Clone)]
pub(crate) enum ColumnFamily {
    Rocks(String),
    #[allow(dead_code)]
    InMemory(String),
}

impl std::fmt::Debug for ColumnFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnFamily::Rocks(name) => write!(f, "RocksDB cf: {}", name),
            ColumnFamily::InMemory(name) => write!(f, "InMemory cf: {}", name),
        }
    }
}

impl ColumnFamily {
    pub(crate) fn name(&self) -> &str {
        match self {
            ColumnFamily::Rocks(name) => name,
            ColumnFamily::InMemory(name) => name,
        }
    }

    pub(crate) fn rocks_cf<'a>(
        &self,
        rocks_db: &'a RocksDB,
    ) -> Arc<rocksdb::BoundColumnFamily<'a>> {
        match &self {
            ColumnFamily::Rocks(name) => rocks_db
                .underlying
                .cf_handle(name)
                .expect("Map-keying column family should have been checked at DB creation"),
            _ => unreachable!("invariant is checked by the caller"),
        }
    }
}

pub(crate) enum Storage {
    Rocks(RocksDB),
    #[allow(dead_code)]
    InMemory(InMemoryDB),
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Storage::Rocks(db) => write!(f, "RocksDB Storage {:?}", db),
            Storage::InMemory(db) => write!(f, "InMemoryDB Storage {:?}", db),
        }
    }
}

pub(crate) enum GetResult<'a> {
    Rocks(DBPinnableSlice<'a>),
    InMemory(Vec<u8>),
}

impl Deref for GetResult<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            GetResult::Rocks(d) => d.deref(),
            GetResult::InMemory(d) => d.deref(),
        }
    }
}

pub enum StorageWriteBatch {
    Rocks(rocksdb::WriteBatch),
    InMemory(InMemoryBatch),
}

pub type RawIter<'a> = rocksdb::DBRawIteratorWithThreadMode<'a, DBWithThreadMode<MultiThreaded>>;

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

#[derive(Debug)]
pub struct Database {
    pub(crate) storage: Storage,
    pub(crate) metric_conf: MetricConf,
}

impl Drop for Database {
    fn drop(&mut self) {
        DBMetrics::get().decrement_num_active_dbs(&self.metric_conf.db_name);
    }
}

impl Database {
    pub(crate) fn new(storage: Storage, metric_conf: MetricConf) -> Self {
        DBMetrics::get().increment_num_active_dbs(&metric_conf.db_name);
        Self {
            storage,
            metric_conf,
        }
    }

    pub(crate) fn get<K: AsRef<[u8]>>(
        &self,
        cf: &ColumnFamily,
        key: K,
        readopts: &ReadOptions,
    ) -> Result<Option<GetResult<'_>>, TypedStoreError> {
        match (&self.storage, cf) {
            (Storage::Rocks(db), ColumnFamily::Rocks(_)) => Ok(db
                .underlying
                .get_pinned_cf_opt(&cf.rocks_cf(db), key, readopts)
                .map_err(typed_store_err_from_rocks_err)?
                .map(GetResult::Rocks)),
            (Storage::InMemory(db), ColumnFamily::InMemory(cf_name)) => {
                Ok(db.get(cf_name, key).map(GetResult::InMemory))
            }

            _ => Err(TypedStoreError::RocksDB(
                "typed store invariant violation".to_string(),
            )),
        }
    }

    pub(crate) fn multi_get<I, K>(
        &self,
        cf: &ColumnFamily,
        keys: I,
        readopts: &ReadOptions,
    ) -> Vec<Result<Option<GetResult<'_>>, TypedStoreError>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        match (&self.storage, cf) {
            (Storage::Rocks(db), ColumnFamily::Rocks(_)) => {
                let res = db.underlying.batched_multi_get_cf_opt(
                    &cf.rocks_cf(db),
                    keys,
                    // sorted_input
                    false,
                    readopts,
                );
                res.into_iter()
                    .map(|r| {
                        r.map_err(typed_store_err_from_rocks_err)
                            .map(|item| item.map(GetResult::Rocks))
                    })
                    .collect()
            }
            (Storage::InMemory(db), ColumnFamily::InMemory(cf_name)) => db
                .multi_get(cf_name, keys)
                .into_iter()
                .map(|r| Ok(r.map(GetResult::InMemory)))
                .collect(),
            _ => unreachable!("typed store invariant violation"),
        }
    }

    pub fn create_cf<N: AsRef<str>>(
        &self,
        name: N,
        opts: &rocksdb::Options,
    ) -> Result<(), rocksdb::Error> {
        match &self.storage {
            Storage::Rocks(db) => db.underlying.create_cf(name, opts),
            Storage::InMemory(_) => Ok(()),
        }
    }

    pub fn cf_handle(&self, name: &str) -> Option<()> {
        match &self.storage {
            Storage::Rocks(db) => db.underlying.cf_handle(name).map(|_| ()),
            Storage::InMemory(db) => db.has_cf(name).then_some(()),
        }
    }

    pub fn drop_cf(&self, name: &str) -> Result<(), rocksdb::Error> {
        match &self.storage {
            Storage::Rocks(db) => db.underlying.drop_cf(name),
            Storage::InMemory(db) => {
                db.drop_cf(name);
                Ok(())
            }
        }
    }

    pub(crate) fn delete_cf<K: AsRef<[u8]>>(
        &self,
        cf: &ColumnFamily,
        key: K,
        writeopts: &WriteOptions,
    ) -> Result<(), TypedStoreError> {
        fail_point!("delete-cf-before");
        let ret = match (&self.storage, cf) {
            (Storage::Rocks(db), ColumnFamily::Rocks(_)) => db
                .underlying
                .delete_cf_opt(&cf.rocks_cf(db), key, writeopts)
                .map_err(typed_store_err_from_rocks_err),
            (Storage::InMemory(db), ColumnFamily::InMemory(cf_name)) => {
                db.delete(cf_name, key.as_ref());
                Ok(())
            }
            _ => Err(TypedStoreError::RocksDB(
                "typed store invariant violation".to_string(),
            )),
        };
        fail_point!("delete-cf-after");
        #[allow(clippy::let_and_return)]
        ret
    }

    pub fn path_for_pruning(&self) -> &Path {
        match &self.storage {
            Storage::Rocks(rocks) => rocks.underlying.path(),
            _ => unimplemented!("method is only supported for rocksdb backend"),
        }
    }

    pub(crate) fn put_cf(
        &self,
        cf: &ColumnFamily,
        key: Vec<u8>,
        value: Vec<u8>,
        writeopts: &WriteOptions,
    ) -> Result<(), TypedStoreError> {
        fail_point!("put-cf-before");
        let ret = match (&self.storage, cf) {
            (Storage::Rocks(db), ColumnFamily::Rocks(_)) => db
                .underlying
                .put_cf_opt(&cf.rocks_cf(db), key, value, writeopts)
                .map_err(typed_store_err_from_rocks_err),
            (Storage::InMemory(db), ColumnFamily::InMemory(cf_name)) => {
                db.put(cf_name, key, value);
                Ok(())
            }
            _ => Err(TypedStoreError::RocksDB(
                "typed store invariant violation".to_string(),
            )),
        };
        fail_point!("put-cf-after");
        #[allow(clippy::let_and_return)]
        ret
    }

    pub(crate) fn key_may_exist_cf<K: AsRef<[u8]>>(
        &self,
        cf: &ColumnFamily,
        key: K,
        readopts: &ReadOptions,
    ) -> bool {
        match &self.storage {
            // [`rocksdb::DBWithThreadMode::key_may_exist_cf`] can have false positives,
            // but no false negatives. We use it to short-circuit the absent case
            Storage::Rocks(rocks) => {
                rocks
                    .underlying
                    .key_may_exist_cf_opt(&rocks_cf(rocks, cf.name()), key, readopts)
            }
            _ => true,
        }
    }

    pub fn write(
        &self,
        batch: StorageWriteBatch,
        writeopts: &WriteOptions,
    ) -> Result<(), TypedStoreError> {
        fail_point!("batch-write-before");
        let ret = match (&self.storage, batch) {
            (Storage::Rocks(rocks), StorageWriteBatch::Rocks(batch)) => rocks
                .underlying
                .write_opt(batch, writeopts)
                .map_err(typed_store_err_from_rocks_err),
            (Storage::InMemory(db), StorageWriteBatch::InMemory(batch)) => {
                db.write(batch);
                Ok(())
            }
            _ => Err(TypedStoreError::RocksDB(
                "using invalid batch type for the database".to_string(),
            )),
        };
        fail_point!("batch-write-after");

        #[allow(clippy::let_and_return)]
        ret
    }

    pub(crate) fn raw_iterator_cf<'a: 'b, 'b>(
        &'a self,
        cf: &ColumnFamily,
        readopts: ReadOptions,
    ) -> RawIter<'b> {
        match &self.storage {
            Storage::Rocks(rocksdb) => rocksdb
                .underlying
                .raw_iterator_cf_opt(&rocks_cf(rocksdb, cf.name()), readopts),
            _ => unimplemented!("iterators not yet supported for other backends"),
        }
    }

    pub(crate) fn compact_range_cf<K: AsRef<[u8]>>(
        &self,
        cf: &ColumnFamily,
        start: Option<K>,
        end: Option<K>,
    ) {
        if let Storage::Rocks(rocksdb) = &self.storage {
            rocksdb
                .underlying
                .compact_range_cf(&rocks_cf(rocksdb, cf.name()), start, end);
        }
    }

    pub fn flush(&self) -> Result<(), TypedStoreError> {
        match &self.storage {
            Storage::Rocks(db) => db
                .underlying
                .flush()
                .map_err(typed_store_err_from_rocks_err),
            Storage::InMemory(_) => Ok(()),
        }
    }

    pub fn checkpoint(&self, path: &Path) -> Result<(), TypedStoreError> {
        // TODO: implement for other storage types
        if let Storage::Rocks(rocks) = &self.storage {
            let checkpoint =
                Checkpoint::new(&rocks.underlying).map_err(typed_store_err_from_rocks_err)?;
            checkpoint
                .create_checkpoint(path)
                .map_err(|e| TypedStoreError::RocksDB(e.to_string()))?;
        }
        Ok(())
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

    pub fn live_files(&self) -> Result<Vec<LiveFile>, Error> {
        match &self.storage {
            Storage::Rocks(rocks) => rocks.underlying.live_files(),
            _ => Ok(vec![]),
        }
    }

    pub(crate) fn try_catch_up_with_primary(&self) -> Result<(), TypedStoreError> {
        if let Storage::Rocks(rocks) = &self.storage {
            rocks
                .underlying
                .try_catch_up_with_primary()
                .map_err(typed_store_err_from_rocks_err)?;
        }
        Ok(())
    }
}

fn rocks_cf_from_db<'a>(
    db: &'a Database,
    cf_name: &str,
) -> Result<Arc<rocksdb::BoundColumnFamily<'a>>, TypedStoreError> {
    match &db.storage {
        Storage::Rocks(rocksdb) => Ok(rocksdb
            .underlying
            .cf_handle(cf_name)
            .expect("Map-keying column family should have been checked at DB creation")),
        _ => Err(TypedStoreError::RocksDB(
            "using invalid batch type for the database".to_string(),
        )),
    }
}

/// An interface to a rocksDB database, keyed by a columnfamily
#[derive(Clone, Debug)]
pub struct DBMap<K, V> {
    pub db: Arc<Database>,
    _phantom: PhantomData<fn(K) -> V>,
    column_family: ColumnFamily,
    pub opts: ReadWriteOptions,
    db_metrics: Arc<DBMetrics>,
    get_sample_interval: SamplingInterval,
    multiget_sample_interval: SamplingInterval,
    write_sample_interval: SamplingInterval,
    iter_sample_interval: SamplingInterval,
    _metrics_task_cancel_handle: Arc<oneshot::Sender<()>>,
}

unsafe impl<K: Send, V: Send> Send for DBMap<K, V> {}

impl<K, V> DBMap<K, V> {
    pub(crate) fn new(
        db: Arc<Database>,
        opts: &ReadWriteOptions,
        column_family: ColumnFamily,
        is_deprecated: bool,
    ) -> Self {
        let db_cloned = Arc::downgrade(&db);
        let db_metrics = DBMetrics::get();
        let db_metrics_cloned = db_metrics.clone();
        let cf = column_family.name().to_string();

        let (sender, mut recv) = tokio::sync::oneshot::channel();
        if !is_deprecated && matches!(db.storage, Storage::Rocks(_)) {
            tokio::task::spawn(async move {
                let mut interval =
                    tokio::time::interval(Duration::from_secs(CF_METRICS_REPORT_PERIOD_SECS));
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            if let Some(db) = db_cloned.upgrade() {
                                let cf = cf.clone();
                                let db_metrics = db_metrics.clone();
                                if let Err(e) = tokio::task::spawn_blocking(move || {
                                    Self::report_rocksdb_metrics(&db, &cf, &db_metrics);
                                }).await {
                                    error!("Failed to log metrics with error: {}", e);
                                }
                            } else {
                                break;
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
            opts: opts.clone(),
            _phantom: PhantomData,
            column_family,
            db_metrics: db_metrics_cloned,
            _metrics_task_cancel_handle: Arc::new(sender),
            get_sample_interval: db.get_sampling_interval(),
            multiget_sample_interval: db.multiget_sampling_interval(),
            write_sample_interval: db.write_sampling_interval(),
            iter_sample_interval: db.iter_sampling_interval(),
        }
    }

    /// Reopens an open database as a typed map operating under a specific
    /// column family. if no column family is passed, the default column
    /// family is used.
    ///
    /// ```
    /// use core::fmt::Error;
    /// use std::sync::Arc;
    ///
    /// use prometheus::Registry;
    /// use tempfile::tempdir;
    /// use typed_store::{metrics::DBMetrics, rocks::*};
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
    ///     let db_cf_2 = DBMap::<u32, u32>::reopen(
    ///         &rocks,
    ///         Some("Second_CF"),
    ///         &ReadWriteOptions::default(),
    ///         false,
    ///     )
    ///     .expect("Failed to open storage");
    ///     Ok(())
    /// }
    /// ```
    #[instrument(level = "debug", skip(db), err)]
    pub fn reopen(
        db: &Arc<Database>,
        opt_cf: Option<&str>,
        rw_options: &ReadWriteOptions,
        is_deprecated: bool,
    ) -> Result<Self, TypedStoreError> {
        let cf_key = opt_cf
            .unwrap_or(rocksdb::DEFAULT_COLUMN_FAMILY_NAME)
            .to_owned();

        let column_family = match &db.storage {
            Storage::Rocks(_) => ColumnFamily::Rocks(cf_key.clone()),
            Storage::InMemory(_) => ColumnFamily::InMemory(cf_key.clone()),
        };
        Ok(DBMap::new(
            db.clone(),
            rw_options,
            column_family,
            is_deprecated,
        ))
    }

    pub fn cf_name(&self) -> &str {
        self.column_family.name()
    }

    pub fn batch(&self) -> DBBatch {
        let batch = match &self.db.storage {
            Storage::Rocks(_) => StorageWriteBatch::Rocks(WriteBatch::default()),
            Storage::InMemory(_) => StorageWriteBatch::InMemory(InMemoryBatch::default()),
        };
        DBBatch::new(
            &self.db,
            batch,
            self.opts.writeopts(),
            &self.db_metrics,
            &self.write_sample_interval,
        )
    }

    pub fn compact_range<J: Serialize>(&self, start: &J, end: &J) -> Result<(), TypedStoreError> {
        let from_buf = be_fix_int_ser(start)?;
        let to_buf = be_fix_int_ser(end)?;
        self.db
            .compact_range_cf(&self.column_family, Some(from_buf), Some(to_buf));
        Ok(())
    }

    pub fn compact_range_raw(
        &self,
        cf_name: &str,
        start: Vec<u8>,
        end: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        let cf = match &self.db.storage {
            Storage::Rocks(_) => ColumnFamily::Rocks(cf_name.to_string()),
            Storage::InMemory(_) => ColumnFamily::InMemory(cf_name.to_string()),
        };
        self.db.compact_range_cf(&cf, Some(start), Some(end));
        Ok(())
    }

    /// Returns a vector of raw values corresponding to the keys provided.
    fn multi_get_pinned<J>(
        &self,
        keys: impl IntoIterator<Item = J>,
    ) -> Result<Vec<Option<GetResult<'_>>>, TypedStoreError>
    where
        J: Borrow<K>,
        K: Serialize,
    {
        let _timer = self
            .db_metrics
            .op_metrics
            .rocksdb_multiget_latency_seconds
            .with_label_values(&[self.cf_name()])
            .start_timer();
        let perf_ctx = if self.multiget_sample_interval.sample() {
            Some(RocksDBPerfContext)
        } else {
            None
        };
        let keys_bytes: Result<Vec<_>, TypedStoreError> = keys
            .into_iter()
            .map(|k| be_fix_int_ser(k.borrow()))
            .collect();
        let results: Result<Vec<_>, TypedStoreError> = self
            .db
            .multi_get(&self.column_family, keys_bytes?, &self.opts.readopts())
            .into_iter()
            .collect();
        let entries = results?;
        let entry_size = entries
            .iter()
            .flatten()
            .map(|entry| entry.len())
            .sum::<usize>();
        self.db_metrics
            .op_metrics
            .rocksdb_multiget_bytes
            .with_label_values(&[self.cf_name()])
            .observe(entry_size as f64);
        if perf_ctx.is_some() {
            self.db_metrics
                .read_perf_ctx_metrics
                .report_metrics(self.cf_name());
        }
        Ok(entries)
    }

    pub fn checkpoint_db(&self, path: &Path) -> Result<(), TypedStoreError> {
        self.db.checkpoint(path)
    }

    pub fn table_summary(&self) -> eyre::Result<TableSummary>
    where
        K: Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned,
    {
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

    // Creates metrics and context for tracking an iterator usage and performance.
    fn create_iter_context(
        &self,
    ) -> (
        Option<HistogramTimer>,
        Option<Histogram>,
        Option<Histogram>,
        Option<RocksDBPerfContext>,
    ) {
        let timer = self
            .db_metrics
            .op_metrics
            .rocksdb_iter_latency_seconds
            .with_label_values(&[self.cf_name()])
            .start_timer();
        let bytes_scanned = self
            .db_metrics
            .op_metrics
            .rocksdb_iter_bytes
            .with_label_values(&[self.cf_name()]);
        let keys_scanned = self
            .db_metrics
            .op_metrics
            .rocksdb_iter_keys
            .with_label_values(&[self.cf_name()]);
        let perf_ctx = if self.iter_sample_interval.sample() {
            Some(RocksDBPerfContext)
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

    // Creates a RocksDB read option with specified lower and upper bounds.
    /// Lower bound is inclusive, and upper bound is exclusive.
    fn create_read_options_with_bounds(
        &self,
        lower_bound: Option<K>,
        upper_bound: Option<K>,
    ) -> ReadOptions
    where
        K: Serialize,
    {
        let mut readopts = self.opts.readopts();
        if let Some(lower_bound) = lower_bound {
            let key_buf = be_fix_int_ser(&lower_bound).unwrap();
            readopts.set_iterate_lower_bound(key_buf);
        }
        if let Some(upper_bound) = upper_bound {
            let key_buf = be_fix_int_ser(&upper_bound).unwrap();
            readopts.set_iterate_upper_bound(key_buf);
        }
        readopts
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
        let upper_bound_key = upper_bound.as_ref().map(|k| be_fix_int_ser(&k));
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

        let db_iter = self.db.raw_iterator_cf(&self.column_family, readopts);
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        let iter = SafeIter::new(
            self.cf_name().to_string(),
            db_iter,
            _timer,
            _perf_ctx,
            bytes_scanned,
            keys_scanned,
            Some(self.db_metrics.clone()),
        );
        Ok(SafeRevIter::new(iter, upper_bound_key.transpose()?))
    }

    // Creates a RocksDB read option with lower and upper bounds set corresponding
    // to `range`.
    fn create_read_options_with_range(&self, range: impl RangeBounds<K>) -> ReadOptions
    where
        K: Serialize,
    {
        let mut readopts = self.opts.readopts();

        let lower_bound = range.start_bound();
        let upper_bound = range.end_bound();

        match lower_bound {
            Bound::Included(lower_bound) => {
                // Rocksdb lower bound is inclusive by default so nothing to do
                let key_buf = be_fix_int_ser(&lower_bound).expect("Serialization must not fail");
                readopts.set_iterate_lower_bound(key_buf);
            }
            Bound::Excluded(lower_bound) => {
                let mut key_buf =
                    be_fix_int_ser(&lower_bound).expect("Serialization must not fail");

                // Since we want exclusive, we need to increment the key to exclude the previous
                big_endian_saturating_add_one(&mut key_buf);
                readopts.set_iterate_lower_bound(key_buf);
            }
            Bound::Unbounded => (),
        };

        match upper_bound {
            Bound::Included(upper_bound) => {
                let mut key_buf =
                    be_fix_int_ser(&upper_bound).expect("Serialization must not fail");

                // If the key is already at the limit, there's nowhere else to go, so no upper
                // bound
                if !is_max(&key_buf) {
                    // Since we want exclusive, we need to increment the key to get the upper bound
                    big_endian_saturating_add_one(&mut key_buf);
                    readopts.set_iterate_upper_bound(key_buf);
                }
            }
            Bound::Excluded(upper_bound) => {
                // Rocksdb upper bound is inclusive by default so nothing to do
                let key_buf = be_fix_int_ser(&upper_bound).expect("Serialization must not fail");
                readopts.set_iterate_upper_bound(key_buf);
            }
            Bound::Unbounded => (),
        };

        readopts
    }
}

/// Provides a mutable struct to form a collection of database write operations,
/// and execute them.
///
/// Batching write and delete operations is faster than performing them one by
/// one and ensures their atomicity,  ie. they are all written or none is.
/// This is also true of operations across column families in the same database.
///
/// Serializations / Deserialization, and naming of column families is performed
/// by passing a DBMap<K,V> with each operation.
///
/// ```
/// use core::fmt::Error;
/// use std::sync::Arc;
///
/// use prometheus::Registry;
/// use tempfile::tempdir;
/// use typed_store::{Map, metrics::DBMetrics, rocks::*};
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
///     for (k, v) in keys_vals_1 {
///         let val = db_cf_1.get(&k).expect("Failed to get inserted key");
///         assert_eq!(Some(v), val);
///     }
///
///     for (k, v) in keys_vals_2 {
///         let val = db_cf_2.get(&k).expect("Failed to get inserted key");
///         assert_eq!(Some(v), val);
///     }
///     Ok(())
/// }
/// ```
pub struct DBBatch {
    database: Arc<Database>,
    batch: StorageWriteBatch,
    opts: WriteOptions,
    db_metrics: Arc<DBMetrics>,
    write_sample_interval: SamplingInterval,
}

impl DBBatch {
    /// Create a new batch associated with a DB reference.
    ///
    /// Use `open_cf` to get the DB reference or an existing open database.
    pub fn new(
        dbref: &Arc<Database>,
        batch: StorageWriteBatch,
        opts: WriteOptions,
        db_metrics: &Arc<DBMetrics>,
        write_sample_interval: &SamplingInterval,
    ) -> Self {
        DBBatch {
            database: dbref.clone(),
            batch,
            opts,
            db_metrics: db_metrics.clone(),
            write_sample_interval: write_sample_interval.clone(),
        }
    }

    /// Consume the batch and write its operations to the database
    #[instrument(level = "trace", skip_all, err)]
    pub fn write(self) -> Result<(), TypedStoreError> {
        let db_name = self.database.db_name();
        let timer = self
            .db_metrics
            .op_metrics
            .rocksdb_batch_commit_latency_seconds
            .with_label_values(&[&db_name])
            .start_timer();
        let batch_size = self.size_in_bytes();

        let perf_ctx = if self.write_sample_interval.sample() {
            Some(RocksDBPerfContext)
        } else {
            None
        };
        self.database.write(self.batch, &self.opts)?;
        self.db_metrics
            .op_metrics
            .rocksdb_batch_commit_bytes
            .with_label_values(&[&db_name])
            .observe(batch_size as f64);

        if perf_ctx.is_some() {
            self.db_metrics
                .write_perf_ctx_metrics
                .report_metrics(&db_name);
        }
        let elapsed = timer.stop_and_record();
        if elapsed > 1.0 {
            warn!(?elapsed, ?db_name, "very slow batch write");
            self.db_metrics
                .op_metrics
                .rocksdb_very_slow_batch_writes_count
                .with_label_values(&[&db_name])
                .inc();
            self.db_metrics
                .op_metrics
                .rocksdb_very_slow_batch_writes_duration_ms
                .with_label_values(&[&db_name])
                .inc_by((elapsed * 1000.0) as u64);
        }
        Ok(())
    }

    pub fn size_in_bytes(&self) -> usize {
        match self.batch {
            StorageWriteBatch::Rocks(ref b) => b.size_in_bytes(),
            StorageWriteBatch::InMemory(_) => 0,
        }
    }

    pub fn delete_batch<J: Borrow<K>, K: Serialize, V>(
        &mut self,
        db: &DBMap<K, V>,
        purged_vals: impl IntoIterator<Item = J>,
    ) -> Result<(), TypedStoreError> {
        if !Arc::ptr_eq(&db.db, &self.database) {
            return Err(TypedStoreError::CrossDBBatch);
        }

        purged_vals
            .into_iter()
            .try_for_each::<_, Result<_, TypedStoreError>>(|k| {
                let k_buf = be_fix_int_ser(k.borrow())?;
                match (&mut self.batch, &db.column_family) {
                    (StorageWriteBatch::Rocks(b), ColumnFamily::Rocks(name)) => {
                        b.delete_cf(&rocks_cf_from_db(&self.database, name)?, k_buf)
                    }
                    (StorageWriteBatch::InMemory(b), ColumnFamily::InMemory(name)) => {
                        b.delete_cf(name, k_buf)
                    }
                    _ => Err(TypedStoreError::RocksDB(
                        "typed store invariant violation".to_string(),
                    ))?,
                }
                Ok(())
            })?;
        Ok(())
    }

    /// Deletes a range of keys between `from` (inclusive) and `to`
    /// (non-inclusive) by writing a range delete tombstone in the db map
    /// If the DBMap is configured with ignore_range_deletions set to false,
    /// the effect of this write will be visible immediately i.e. you won't
    /// see old values when you do a lookup or scan. But if it is configured
    /// with ignore_range_deletions set to true, the old value are visible until
    /// compaction actually deletes them which will happen sometime after. By
    /// default ignore_range_deletions is set to true on a DBMap (unless it is
    /// overridden in the config), so please use this function with caution
    pub fn schedule_delete_range<K: Serialize, V>(
        &mut self,
        db: &DBMap<K, V>,
        from: &K,
        to: &K,
    ) -> Result<(), TypedStoreError> {
        if !Arc::ptr_eq(&db.db, &self.database) {
            return Err(TypedStoreError::CrossDBBatch);
        }

        let from_buf = be_fix_int_ser(from)?;
        let to_buf = be_fix_int_ser(to)?;

        if let StorageWriteBatch::Rocks(b) = &mut self.batch {
            b.delete_range_cf(
                &rocks_cf_from_db(&self.database, db.cf_name())?,
                from_buf,
                to_buf,
            );
        }
        Ok(())
    }

    /// inserts a range of (key, value) pairs given as an iterator
    pub fn insert_batch<J: Borrow<K>, K: Serialize, U: Borrow<V>, V: Serialize>(
        &mut self,
        db: &DBMap<K, V>,
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
                match (&mut self.batch, &db.column_family) {
                    (StorageWriteBatch::Rocks(b), ColumnFamily::Rocks(name)) => {
                        b.put_cf(&rocks_cf_from_db(&self.database, name)?, k_buf, v_buf)
                    }
                    (StorageWriteBatch::InMemory(b), ColumnFamily::InMemory(name)) => {
                        b.put_cf(name, k_buf, v_buf)
                    }
                    _ => Err(TypedStoreError::RocksDB(
                        "typed store invariant violation".to_string(),
                    ))?,
                }
                Ok(())
            })?;
        self.db_metrics
            .op_metrics
            .rocksdb_batch_put_bytes
            .with_label_values(&[db.cf_name()])
            .observe(total as f64);
        Ok(self)
    }
}

impl<'a, K, V> Map<'a, K, V> for DBMap<K, V>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    type Error = TypedStoreError;
    type SafeIterator = SafeIter<'a, K, V>;

    #[instrument(level = "trace", skip_all, err)]
    fn contains_key(&self, key: &K) -> Result<bool, TypedStoreError> {
        let key_buf = be_fix_int_ser(key)?;
        let readopts = self.opts.readopts();
        Ok(self
            .db
            .key_may_exist_cf(&self.column_family, &key_buf, &readopts)
            && self
                .db
                .get(&self.column_family, &key_buf, &readopts)?
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
        let _timer = self
            .db_metrics
            .op_metrics
            .rocksdb_get_latency_seconds
            .with_label_values(&[self.cf_name()])
            .start_timer();
        let perf_ctx = if self.get_sample_interval.sample() {
            Some(RocksDBPerfContext)
        } else {
            None
        };
        let key_buf = be_fix_int_ser(key)?;
        let res = self
            .db
            .get(&self.column_family, &key_buf, &self.opts.readopts())?;
        self.db_metrics
            .op_metrics
            .rocksdb_get_bytes
            .with_label_values(&[self.cf_name()])
            .observe(res.as_ref().map_or(0.0, |v| v.len() as f64));
        if perf_ctx.is_some() {
            self.db_metrics
                .read_perf_ctx_metrics
                .report_metrics(self.cf_name());
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
        let timer = self
            .db_metrics
            .op_metrics
            .rocksdb_put_latency_seconds
            .with_label_values(&[self.cf_name()])
            .start_timer();
        let perf_ctx = if self.write_sample_interval.sample() {
            Some(RocksDBPerfContext)
        } else {
            None
        };
        let key_buf = be_fix_int_ser(key)?;
        let value_buf = bcs::to_bytes(value).map_err(typed_store_err_from_bcs_err)?;
        self.db_metrics
            .op_metrics
            .rocksdb_put_bytes
            .with_label_values(&[self.cf_name()])
            .observe((key_buf.len() + value_buf.len()) as f64);
        if perf_ctx.is_some() {
            self.db_metrics
                .write_perf_ctx_metrics
                .report_metrics(self.cf_name());
        }
        self.db.put_cf(
            &self.column_family,
            key_buf,
            value_buf,
            &self.opts.writeopts(),
        )?;

        let elapsed = timer.stop_and_record();
        if elapsed > 1.0 {
            warn!(?elapsed, cf = ?self.cf_name(), "very slow insert");
            self.db_metrics
                .op_metrics
                .rocksdb_very_slow_puts_count
                .with_label_values(&[self.cf_name()])
                .inc();
            self.db_metrics
                .op_metrics
                .rocksdb_very_slow_puts_duration_ms
                .with_label_values(&[self.cf_name()])
                .inc_by((elapsed * 1000.0) as u64);
        }

        Ok(())
    }

    #[instrument(level = "trace", skip_all, err)]
    fn remove(&self, key: &K) -> Result<(), TypedStoreError> {
        let _timer = self
            .db_metrics
            .op_metrics
            .rocksdb_delete_latency_seconds
            .with_label_values(&[self.cf_name()])
            .start_timer();
        let perf_ctx = if self.write_sample_interval.sample() {
            Some(RocksDBPerfContext)
        } else {
            None
        };
        let key_buf = be_fix_int_ser(key)?;
        self.db
            .delete_cf(&self.column_family, key_buf, &self.opts.writeopts())?;
        self.db_metrics
            .op_metrics
            .rocksdb_deletes
            .with_label_values(&[self.cf_name()])
            .inc();
        if perf_ctx.is_some() {
            self.db_metrics
                .write_perf_ctx_metrics
                .report_metrics(self.cf_name());
        }
        Ok(())
    }

    /// This method first drops the existing column family and then creates a
    /// new one with the same name. The two operations are not atomic and
    /// hence it is possible to get into a race condition where the column
    /// family has been dropped but new one is not created yet
    #[instrument(level = "trace", skip_all, err)]
    fn unsafe_clear(&self) -> Result<(), TypedStoreError> {
        let _ = self.db.drop_cf(self.cf_name());
        self.db
            .create_cf(self.cf_name(), &crate::rocks::default_db_options().options)
            .map_err(typed_store_err_from_rocks_err)?;
        Ok(())
    }

    /// Writes a range delete tombstone to delete all entries in the db map
    /// If the DBMap is configured with ignore_range_deletions set to false,
    /// the effect of this write will be visible immediately i.e. you won't
    /// see old values when you do a lookup or scan. But if it is configured
    /// with ignore_range_deletions set to true, the old value are visible until
    /// compaction actually deletes them which will happen sometime after. By
    /// default ignore_range_deletions is set to true on a DBMap (unless it is
    /// overridden in the config), so please use this function with caution
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
        let db_iter = self
            .db
            .raw_iterator_cf(&self.column_family, self.opts.readopts());
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        SafeIter::new(
            self.cf_name().to_string(),
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
        let db_iter = self.db.raw_iterator_cf(&self.column_family, readopts);
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        SafeIter::new(
            self.cf_name().to_string(),
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
        let db_iter = self.db.raw_iterator_cf(&self.column_family, readopts);
        let (_timer, bytes_scanned, keys_scanned, _perf_ctx) = self.create_iter_context();
        SafeIter::new(
            self.cf_name().to_string(),
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
        let values_parsed: Result<Vec<_>, TypedStoreError> = results
            .into_iter()
            .map(|value_byte| match value_byte {
                Some(data) => Ok(Some(
                    bcs::from_bytes(&data).map_err(typed_store_err_from_bcs_err)?,
                )),
                None => Ok(None),
            })
            .collect();

        values_parsed
    }

    /// Convenience method for batch insertion
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

    /// Convenience method for batch removal
    #[instrument(level = "trace", skip_all, err)]
    fn multi_remove<J>(&self, keys: impl IntoIterator<Item = J>) -> Result<(), Self::Error>
    where
        J: Borrow<K>,
    {
        let mut batch = self.batch();
        batch.delete_batch(self, keys)?;
        batch.write()
    }

    /// Try to catch up with primary when running as secondary
    #[instrument(level = "trace", skip_all, err)]
    fn try_catch_up_with_primary(&self) -> Result<(), Self::Error> {
        self.db.try_catch_up_with_primary()
    }
}

#[derive(Clone, Debug)]
pub struct ReadWriteOptions {
    pub ignore_range_deletions: bool,
    // Whether to sync to disk on every write.
    sync_to_disk: bool,
}

impl ReadWriteOptions {
    pub fn readopts(&self) -> ReadOptions {
        let mut readopts = ReadOptions::default();
        readopts.set_ignore_range_deletions(self.ignore_range_deletions);
        readopts
    }

    pub fn writeopts(&self) -> WriteOptions {
        let mut opts = WriteOptions::default();
        opts.set_sync(self.sync_to_disk);
        opts
    }

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

/// Given a `vec<u8>`, find the value which is one more than the vector
/// if the vector was a big endian number.
/// If the vector is already minimum, don't change it.
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

/// Check if all the bytes in the vector are 0xFF
pub(crate) fn is_max(v: &[u8]) -> bool {
    v.iter().all(|&x| x == u8::MAX)
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

    // TBD: More tests coming with randomized arrays
}
