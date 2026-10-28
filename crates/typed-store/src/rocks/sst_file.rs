// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{marker::PhantomData, path::Path};

use serde::Serialize;
use typed_store_error::TypedStoreError;

use crate::{
    be_fix_int_ser,
    rocks::errors::{typed_store_err_from_bcs_err, typed_store_err_from_rocks_err},
};

/// Rows of one table, held in memory until [`Self::write_file`] writes them,
/// sorted, to an SST file for [`DBMap::ingest_sst_files`].
///
/// Writing rows this way bypasses the memtables, which RocksDB flushes one at
/// a time per column family, so many threads can each fill their own files
/// at once. Keys and values are encoded as [`DBMap`] encodes them.
///
/// [`DBMap`]: crate::rocks::DBMap
/// [`DBMap::ingest_sst_files`]: crate::rocks::DBMap::ingest_sst_files
pub struct SstFileBuffer<K, V> {
    /// Every row's key followed by its value.
    bytes: Vec<u8>,
    /// Where each row's key starts, where its value starts and where it ends
    /// in `bytes`.
    rows: Vec<(usize, usize, usize)>,
    _phantom: PhantomData<fn(K, V)>,
}

impl<K: Serialize, V: Serialize> SstFileBuffer<K, V> {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            rows: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn insert(&mut self, key: &K, value: &V) -> Result<(), TypedStoreError> {
        let start = self.bytes.len();
        self.bytes.extend_from_slice(&be_fix_int_ser(key));
        let value_start = self.bytes.len();
        bcs::serialize_into(&mut self.bytes, value).map_err(typed_store_err_from_bcs_err)?;
        self.rows.push((start, value_start, self.bytes.len()));
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The memory the buffered rows take.
    pub fn size_in_bytes(&self) -> usize {
        self.bytes.len() + self.rows.len() * size_of::<(usize, usize, usize)>()
    }

    /// Writes the buffered rows, sorted by key, to a new SST file at `path`
    /// and empties the buffer. A key inserted more than once keeps its last
    /// value.
    ///
    /// `options` must be those of the column family the file is ingested
    /// into. The buffer must not be empty, since RocksDB cannot write an SST
    /// file without rows.
    pub fn write_file(
        &mut self,
        path: &Path,
        options: &rocksdb::Options,
    ) -> Result<(), TypedStoreError> {
        let bytes = &self.bytes;
        let key = |&(start, value_start, _): &(usize, usize, usize)| &bytes[start..value_start];
        // Stable, so that of equal keys the last inserted stays last.
        self.rows.sort_by(|a, b| key(a).cmp(key(b)));

        let mut writer = rocksdb::SstFileWriter::create(options);
        writer.open(path).map_err(typed_store_err_from_rocks_err)?;
        for (index, row) in self.rows.iter().enumerate() {
            if self
                .rows
                .get(index + 1)
                .is_some_and(|next| key(next) == key(row))
            {
                continue;
            }
            let (_, value_start, end) = *row;
            writer
                .put(key(row), &bytes[value_start..end])
                .map_err(typed_store_err_from_rocks_err)?;
        }
        writer.finish().map_err(typed_store_err_from_rocks_err)?;

        self.bytes.clear();
        self.rows.clear();
        Ok(())
    }
}

impl<K: Serialize, V: Serialize> Default for SstFileBuffer<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
