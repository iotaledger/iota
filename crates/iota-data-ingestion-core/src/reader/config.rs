// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Configuration types for the checkpoint reader.
//!
//! The two main types are:
//!
//! - [`CheckpointReaderConfig`]: the base configuration for the checkpoint
//!   reader. Suited for most default use cases.
//! - [`CheckpointReaderConfigExt`]: extends [`CheckpointReaderConfig`] with
//!   opt-in configuration toggles beyond the base, such as server-side
//!   transaction filters for fullnode connections.
//!
//! [`CheckpointReaderConfigExt`] wraps [`CheckpointReaderConfig`] and exposes
//! a builder for the extra toggles. A `From<CheckpointReaderConfig>` impl is
//! provided so existing [`CheckpointReaderConfig`] values can be converted
//! to [`CheckpointReaderConfigExt`].

use std::path::PathBuf;

use crate::filters::fullnode::TransactionFilter;
pub use crate::{
    ReaderOptions,
    reader::v2::{CheckpointReaderConfig, RemoteUrl},
};

/// Extends [`CheckpointReaderConfig`] with opt-in configuration toggles.
///
/// Use this type when you need framework features beyond what the base
/// [`CheckpointReaderConfig`] exposes (e.g. server-side transaction filtering
/// for fullnode connections).
///
/// # Example
///
/// ```rust
/// use iota_data_ingestion_core::{
///     ReaderOptions,
///     filters::fullnode::{ExecutionStatusFilter, TransactionFilter},
///     reader::config::{CheckpointReaderConfig, CheckpointReaderConfigExt, RemoteUrl},
/// };
///
/// let filter = TransactionFilter::default()
///     .with_execution_status(ExecutionStatusFilter::default().with_success(true));
///
/// let config = CheckpointReaderConfigExt::new(ReaderOptions::default())
///     .with_remote_store_url(RemoteUrl::Fullnode("http://127.0.0.1:50051".into()))
///     .with_fullnode_transaction_filter(filter);
/// ```
/// # Example with an existing [`CheckpointReaderConfig`]
///
/// ```rust
/// use iota_data_ingestion_core::{
///     ReaderOptions,
///     filters::fullnode::{ExecutionStatusFilter, TransactionFilter},
///     reader::config::{CheckpointReaderConfig, CheckpointReaderConfigExt, RemoteUrl},
/// };
///
/// let base_config = CheckpointReaderConfig {
///     reader_options: ReaderOptions::default(),
///     remote_store_url: Some(RemoteUrl::Fullnode("http://127.0.0.1:50051".into())),
///     ..Default::default()
/// };
///
/// let filter = TransactionFilter::default()
///     .with_execution_status(ExecutionStatusFilter::default().with_success(true));
/// let config =
///     CheckpointReaderConfigExt::from(base_config).with_fullnode_transaction_filter(filter);
/// ```
#[derive(Clone, Default)]
pub struct CheckpointReaderConfigExt {
    /// The base configuration for the checkpoint reader.
    pub(crate) base: CheckpointReaderConfig,
    /// Filter applied to transactions within a checkpoint.
    pub(crate) fullnode_transaction_filter: Option<TransactionFilter>,
}

impl From<CheckpointReaderConfig> for CheckpointReaderConfigExt {
    fn from(config: CheckpointReaderConfig) -> Self {
        Self {
            base: config,
            ..Default::default()
        }
    }
}

impl CheckpointReaderConfigExt {
    /// Constructs a new configuration with the given [`ReaderOptions`].
    pub fn new(reader_options: ReaderOptions) -> Self {
        Self {
            base: CheckpointReaderConfig {
                reader_options,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Sets the local path where checkpoints will be ingested from.
    ///
    /// If not provided, checkpoints will be ingested from a temporary
    /// directory.
    pub fn with_ingestion_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.base.ingestion_path = Some(path.into());
        self
    }

    /// Sets the remote source from which checkpoints are downloaded.
    pub fn with_remote_store_url(mut self, url: RemoteUrl) -> Self {
        self.base.remote_store_url = Some(url);
        self
    }

    /// Enables server-side filtering of transactions within each checkpoint.
    ///
    /// When set, the remote source will only return checkpoints containing
    /// transactions that match the provided [`TransactionFilter`].
    ///
    /// # Errors
    ///
    /// Using this filter with any source other than [`RemoteUrl::Fullnode`]
    /// will cause the executor to return an error when started.
    pub fn with_fullnode_transaction_filter(mut self, filter: TransactionFilter) -> Self {
        self.fullnode_transaction_filter = Some(filter);
        self
    }
}
