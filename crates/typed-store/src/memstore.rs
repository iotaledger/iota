// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, Bound, HashMap},
    sync::{Arc, RwLock},
};

use bincode::Options;
use serde::de::DeserializeOwned;
use typed_store_error::TypedStoreError;

use crate::{DbIterator, database::error_iterator};

type ColumnFamily = BTreeMap<Vec<u8>, Vec<u8>>;
type ColumnFamilies = HashMap<String, ColumnFamily>;
type InMemoryStoreInternal = Arc<RwLock<ColumnFamilies>>;

fn column_family<'a>(
    data: &'a ColumnFamilies,
    cf_name: &str,
) -> Result<&'a ColumnFamily, TypedStoreError> {
    data.get(cf_name)
        .ok_or_else(|| TypedStoreError::UnregisteredColumn(cf_name.to_string()))
}

fn column_family_mut<'a>(
    data: &'a mut ColumnFamilies,
    cf_name: &str,
) -> Result<&'a mut ColumnFamily, TypedStoreError> {
    data.get_mut(cf_name)
        .ok_or_else(|| TypedStoreError::UnregisteredColumn(cf_name.to_string()))
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryDB {
    data: InMemoryStoreInternal,
}

#[derive(Clone, Debug)]
enum InMemoryChange {
    Delete((String, Vec<u8>)),
    Put((String, Vec<u8>, Vec<u8>)),
}

