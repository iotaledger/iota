// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains the error types and result types used in the indexer streaming
//! crate.

use iota_indexer::errors::IndexerError;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub type IndexerStreamingResult<T> = std::result::Result<T, IndexerStreamingError>;

#[derive(thiserror::Error, Debug, Clone)]
pub enum IndexerStreamingError {
    #[error("postgres error: {0}")]
    Postgres(String),
    #[error("streaming data processor error: {0}")]
    StreamingDataProcessor(String),
    #[error("indexer error: {0}")]
    Indexer(String),
    #[error(transparent)]
    Lagged(#[from] BroadcastStreamRecvError),
}

impl From<tokio_postgres::Error> for IndexerStreamingError {
    fn from(error: tokio_postgres::Error) -> Self {
        IndexerStreamingError::Postgres(error.to_string())
    }
}

impl From<IndexerError> for IndexerStreamingError {
    fn from(error: IndexerError) -> Self {
        IndexerStreamingError::Indexer(error.to_string())
    }
}
