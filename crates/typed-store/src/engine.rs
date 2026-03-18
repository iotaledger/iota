// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, ops::Deref, path::Path, sync::Arc};

use typed_store_error::TypedStoreError;

// ---------------------------------------------------------------------------
// MetricsTimer — thin wrapper around an optional Prometheus histogram timer
// ---------------------------------------------------------------------------

/// Wraps an optional [`prometheus::HistogramTimer`] so that the generic
/// storage layer can record latencies without knowing the concrete metrics
/// type at compile time.
///
/// Calling [`stop_and_record`] on a no-op timer (inner `None`) returns `0.0`.
pub struct MetricsTimer(pub Option<prometheus::HistogramTimer>);

impl MetricsTimer {
    pub fn stop_and_record(self) -> f64 {
        self.0.map(|t| t.stop_and_record()).unwrap_or(0.0)
    }
}

// ---------------------------------------------------------------------------
// StorageMetrics — backend-agnostic metrics interface used by generic code
// ---------------------------------------------------------------------------

/// Metrics interface consumed by the generic storage layer (`Database`,
/// `DBMap`, `DBBatch`).
///
/// All methods have default no-op implementations so that backends without
/// instrumentation (e.g. the in-memory store) need not override anything.
/// RocksDB provides a full implementation via [`DBMetrics`].
pub trait StorageMetrics: Send + Sync + 'static {
    /// Called when a [`Database`] handle is created.
    fn on_db_opened(&self, _db_name: &str) {}

    /// Called when a [`Database`] handle is dropped.
    fn on_db_closed(&self, _db_name: &str) {}

    /// Start a timer measuring the duration of a batch commit.
    fn start_batch_commit_timer(&self, _db_name: &str) -> MetricsTimer {
        MetricsTimer(None)
    }

    /// Record the byte size of a committed batch.
    fn observe_batch_commit_bytes(&self, _db_name: &str, _bytes: f64) {}

    /// Activate per-thread write performance context capture.
    ///
    /// Called immediately before a batch write when `sample` is `true`.
    fn begin_write_perf_ctx(&self, _sample: bool) {}

    /// Deactivate per-thread write performance context and emit metrics.
    ///
    /// Called immediately after a batch write when `sample` is `true`.
    fn end_write_perf_ctx(&self, _sample: bool, _db_name: &str) {}

    /// Record a batch write that exceeded the slow-write threshold.
    fn inc_very_slow_batch_writes(&self, _db_name: &str, _elapsed_ms: u64) {}

    /// Record the byte size of key+value pairs appended to a batch.
    fn observe_batch_put_bytes(&self, _cf_name: &str, _bytes: f64) {}
}

/// No-op metrics implementation used by the in-memory backend.
impl StorageMetrics for () {}

/// Cursor interface over raw (byte-level) key-value pairs.
///
/// Implementors may be seekable forward/backward iterators (e.g. a RocksDB raw
/// iterator) or stub types for backends that do not support iteration.
pub trait RawDbIterator {
    fn seek_to_first(&mut self);
    fn seek_to_last(&mut self);
    fn seek(&mut self, key: &[u8]);
    fn seek_for_prev(&mut self, key: &[u8]);
    fn valid(&self) -> bool;
    fn key(&self) -> Option<&[u8]>;
    fn value(&self) -> Option<&[u8]>;
    fn next(&mut self);
    fn prev(&mut self);
    fn status(&self) -> Result<(), TypedStoreError>;
}

