// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{marker::PhantomData, sync::Arc};

use bincode::Options;
use prometheus::{Histogram, HistogramTimer};
use serde::de::DeserializeOwned;
use typed_store_error::TypedStoreError;

use super::{
    RawIter,
    metrics::{DBMetrics, RocksDBPerfContext},
};

enum IterDirection {
    Forward,
    Reverse,
}

/// A typed, forward-or-reverse iterator over key-value pairs in a RocksDB
/// column family.
pub struct SafeIter<'a, K, V> {
    cf_name: String,
    db_iter: RawIter<'a>,
    _phantom: PhantomData<(K, V)>,
    direction: IterDirection,
    is_initialized: bool,
    _timer: Option<HistogramTimer>,
    _perf_ctx: Option<RocksDBPerfContext>,
    bytes_scanned: Option<Histogram>,
    keys_scanned: Option<Histogram>,
    db_metrics: Option<Arc<DBMetrics>>,
    bytes_scanned_counter: usize,
    keys_returned_counter: usize,
}

impl<'a, K: DeserializeOwned, V: DeserializeOwned> SafeIter<'a, K, V> {
    pub(crate) fn new(
        cf_name: String,
        db_iter: RawIter<'a>,
        _timer: Option<HistogramTimer>,
        _perf_ctx: Option<RocksDBPerfContext>,
        bytes_scanned: Option<Histogram>,
        keys_scanned: Option<Histogram>,
        db_metrics: Option<Arc<DBMetrics>>,
    ) -> Self {
        Self {
            cf_name,
            db_iter,
            _phantom: PhantomData,
            direction: IterDirection::Forward,
            is_initialized: false,
            _timer,
            _perf_ctx,
            bytes_scanned,
            keys_scanned,
            db_metrics,
            bytes_scanned_counter: 0,
            keys_returned_counter: 0,
        }
    }
}

impl<K: DeserializeOwned, V: DeserializeOwned> Iterator for SafeIter<'_, K, V> {
    type Item = Result<(K, V), TypedStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.is_initialized {
            self.db_iter.seek_to_first();
            self.is_initialized = true;
        }
        if self.db_iter.valid() {
            let config = bincode::DefaultOptions::new()
                .with_big_endian()
                .with_fixint_encoding();
            let raw_key = self
                .db_iter
                .key()
                .expect("Valid iterator failed to get key");
            let raw_value = self
                .db_iter
                .value()
                .expect("Valid iterator failed to get value");
            self.bytes_scanned_counter += raw_key.len() + raw_value.len();
            self.keys_returned_counter += 1;
            let key = config.deserialize(raw_key).ok();
            let value = bcs::from_bytes(raw_value).ok();
            match self.direction {
                IterDirection::Forward => self.db_iter.next(),
                IterDirection::Reverse => self.db_iter.prev(),
            }
            key.and_then(|k| value.map(|v| Ok((k, v))))
        } else {
            match self.db_iter.status() {
                Ok(_) => None,
                Err(err) => Some(Err(TypedStoreError::RocksDB(format!("{err}")))),
            }
        }
    }
}

impl<K, V> Drop for SafeIter<'_, K, V> {
    fn drop(&mut self) {
        if let Some(bytes_scanned) = self.bytes_scanned.take() {
            bytes_scanned.observe(self.bytes_scanned_counter as f64);
        }
        if let Some(keys_scanned) = self.keys_scanned.take() {
            keys_scanned.observe(self.keys_returned_counter as f64);
        }
        if let Some(db_metrics) = self.db_metrics.take() {
            db_metrics.report_read_perf_ctx(&self.cf_name);
        }
    }
}

/// An iterator with reversed direction. Wraps a `SafeIter` configured to
/// traverse in reverse order.
pub struct SafeRevIter<'a, K, V> {
    iter: SafeIter<'a, K, V>,
}

impl<'a, K, V> SafeRevIter<'a, K, V> {
    pub(crate) fn new(mut iter: SafeIter<'a, K, V>, upper_bound: Option<Vec<u8>>) -> Self {
        iter.is_initialized = true;
        iter.direction = IterDirection::Reverse;
        match upper_bound {
            None => iter.db_iter.seek_to_last(),
            Some(ref key) => iter.db_iter.seek_for_prev(key),
        }
        Self { iter }
    }
}

impl<K: DeserializeOwned, V: DeserializeOwned> Iterator for SafeRevIter<'_, K, V> {
    type Item = Result<(K, V), TypedStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
