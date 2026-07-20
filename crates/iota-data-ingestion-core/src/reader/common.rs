// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module shares common types and logic for the checkpoint reader.

use std::collections::BTreeMap;

use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
};

/// Options for configuring how the checkpoint reader fetches new checkpoints.
#[derive(Clone)]
pub struct ReaderOptions {
    /// How often to check for new checkpoints, lower values mean faster
    /// detection but more CPU usage.
    ///
    /// Default: 100ms.
    pub tick_interval_ms: u64,
    /// Network request timeout, it applies to remote store operations.
    ///
    /// Default: 5 seconds.
    pub timeout_secs: u64,
    /// Number of maximum concurrent requests to the remote store. Increase it
    /// for backfills, higher values increase throughput but use more resources.
    ///
    /// Default: 10.
    pub batch_size: usize,
    /// Maximum memory (bytes) for batch checkpoint processing to prevent OOM
    /// errors. Zero indicates no limit.
    ///
    /// Default: 0.
    pub data_limit: usize,
}

impl Default for ReaderOptions {
    fn default() -> Self {
        Self {
            tick_interval_ms: 100,
            timeout_secs: 5,
            batch_size: 10,
            data_limit: 0,
        }
    }
}

/// Tracks and limits the total in-progress data size for checkpoint processing.
///
/// `DataLimiter` is used to prevent excessive memory usage by keeping track of
/// the cumulative size of checkpoints currently being processed. It maintains a
/// queue of checkpoint sequence numbers and their associated sizes, and
/// provides methods to check if the limit is exceeded, add new checkpoints, and
/// perform garbage collection of processed checkpoints.
pub struct DataLimiter {
    /// The maximum allowed in-progress data size (in bytes). Zero means no
    /// limit.
    limit: usize,
    /// A mapping from checkpoint sequence number to its data size (in bytes)
    queue: BTreeMap<CheckpointSequenceNumber, usize>,
    /// The current total in-progress data size (in bytes).
    in_progress: usize,
}

impl DataLimiter {
    /// Creates a new `DataLimiter` with the specified memory limit (in bytes).
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            queue: BTreeMap::new(),
            in_progress: 0,
        }
    }

    /// Returns `true` if the current in-progress data size exceeds the
    /// configured limit.
    pub fn exceeds(&self) -> bool {
        self.limit > 0 && self.in_progress >= self.limit
    }

    /// Adds a checkpoint's data size to the in-progress queue.
    pub fn add(&mut self, checkpoint: &CheckpointData, size: usize) {
        if self.limit == 0 {
            return;
        }
        self.in_progress += size;
        self.queue
            .insert(checkpoint.checkpoint_summary.sequence_number, size);
    }

    /// Performs garbage collection by removing all checkpoints with a sequence
    /// number less than the given `watermark`, and recalculates the total
    /// in-progress size.
    pub fn gc(&mut self, watermark: CheckpointSequenceNumber) {
        if self.limit == 0 {
            return;
        }
        self.queue = self.queue.split_off(&watermark);
        self.in_progress = self.queue.values().sum();
    }
}
