// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use crate::full_checkpoint_content::CheckpointData;

/// Holds the results a checkpoint carries until the checkpoint executor
/// reaches it, so it can commit them instead of re-executing the
/// transactions.
///
/// Defined here rather than in `iota-core` so that state sync, which cannot
/// depend on `iota-core`, can hand over what it downloaded.
///
/// The results must be committed by the executor rather than written on
/// arrival: object versions enter the writeback cache under a
/// monotonically-increasing invariant, and state sync inserts checkpoints far
/// ahead of execution. Writing there would race the executor's own writes for
/// older checkpoints.
pub trait CacheCheckpointResults: Send + Sync {
    /// Offers a checkpoint's verified results to the executor.
    ///
    /// The caller must already have verified the checkpoint summary's
    /// authority signatures and its `contents_digest`; those are what make the
    /// effects trustworthy.
    ///
    /// Returns `false` when the results were not kept — the cache is at its
    /// size limit — in which case the executor will execute the checkpoint's
    /// transactions as usual. Dropping results is always safe.
    fn cache_checkpoint_results(&self, checkpoint: Arc<CheckpointData>) -> bool;
}
