// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[non_exhaustive]
#[derive(Error, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
pub enum TypedStoreError {
    #[error("rocksdb error: {0}")]
    RocksDB(String),
    #[error("(de)serialization error: {0}")]
    Serialization(String),
    #[error("column family {0} is not open")]
    UnregisteredColumn(String),
    /// The store declined the operation because the data it names has been
    /// pruned and will not come back. Nothing failed; carries its own
    /// message, since what was pruned and what is still retained is the
    /// store's to explain.
    #[error("{0}")]
    Pruned(String),
    #[error("a batch operation can't operate across databases")]
    CrossDBBatch,
    #[error("Metric reporting thread failed with error")]
    MetricsReporting,
    #[error("Transaction should be retried")]
    RetryableTransaction,
}
