// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
    sync::{Arc, RwLock},
};

use typed_store_error::TypedStoreError;

use crate::engine::{RawDbIterator, StorageEngine};

type InMemoryStoreInternal = Arc<RwLock<HashMap<String, BTreeMap<Vec<u8>, Vec<u8>>>>>;

#[derive(Clone, Debug, Default)]
pub(crate) struct InMemoryDB {
    data: InMemoryStoreInternal,
}

#[derive(Clone, Debug)]
pub(crate) enum InMemoryChange {
    Delete((String, Vec<u8>)),
    Put((String, Vec<u8>, Vec<u8>)),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct InMemoryBatch {
    data: Vec<InMemoryChange>,
}

impl InMemoryBatch {
    pub(crate) fn delete_cf<K: AsRef<[u8]>>(&mut self, cf_name: &str, key: K) {
        self.data.push(InMemoryChange::Delete((
            cf_name.to_string(),
            key.as_ref().to_vec(),
        )));
    }

    pub(crate) fn put_cf<K, V>(&mut self, cf_name: &str, key: K, value: V)
    where
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
    {
        self.data.push(InMemoryChange::Put((
            cf_name.to_string(),
            key.as_ref().to_vec(),
            value.as_ref().to_vec(),
        )));
    }
}

impl InMemoryDB {
    pub(crate) fn get<K: AsRef<[u8]>>(&self, cf_name: &str, key: K) -> Option<Vec<u8>> {
        let data = self.data.read().expect("can't read data");
        match data.get(cf_name) {
            Some(cf) => cf.get(key.as_ref()).cloned(),
            None => None,
        }
    }

    pub(crate) fn multi_get<I, K>(&self, cf_name: &str, keys: I) -> Vec<Option<Vec<u8>>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let data = self.data.read().expect("can't read data");
        match data.get(cf_name) {
            Some(cf) => keys
                .into_iter()
                .map(|k| cf.get(k.as_ref()).cloned())
                .collect(),
            None => vec![],
        }
    }

    pub(crate) fn delete(&self, cf_name: &str, key: &[u8]) {
        let mut data = self.data.write().expect("can't write data");
        data.entry(cf_name.to_string()).or_default().remove(key);
    }

    pub(crate) fn put(&self, cf_name: &str, key: Vec<u8>, value: Vec<u8>) {
        let mut data = self.data.write().expect("can't write data");
        data.entry(cf_name.to_string())
            .or_default()
            .insert(key, value);
    }

    pub(crate) fn write(&self, batch: InMemoryBatch) {
        for change in batch.data {
            match change {
                InMemoryChange::Delete((cf_name, key)) => self.delete(&cf_name, &key),
                InMemoryChange::Put((cf_name, key, value)) => self.put(&cf_name, key, value),
            }
        }
    }

    pub(crate) fn contains_cf(&self, name: &str) -> bool {
        self.data
            .read()
            .expect("can't read data")
            .contains_key(name)
    }

    pub(crate) fn drop_cf(&self, name: &str) {
        self.data.write().expect("can't write data").remove(name);
    }

    pub(crate) fn create_cf(&self, name: &str) {
        self.data
            .write()
            .expect("can't write data")
            .entry(name.to_string())
            .or_default();
    }
}

// ---------------------------------------------------------------------------
// NeverIter — stub raw iterator for backends that do not support iteration
// ---------------------------------------------------------------------------

pub(crate) struct NeverIter;

impl RawDbIterator for NeverIter {
    fn seek_to_first(&mut self) {
        unimplemented!("iteration not supported for the in-memory backend")
    }

    fn seek_to_last(&mut self) {
        unimplemented!("iteration not supported for the in-memory backend")
    }

    fn seek(&mut self, _key: &[u8]) {
        unimplemented!("iteration not supported for the in-memory backend")
    }

    fn seek_for_prev(&mut self, _key: &[u8]) {
        unimplemented!("iteration not supported for the in-memory backend")
    }

    fn valid(&self) -> bool {
        false
    }

    fn key(&self) -> Option<&[u8]> {
        None
    }

    fn value(&self) -> Option<&[u8]> {
        None
    }