/// Abstraction over a key-value storage backend, parameterised on concrete
/// batch, read-option, write-option, column-family-option, and raw-iterator
/// types.
///
/// Implementors provide a concrete backend (e.g. RocksDB, in-memory).  All
/// column-family operations accept the column-family name as a plain `&str`;
/// the implementation is responsible for resolving handles internally.
///
/// Default method implementations are provided for operations that are either
/// optional or only meaningful for file-backed backends.
pub trait StorageEngine: Send + Sync + fmt::Debug + 'static {
    /// The type used to accumulate writes before committing them atomically.
    type Batch: Send;

    /// The byte slice view returned by a single-key `get`.  Must `Deref` to
    /// `[u8]`.  May borrow from `self` (lifetime `'a`).
    type GetValue<'a>: Deref<Target = [u8]>
    where
        Self: 'a;

    /// Options governing read / iterator operations.
    type ReadOptions: Default + Clone;

    /// Options governing single-key and batch write operations.
    type WriteOptions: Default + Clone;

    /// Options governing the creation of a column family.
    type CfOptions: Default;

    /// Raw seekable cursor over a column family.
    /// May be `NeverIter` for backends that do not support iteration.
    type RawIter<'a>: RawDbIterator
    where
        Self: 'a;

    /// Backend-specific metrics implementation.
    ///
    /// RocksDB sets this to `DBMetrics`; the in-memory backend uses `()`.
    /// Generic code interacts with metrics exclusively through the
    /// [`StorageMetrics`] trait so that no backend-specific type leaks into
    /// shared infrastructure.
    type Metrics: StorageMetrics;

    /// Return the shared metrics instance for this engine type.
    ///
    /// Implementations typically return a static singleton (e.g.
    /// `DBMetrics::get().clone()`).
    fn get_metrics() -> Arc<Self::Metrics>;

    // -- Factory / builder methods (no `self` receiver) --

    /// Set the lower iteration bound on a `ReadOptions` value (inclusive).
    fn set_iter_lower_bound(opts: &mut Self::ReadOptions, bound: Vec<u8>);

    /// Set the upper iteration bound on a `ReadOptions` value (exclusive).
    fn set_iter_upper_bound(opts: &mut Self::ReadOptions, bound: Vec<u8>);

    /// Construct the default column-family options for this backend.
    /// Used by `unsafe_clear` to recreate a dropped column family.
    fn default_cf_options() -> Self::CfOptions {
        Self::CfOptions::default()
    }

    // -- Point reads --

    fn get<K: AsRef<[u8]>>(
        &self,
        cf_name: &str,
        key: K,
        readopts: &Self::ReadOptions,
    ) -> Result<Option<Self::GetValue<'_>>, TypedStoreError>;

    fn multi_get<I, K>(
        &self,
        cf_name: &str,
        keys: I,
        readopts: &Self::ReadOptions,
    ) -> Vec<Result<Option<Self::GetValue<'_>>, TypedStoreError>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>;

    // -- Point writes --

    fn put(
        &self,
        cf_name: &str,
        key: Vec<u8>,
        value: Vec<u8>,
        writeopts: &Self::WriteOptions,
    ) -> Result<(), TypedStoreError>;

    fn delete(
        &self,
        cf_name: &str,
        key: &[u8],
        writeopts: &Self::WriteOptions,
    ) -> Result<(), TypedStoreError>;

    // -- Batch operations --

    /// Allocate a fresh, empty write batch.
    fn new_batch(&self) -> Self::Batch;

    /// Append a put to an existing batch.
    fn batch_put(
        &self,
        batch: &mut Self::Batch,
        cf_name: &str,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<(), TypedStoreError>;

    /// Append a delete to an existing batch.
    fn batch_delete(
        &self,
        batch: &mut Self::Batch,
        cf_name: &str,
        key: Vec<u8>,
    ) -> Result<(), TypedStoreError>;

    /// Append a range delete to an existing batch.
    /// Default: no-op (not all backends support range deletions).
    fn batch_delete_range(
        &self,
        _batch: &mut Self::Batch,
        _cf_name: &str,
        _from: Vec<u8>,
        _to: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        Ok(())
    }

    /// Approximate byte size of the batch (0 if unsupported).
    fn batch_size_in_bytes(&self, batch: &Self::Batch) -> usize;

    /// Atomically commit a write batch.
    fn write_batch(
        &self,
        batch: Self::Batch,
        writeopts: &Self::WriteOptions,
    ) -> Result<(), TypedStoreError>;

    // -- Column-family management --

    fn create_cf(&self, name: &str, opts: &Self::CfOptions) -> Result<(), TypedStoreError>;

    fn has_cf(&self, name: &str) -> bool;

    fn drop_cf(&self, name: &str) -> Result<(), TypedStoreError>;

    // -- Database-level operations --

    fn flush(&self) -> Result<(), TypedStoreError>;

    fn checkpoint(&self, path: &Path) -> Result<(), TypedStoreError>;

    fn compact_range(&self, cf_name: &str, start: Option<&[u8]>, end: Option<&[u8]>);

    // -- Iterator support --

    /// Open a raw iterator over a column family.
    fn raw_iterator<'a>(&'a self, cf_name: &str, readopts: Self::ReadOptions)
    -> Self::RawIter<'a>;

    // -- Bloom filter hint --

    /// Bloom-filter hint: may have false positives, never false negatives.
    /// Default: `true` (conservative / always performs a real lookup).
    fn key_may_exist(
        &self,
        _cf_name: &str,
        _key: &[u8],
        _readopts: &Self::ReadOptions,
    ) -> bool {
        true
    }

    // -- Secondary / read-only mode --

    /// Attempt to catch up with the primary for secondary instances.
    /// Default: no-op.
    fn try_catch_up_with_primary(&self) -> Result<(), TypedStoreError> {
        Ok(())
    }

    // -- Metrics --

    /// Emit per-column-family Prometheus metrics.
    /// Default: no-op.  RocksDB overrides this.
    fn report_cf_metrics(
        &self,
        _cf_name: &str,
        _db_name: &str,
        _metrics: &Arc<Self::Metrics>,
    ) {
    }
}
