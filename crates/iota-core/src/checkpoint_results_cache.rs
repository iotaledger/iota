// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use iota_sdk_types::CheckpointSequenceNumber;
use iota_types::{
    full_checkpoint_content::CheckpointData, storage::CacheCheckpointResults,
    transaction::TransactionAPI,
};
use tracing::debug;

/// Holds verified checkpoint results between state sync downloading them and
/// the checkpoint executor reaching that checkpoint.
///
/// State sync runs ahead of execution, so this bounds what it keeps by an
/// estimate of the retained bytes. Once the budget is used up further results
/// are declined and their checkpoints are executed normally, which is always
/// correct.
pub struct CheckpointResultsCache {
    inner: Mutex<Inner>,
    budget_bytes: usize,
}

struct Inner {
    /// Keyed by sequence number; the executor consumes in increasing order.
    results: BTreeMap<CheckpointSequenceNumber, Arc<CheckpointData>>,
    retained_bytes: usize,
}

impl CheckpointResultsCache {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                results: BTreeMap::new(),
                retained_bytes: 0,
            }),
            budget_bytes,
        }
    }

    /// Takes the results for `sequence_number`, if they were kept.
    pub fn take(&self, sequence_number: CheckpointSequenceNumber) -> Option<Arc<CheckpointData>> {
        let mut inner = self.inner.lock().expect("results cache mutex poisoned");
        let data = inner.results.remove(&sequence_number)?;
        inner.retained_bytes = inner.retained_bytes.saturating_sub(estimate_bytes(&data));
        Some(data)
    }

    /// Drops everything below `sequence_number`.
    ///
    /// Results the executor passed without taking are unreachable: it consumes
    /// in increasing order and never revisits a checkpoint.
    pub fn forget_below(&self, sequence_number: CheckpointSequenceNumber) {
        let mut inner = self.inner.lock().expect("results cache mutex poisoned");
        while let Some(entry) = inner.results.first_entry() {
            if *entry.key() >= sequence_number {
                break;
            }
            let bytes = estimate_bytes(entry.get());
            entry.remove();
            inner.retained_bytes = inner.retained_bytes.saturating_sub(bytes);
        }
    }

    pub fn retained_bytes(&self) -> usize {
        self.inner
            .lock()
            .expect("results cache mutex poisoned")
            .retained_bytes
    }
}

impl CacheCheckpointResults for CheckpointResultsCache {
    fn cache_checkpoint_results(&self, checkpoint: Arc<CheckpointData>) -> bool {
        let sequence_number = checkpoint.checkpoint_summary.sequence_number;
        let bytes = estimate_bytes(&checkpoint);
        let mut inner = self.inner.lock().expect("results cache mutex poisoned");
        if inner.retained_bytes + bytes > self.budget_bytes {
            debug!(
                ?sequence_number,
                retained_bytes = inner.retained_bytes,
                "declining checkpoint results: the cache is at its size limit"
            );
            return false;
        }
        inner.retained_bytes += bytes;
        inner.results.insert(sequence_number, checkpoint);
        true
    }
}

/// Approximates the memory a checkpoint's results occupy.
///
/// Serialising every object to measure it exactly would cost more than the
/// bound is worth, so this counts objects and transactions against a flat
/// per-item size. It only has to be close enough to keep the cache from
/// growing without limit.
fn estimate_bytes(checkpoint: &CheckpointData) -> usize {
    /// Rough average size of a stored object, from the objects table's bytes
    /// per key on a mainnet-scale node.
    const BYTES_PER_OBJECT: usize = 512;
    /// Covers a transaction, its effects and its events.
    const BYTES_PER_TRANSACTION: usize = 2048;

    checkpoint
        .transactions
        .iter()
        .map(|tx| {
            BYTES_PER_TRANSACTION
                + (tx.input_objects.len() + tx.output_objects.len()) * BYTES_PER_OBJECT
        })
        .sum()
}

/// Whether this transaction's results may be committed from checkpoint data.
///
/// The end-of-epoch transaction drives reconfiguration and always executes.
pub fn may_commit_from_checkpoint(
    tx: &iota_types::full_checkpoint_content::CheckpointTransaction,
) -> bool {
    !tx.transaction.transaction().is_end_of_epoch_tx()
}
