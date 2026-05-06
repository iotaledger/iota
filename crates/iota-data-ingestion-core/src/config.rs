// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Configuration options for the ingestion framework.

use std::path::PathBuf;

use crate::{
    IngestionLimit, ReaderOptions,
    filters::fullnode::TransactionFilter,
    reader::v2::{CheckpointReaderConfig, RemoteUrl},
};

/// Configuration options for the ingestion framework.
#[derive(Clone, Default)]
pub struct IngestionConfig {
    /// Config the checkpoint reader behavior for downloading new checkpoints.
    pub(crate) reader_options: ReaderOptions,
    /// Local path for checkpoint ingestion. If not provided, checkpoints will
    /// be ingested from a temporary directory.
    pub(crate) ingestion_path: Option<PathBuf>,
    /// Remote source for checkpoint data stream.
    pub(crate) remote_store_url: Option<RemoteUrl>,
    /// Determines when the ingestion process should stop.
    pub(crate) ingestion_limit: Option<IngestionLimit>,
    /// Filter applied to transactions within a checkpoint.
    pub(crate) fullnode_transaction_filter: Option<TransactionFilter>,
}

impl From<CheckpointReaderConfig> for IngestionConfig {
    fn from(config: CheckpointReaderConfig) -> Self {
        Self {
            reader_options: config.reader_options,
            ingestion_path: config.ingestion_path,
            remote_store_url: config.remote_store_url,
            ..Default::default()
        }
    }
}

impl IngestionConfig {
    pub fn new(reader_options: ReaderOptions) -> Self {
        Self {
            reader_options,
            ..Default::default()
        }
    }

    /// Sets the local path where checkpoints will be ingested from.
    ///
    /// If not provided, checkpoints will be ingested from a temporary
    /// directory.
    pub fn with_ingestion_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.ingestion_path = Some(path.into());
        self
    }

    /// Sets the remote store URL for checkpoint to be downloaded from.
    pub fn with_remote_store_url(mut self, url: RemoteUrl) -> Self {
        self.remote_store_url = Some(url);
        self
    }

    /// Adds an upper‑limit policy that determines when the ingestion
    /// process should stop.
    pub fn with_ingestion_limit(mut self, limit: IngestionLimit) -> Self {
        self.ingestion_limit = Some(limit);
        self
    }

    /// Enables server-side filtering of transactions within each checkpoint.
    ///
    /// When set, the remote source will only return checkpoints containing
    /// transactions that match the provided [`TransactionFilter`].
    ///
    /// ### Connection Requirements
    /// This is only supported for [`RemoteUrl::Fullnode`] connections.
    pub fn with_fullnode_transaction_filter(mut self, filter: TransactionFilter) -> Self {
        self.fullnode_transaction_filter = Some(filter);
        self
    }
}
