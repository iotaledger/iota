// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::marker::PhantomData;

use serde::de::DeserializeOwned;

use super::RocksDBRawIter;
use crate::TypedStoreError;

/// An iterator over the values of a prefix.
pub struct Values<'a, V> {
    db_iter: RocksDBRawIter<'a>,
    _phantom: PhantomData<V>,
}

impl<'a, V: DeserializeOwned> Values<'a, V> {
    pub(crate) fn new(db_iter: RocksDBRawIter<'a>) -> Self {
        Self {
            db_iter,
            _phantom: PhantomData,
        }
    }
}

impl<'a, V: DeserializeOwned> Iterator for Values<'a, V> {
    type Item = Result<V, TypedStoreError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.db_iter.valid() {
            let value = self
                .db_iter
                .key()
                .and_then(|_| self.db_iter.value().and_then(|v| bcs::from_bytes(v).ok()));

            self.db_iter.next();
            value.map(Ok)
        } else {
            match self.db_iter.status() {
                Ok(_) => None,
                Err(err) => Some(Err(TypedStoreError::RocksDBError(format!("{err}")))),
            }
        }
    }
}