    fn next(&mut self) {}

    fn prev(&mut self) {}

    fn status(&self) -> Result<(), TypedStoreError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// impl StorageEngine for InMemoryDB
// ---------------------------------------------------------------------------

impl StorageEngine for InMemoryDB {
    type Batch = InMemoryBatch;
    type GetValue<'a>
        = Vec<u8>
    where
        Self: 'a;
    /// The in-memory backend ignores read options; `()` is a zero-cost token.
    type ReadOptions = ();
    /// The in-memory backend ignores write options; `()` is a zero-cost token.
    type WriteOptions = ();
    /// The in-memory backend ignores CF options; `()` is a zero-cost token.
    type CfOptions = ();
    type RawIter<'a>
        = NeverIter
    where
        Self: 'a;
    /// The in-memory backend has no metrics; `()` is the no-op implementation.
    type Metrics = ();

    fn get_metrics() -> Arc<()> {
        Arc::new(())
    }

    fn set_iter_lower_bound(_opts: &mut (), _bound: Vec<u8>) {}
    fn set_iter_upper_bound(_opts: &mut (), _bound: Vec<u8>) {}

    fn get<K: AsRef<[u8]>>(
        &self,
        cf_name: &str,
        key: K,
        _readopts: &(),
    ) -> Result<Option<Vec<u8>>, TypedStoreError> {
        Ok(InMemoryDB::get(self, cf_name, key))
    }

    fn multi_get<I, K>(
        &self,
        cf_name: &str,
        keys: I,
        _readopts: &(),
    ) -> Vec<Result<Option<Vec<u8>>, TypedStoreError>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        InMemoryDB::multi_get(self, cf_name, keys)
            .into_iter()
            .map(Ok)
            .collect()
    }

    fn put(
        &self,
        cf_name: &str,
        key: Vec<u8>,
        value: Vec<u8>,
        _writeopts: &(),
    ) -> Result<(), TypedStoreError> {
        InMemoryDB::put(self, cf_name, key, value);
        Ok(())
    }

    fn delete(&self, cf_name: &str, key: &[u8], _writeopts: &()) -> Result<(), TypedStoreError> {
        InMemoryDB::delete(self, cf_name, key);
        Ok(())
    }

    fn new_batch(&self) -> InMemoryBatch {
        InMemoryBatch::default()
    }

    fn batch_put(
        &self,
        batch: &mut InMemoryBatch,
        cf_name: &str,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        batch.put_cf(cf_name, key, value);
        Ok(())
    }

    fn batch_delete(
        &self,
        batch: &mut InMemoryBatch,
        cf_name: &str,
        key: Vec<u8>,
    ) -> Result<(), TypedStoreError> {
        batch.delete_cf(cf_name, key);
        Ok(())
    }

    // batch_delete_range: uses the default no-op implementation.

    fn batch_size_in_bytes(&self, _batch: &InMemoryBatch) -> usize {
        0
    }

    fn write_batch(&self, batch: InMemoryBatch, _writeopts: &()) -> Result<(), TypedStoreError> {
        InMemoryDB::write(self, batch);
        Ok(())
    }

    fn create_cf(&self, name: &str, _opts: &()) -> Result<(), TypedStoreError> {
        InMemoryDB::create_cf(self, name);
        Ok(())
    }

    fn has_cf(&self, name: &str) -> bool {
        InMemoryDB::contains_cf(self, name)
    }

    fn drop_cf(&self, name: &str) -> Result<(), TypedStoreError> {
        InMemoryDB::drop_cf(self, name);
        Ok(())
    }

    fn flush(&self) -> Result<(), TypedStoreError> {
        Ok(())
    }

    fn checkpoint(&self, _path: &Path) -> Result<(), TypedStoreError> {
        Ok(())
    }

    fn compact_range(&self, _cf_name: &str, _start: Option<&[u8]>, _end: Option<&[u8]>) {}

    fn raw_iterator<'a>(&'a self, _cf_name: &str, _readopts: ()) -> NeverIter {
        NeverIter
    }

    // key_may_exist, try_catch_up_with_primary, report_cf_metrics: all use defaults.
}