impl InMemoryChange {
    fn column_family(&self) -> &str {
        match self {
            InMemoryChange::Delete((cf_name, _)) | InMemoryChange::Put((cf_name, _, _)) => cf_name,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryBatch {
    data: Vec<InMemoryChange>,
}

impl InMemoryBatch {
    pub fn delete_cf<K: AsRef<[u8]>>(&mut self, cf_name: &str, key: K) {
        self.data.push(InMemoryChange::Delete((
            cf_name.to_string(),
            key.as_ref().to_vec(),
        )));
    }

    pub fn put_cf<K, V>(&mut self, cf_name: &str, key: K, value: V)
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
    pub fn get<K: AsRef<[u8]>>(
        &self,
        cf_name: &str,
        key: K,
    ) -> Result<Option<Vec<u8>>, TypedStoreError> {
        let data = self.data.read().expect("can't read data");
        Ok(column_family(&data, cf_name)?.get(key.as_ref()).cloned())
    }

    pub fn multi_get<I, K>(
        &self,
        cf_name: &str,
        keys: I,
    ) -> Vec<Result<Option<Vec<u8>>, TypedStoreError>>
    where
        I: IntoIterator<Item = K>,
        K: AsRef<[u8]>,
    {
        let data = self.data.read().expect("can't read data");
        let cf = column_family(&data, cf_name);
        // One slot per key, even when the column family is missing: a shorter
        // result would silently misalign with the keys.
        keys.into_iter()
            .map(|k| Ok(cf.as_ref().map_err(Clone::clone)?.get(k.as_ref()).cloned()))
            .collect()
    }

    pub fn delete(&self, cf_name: &str, key: &[u8]) -> Result<(), TypedStoreError> {
        let mut data = self.data.write().expect("can't write data");
        column_family_mut(&mut data, cf_name)?.remove(key);
        Ok(())
    }

    pub fn put(&self, cf_name: &str, key: Vec<u8>, value: Vec<u8>) -> Result<(), TypedStoreError> {
        let mut data = self.data.write().expect("can't write data");
        column_family_mut(&mut data, cf_name)?.insert(key, value);
        Ok(())
    }

    pub fn write(&self, batch: InMemoryBatch) -> Result<(), TypedStoreError> {
        let mut data = self.data.write().expect("can't write data");
        // Every column family is checked before anything is applied, so a
        // batch naming a missing one leaves the store untouched.
        for change in &batch.data {
            column_family(&data, change.column_family())?;
        }
        for change in batch.data {
            match change {
                InMemoryChange::Delete((cf_name, key)) => {
                    column_family_mut(&mut data, &cf_name)?.remove(&key);
                }
                InMemoryChange::Put((cf_name, key, value)) => {
                    column_family_mut(&mut data, &cf_name)?.insert(key, value);
                }
            }
        }
        Ok(())
    }

    /// Creates a new column family. Fails if a column family with this name
    /// already exists, matching the RocksDB backend.
    pub fn create_cf(&self, name: &str) -> Result<(), TypedStoreError> {
        match self
            .data
            .write()
            .expect("can't write data")
            .entry(name.to_string())
        {
            std::collections::hash_map::Entry::Occupied(_) => Err(TypedStoreError::RocksDB(
                format!("column family {name} already exists"),
            )),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(BTreeMap::new());
                Ok(())
            }
        }
    }

    pub fn has_cf(&self, name: &str) -> bool {
        self.data
            .read()
            .expect("can't read data")
            .contains_key(name)
    }

    pub fn drop_cf(&self, name: &str) {
        self.data.write().expect("can't write data").remove(name);
    }

    pub fn iterator<'a, K, V>(
        &'a self,
        cf_name: &str,
        lower_bound: Option<Vec<u8>>,
        upper_bound: Option<Vec<u8>>,
        reverse: bool,
    ) -> DbIterator<'a, (K, V)>
    where
        K: DeserializeOwned + 'a,
        V: DeserializeOwned + 'a,
    {
        let config = bincode::DefaultOptions::new()
            .with_big_endian()
            .with_fixint_encoding();
        // `BTreeMap::range` panics on an inverted range where RocksDB scans
        // nothing; normalize to an empty scan.
        let inverted = matches!(
            (&lower_bound, &upper_bound),
            (Some(lower), Some(upper)) if lower > upper
        );
        let lower_bound = lower_bound.map(Bound::Included).unwrap_or(Bound::Unbounded);
        let upper_bound = upper_bound.map(Bound::Excluded).unwrap_or(Bound::Unbounded);

        let data = self.data.read().expect("can't read data");
        let cf = match column_family(&data, cf_name) {
            Ok(cf) => cf,
            Err(e) => return error_iterator(e),
        };
        let mut section: Vec<_> = if inverted {
            Vec::new()
        } else {
            cf.range((lower_bound, upper_bound))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };
        if reverse {
            section.reverse();
        }
        Box::new(section.into_iter().map(move |(raw_key, raw_value)| {
            let key = config
                .deserialize(&raw_key)
                .map_err(|e| TypedStoreError::Serialization(e.to_string()))?;
            let value = bcs::from_bytes(&raw_value)
                .map_err(|e| TypedStoreError::Serialization(e.to_string()))?;
            Ok((key, value))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_cf_fails_on_an_existing_column_family() {
        let db = InMemoryDB::default();
        db.create_cf("cf").unwrap();
        assert!(db.create_cf("cf").is_err());
    }

    #[test]
    fn multi_get_returns_one_slot_per_key() {
        let db = InMemoryDB::default();
        db.create_cf("cf").unwrap();
        db.put("cf", vec![1], vec![10]).unwrap();

        // Duplicate and absent keys each keep their own slot, in order.
        let values = db
            .multi_get("cf", [vec![1u8], vec![2], vec![1]])
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(values, vec![Some(vec![10]), None, Some(vec![10])]);

        // A missing column family fails every slot rather than shortening
        // the result.
        let results = db.multi_get("missing", [vec![1u8], vec![2]]);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(Result::is_err));
    }

    #[test]
    fn inverted_range_scans_nothing() {
        let db = InMemoryDB::default();
        db.create_cf("cf").unwrap();
        assert!(
            db.iterator::<Vec<u8>, Vec<u8>>("cf", Some(vec![2]), Some(vec![1]), false)
                .next()
                .is_none()
        );
    }

    #[test]
    fn operations_on_a_missing_column_family_report_an_error() {
        let db = InMemoryDB::default();
        let unregistered = |op: &str, error: TypedStoreError| {
            assert!(
                matches!(&error, TypedStoreError::UnregisteredColumn(cf) if cf == "cf"),
                "{op}: unexpected error: {error}"
            );
        };

        unregistered("get", db.get("cf", b"key").unwrap_err());
        unregistered("put", db.put("cf", vec![1], vec![1]).unwrap_err());
        unregistered("delete", db.delete("cf", &[1u8]).unwrap_err());
        unregistered(
            "iterator",
            db.iterator::<Vec<u8>, Vec<u8>>("cf", None, None, false)
                .next()
                .expect("the scan should yield an item")
                .unwrap_err(),
        );

        // None of them created the column family on the way.
        assert!(!db.has_cf("cf"));
    }

    #[test]
    fn a_batch_naming_a_missing_column_family_applies_nothing() {
        let db = InMemoryDB::default();
        db.create_cf("kept").unwrap();

        let mut batch = InMemoryBatch::default();
        batch.put_cf("kept", vec![1], vec![1]);
        batch.put_cf("missing", vec![1], vec![1]);

        assert!(db.write(batch).is_err());
        assert_eq!(db.get("kept", [1]).unwrap(), None);
        assert!(!db.has_cf("missing"));
    }
}
