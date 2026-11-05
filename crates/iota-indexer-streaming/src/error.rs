// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Contains the error types and result types used in the indexer streaming
//! crate.

use iota_indexer::errors::IndexerError;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub type IndexerStreamingResult<T> = std::result::Result<T, IndexerStreamingError>;

#[derive(thiserror::Error, Debug, Clone)]
pub enum IndexerStreamingError {
    #[error("postgres error")]
    Postgres,
    #[error("streaming data processor error: {0}")]
    StreamingDataProcessor(String),
    #[error("indexer error: {0}")]
    Indexer(String),
    #[error(transparent)]
    Lagged(#[from] BroadcastStreamRecvError),
    #[error("not found: {0}")]
    NotFound(String),
}

impl From<diesel::result::Error> for IndexerStreamingError {
    fn from(error: diesel::result::Error) -> Self {
        tracing::error!("postgres error: {error:?}");
        IndexerStreamingError::Postgres
    }
}

impl From<IndexerError> for IndexerStreamingError {
    fn from(error: IndexerError) -> Self {
        IndexerStreamingError::Indexer(error.to_string())
    }
}
