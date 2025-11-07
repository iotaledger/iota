// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use rocksdb::Error as RocksError;
use typed_store_error::TypedStoreError;

pub fn typed_store_err_from_bincode_err(err: bincode::Error) -> TypedStoreError {
    TypedStoreError::Serialization(format!("{err}"))
}

pub fn typed_store_err_from_bcs_err(err: bcs::Error) -> TypedStoreError {
    TypedStoreError::Serialization(format!("{err}"))
}

pub fn typed_store_err_from_rocks_err(err: RocksError) -> TypedStoreError {
    TypedStoreError::RocksDB(format!("{err}"))
}
