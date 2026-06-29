// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{borrow::Borrow, error::Error, ops::RangeBounds};

use serde::{Serialize, de::DeserializeOwned};

use crate::TypedStoreError;

pub type DbIterator<'a, T> = Box<dyn Iterator<Item = Result<T, TypedStoreError>> + 'a>;

pub trait Map<'a, K, V>
where
    K: Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned,
{
    type Error: Error;

    /// Returns true if the map contains a value for the specified key.
    fn contains_key(&self, key: &K) -> Result<bool, Self::Error>;

    /// Returns true if the map contains a value for the specified key.
    fn multi_contains_keys<J>(
        &self,
        keys: impl IntoIterator<Item = J>,
    ) -> Result<Vec<bool>, Self::Error>
    where
        J: Borrow<K>,
    {
        keys.into_iter()
            .map(|key| self.contains_key(key.borrow()))
            .collect()
    }

    /// Returns the value for the given key from the map, if it exists.
    fn get(&self, key: &K) -> Result<Option<V>, Self::Error>;

    /// Inserts the given key-value pair into the map.
    fn insert(&self, key: &K, value: &V) -> Result<(), Self::Error>;

    /// Removes the entry for the given key from the map.
    fn remove(&self, key: &K) -> Result<(), Self::Error>;

    /// Uses delete range on the entire key range
    fn schedule_delete_all(&self) -> Result<(), TypedStoreError>;

    /// Returns true if the map is empty, otherwise false.
    fn is_empty(&self) -> bool;

    /// Iterates over all entries in key order.
    fn safe_iter(&'a self) -> DbIterator<'a, (K, V)>;

    /// Iterates over the half-open key range `[lower_bound, upper_bound)` —
    /// the lower bound is inclusive, the upper bound exclusive, and a `None`
    /// bound leaves that side unbounded. Equivalent to
    /// `safe_range_iter(lower_bound..upper_bound)`.
    fn safe_iter_with_bounds(
        &'a self,
        lower_bound: Option<K>,
        upper_bound: Option<K>,
    ) -> DbIterator<'a, (K, V)>;

    /// Iterates over the keys within `range`, honoring the range's own bound
    /// inclusivity (e.g. `lo..hi` excludes `hi`, `lo..=hi` includes it).
    fn safe_range_iter(&'a self, range: impl RangeBounds<K>) -> DbIterator<'a, (K, V)>;

    /// Returns a vector of values corresponding to the keys provided,
    /// non-atomically.
    fn multi_get<J>(&self, keys: impl IntoIterator<Item = J>) -> Result<Vec<Option<V>>, Self::Error>
    where
        J: Borrow<K>,
    {
        keys.into_iter().map(|key| self.get(key.borrow())).collect()
    }

    /// Inserts key-value pairs, non-atomically.
    fn multi_insert<J, U>(
        &self,
        key_val_pairs: impl IntoIterator<Item = (J, U)>,
    ) -> Result<(), Self::Error>
    where
        J: Borrow<K>,
        U: Borrow<V>,
    {
        key_val_pairs
            .into_iter()
            .try_for_each(|(key, value)| self.insert(key.borrow(), value.borrow()))
    }

    /// Removes keys, non-atomically.
    fn multi_remove<J>(&self, keys: impl IntoIterator<Item = J>) -> Result<(), Self::Error>
    where
        J: Borrow<K>,
    {
        keys.into_iter()
            .try_for_each(|key| self.remove(key.borrow()))
    }

    /// Try to catch up with primary when running as secondary
    fn try_catch_up_with_primary(&self) -> Result<(), Self::Error>;
}

pub struct TableSummary {
    pub num_keys: u64,
    pub key_bytes_total: usize,
    pub value_bytes_total: usize,
    pub key_hist: hdrhistogram::Histogram<u64>,
    pub value_hist: hdrhistogram::Histogram<u64>,
}
