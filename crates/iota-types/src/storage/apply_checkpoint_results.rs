// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{full_checkpoint_content::CheckpointData, storage::error::Error as StorageError};

/// Commits the results a checkpoint carries, so its transactions do not have to
/// be executed again to reproduce them.
///
/// Defined here rather than in `iota-core` so that state sync, which cannot
/// depend on `iota-core`, can drive it.
pub trait ApplyCheckpointResults {
    /// Verifies the checkpoint's payloads against the digests its effects
    /// record and writes the results to the store.
    ///
    /// The caller must already have verified the checkpoint summary's
    /// authority signatures and its `contents_digest`; those are what make the
    /// effects — and so the digests checked here — trustworthy.
    ///
    /// Returns `false` when the results were left for the checkpoint executor
    /// to produce by executing the transactions, which is always a valid
    /// outcome: applying is an optimisation, and executing is the fallback.
    ///
    /// Implementations must write all of a checkpoint's results or none of
    /// them.
    fn try_apply_checkpoint_results(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<bool, StorageError>;
}
