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

mod errors;
mod executor;
pub mod history;
mod metrics;
mod progress_store;
mod reader;
mod reducer;
#[cfg(test)]
mod tests;
mod util;
mod worker_pool;

use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use async_trait::async_trait;
pub use errors::{IngestionError, IngestionResult};
pub use executor::{IndexerExecutor, MAX_CHECKPOINTS_IN_PROGRESS, setup_single_workflow};
use iota_types::full_checkpoint_content::CheckpointData;
pub use metrics::DataIngestionMetrics;
pub use progress_store::{FileProgressStore, ProgressStore, ShimProgressStore};
pub use reader::ReaderOptions;
pub use reducer::Reducer;
pub use util::{create_remote_store_client, create_remote_store_client_with_ops};
pub use worker_pool::WorkerPool;

/// Processes individual checkpoints and produces messages for optional batch
/// processing.
///
/// The Worker trait defines the core processing logic for checkpoint data.
/// Workers run in parallel within a [`WorkerPool`] to process checkpoints and
/// generate messages that can optionally be batched and processed by a
/// [`Reducer`].
///
/// # Processing Modes
///
/// Workers support two processing modes:
/// * **Direct Processing**: Messages are handled immediately without batching.
/// * **Batch Processing**: Messages are accumulated and processed in batches by
///   a [`Reducer`].
///
/// The processing mode is determined by the presence of a [`Reducer`] in the
/// [`WorkerPool`] configuration.
///
/// # Concurrency
///
/// Multiple instances of a worker can run in parallel in the worker pool. The
/// implementation must be thread-safe and handle checkpoint processing
/// efficiently.
///
/// # Integration with Optional Reducer
///
/// Messages produced by the worker can be:
/// * Processed directly without batching.
/// * Accumulated and passed to a [`Reducer`] for batch processing.
///
/// The worker's [`Message`](Worker::Message) type must match the reducer's
/// input type when batch processing is enabled.
#[async_trait]
pub trait Worker: Send + Sync {
    type Error: Debug + Display;
    type Message: Send + Sync;

    /// Processes a single checkpoint and returns a message.
    ///
    /// This method contains the core logic for processing checkpoint data.
    ///
    /// # Note
    /// - Checkpoints are processed in order when a single worker is used.
    /// - Parallel processing with multiple workers does not guarantee
    ///   checkpoint order.
    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error>;

    /// A hook that allows preprocessing a checkpoint before it's fully
    /// processed.
    ///
    /// This method can be used to perform actions like validation or data
    /// transformation before the main
    /// [`process_checkpoint`](Worker::process_checkpoint) logic is executed.
    ///
    /// # Default implementation
    ///
    /// By default it returns `Ok(())`.
    fn preprocess_hook(&self, _: Arc<CheckpointData>) -> Result<(), Self::Error> {
        Ok(())
    }
}
