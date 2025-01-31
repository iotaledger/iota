// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This library provides an easy way to create custom indexers.
//! <br>
//!
//! ## Graceful shutdown
//!
//! The shutdown sequence in the data ingestion system ensures clean termination
//! of all components while preserving data integrity. It is initiated via a
//! [CancellationToken](tokio_util::sync::CancellationToken), which triggers a
//! hierarchical and graceful shutdown process.
//!
//! The shutdown process follows a top-down hierarchy:
//! 1. [`Worker`]: Individual workers within a [`WorkerPool`] detect the
//!    cancellation signal, completes current checkpoint processing, sends final
//!    progress updates and signals completion to parent [`WorkerPool`] via
//!    `WorkerStatus::Shutdown` message.
//! 2. [`WorkerPool`]: Coordinates worker shutdowns, ensures all progress
//!    messages are processed, waits for all workers' shutdown signals and
//!    notifies [`IndexerExecutor`] with `WorkerPoolStatus::Shutdown` message
//!    when fully terminated.
//! 3. [`IndexerExecutor`]: Orchestrates the shutdown of all worker pools and
//!    and finalizes system termination.

mod executor;
mod metrics;
mod progress_store;
mod reader;
#[cfg(test)]
mod tests;
mod util;
mod worker_pool;

use anyhow::Result;
use async_trait::async_trait;
pub use executor::{IndexerExecutor, MAX_CHECKPOINTS_IN_PROGRESS, setup_single_workflow};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};
pub use metrics::DataIngestionMetrics;
pub use progress_store::{FileProgressStore, ProgressStore, ShimProgressStore};
pub use reader::ReaderOptions;
pub use util::{create_remote_store_client, create_remote_store_client_with_ops};
pub use worker_pool::WorkerPool;

#[async_trait]
pub trait Worker: Send + Sync {
    async fn process_checkpoint(&self, checkpoint: CheckpointData) -> Result<()>;
    /// Optional method. Allows controlling when workflow progress is updated in
    /// the progress store. For instance, some pipelines may benefit from
    /// aggregating checkpoints, thus skipping the saving of updates for
    /// intermediate checkpoints. The default implementation is to update
    /// the progress store for every processed checkpoint.
    async fn save_progress(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> Option<CheckpointSequenceNumber> {
        Some(sequence_number)
    }

    fn preprocess_hook(&self, _: CheckpointData) -> Result<()> {
        Ok(())
    }
}
