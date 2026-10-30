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
use prometheus_filtered::{IntGauge, MetricLevel, Registry, register_int_gauge_with_registry};
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
    /// Published so operators can see the cache fill while a node is behind
    /// and drain to zero once it has caught up.
    retained_bytes_gauge: IntGauge,
}

struct Inner {
    /// Keyed by sequence number; the executor consumes in increasing order.
    results: BTreeMap<CheckpointSequenceNumber, Entry>,
    retained_bytes: usize,
}

struct Entry {
    data: Arc<CheckpointData>,
    /// What this entry was charged on admission. Kept so releasing it credits
    /// back the same number, without re-walking its objects under the lock.
    bytes: usize,
}

impl CheckpointResultsCache {
    pub fn new(budget_bytes: usize, registry: &Registry) -> Self {
        Self {
            inner: Mutex::new(Inner {
                results: BTreeMap::new(),
                retained_bytes: 0,
            }),
            budget_bytes,
            retained_bytes_gauge: register_int_gauge_with_registry!(
                "checkpoint_results_cache_retained_bytes",
                "Approximate memory held in checkpoint results downloaded but not yet committed",
                registry;
                MetricLevel::Warn,
            )
            .unwrap(),
        }
    }

    /// What the cache currently counts as retained.
    #[cfg(test)]
    fn retained_bytes_for_testing(&self) -> usize {
        self.inner
            .lock()
            .expect("results cache mutex poisoned")
            .retained_bytes
    }

    /// Takes the results for `sequence_number`, if they were kept.
    pub fn take(&self, sequence_number: CheckpointSequenceNumber) -> Option<Arc<CheckpointData>> {
        let mut inner = self.inner.lock().expect("results cache mutex poisoned");
        let entry = inner.results.remove(&sequence_number)?;
        inner.retained_bytes = inner.retained_bytes.saturating_sub(entry.bytes);
        self.retained_bytes_gauge.set(inner.retained_bytes as i64);
        Some(entry.data)
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
            let bytes = entry.get().bytes;
            entry.remove();
            inner.retained_bytes = inner.retained_bytes.saturating_sub(bytes);
        }
        self.retained_bytes_gauge.set(inner.retained_bytes as i64);
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

        // State sync retries a failed batch, which offers its checkpoints
        // again. The copy being replaced is credited back before the budget is
        // tested, so a re-offer is weighed as the replacement it is rather than
        // as a second copy.
        if let Some(replaced) = inner.results.remove(&sequence_number) {
            inner.retained_bytes = inner.retained_bytes.saturating_sub(replaced.bytes);
        }

        if inner.retained_bytes + bytes > self.budget_bytes {
            debug!(
                ?sequence_number,
                retained_bytes = inner.retained_bytes,
                "declining checkpoint results: the cache is at its size limit"
            );
            self.retained_bytes_gauge.set(inner.retained_bytes as i64);
            return false;
        }

        inner.retained_bytes += bytes;
        inner.results.insert(
            sequence_number,
            Entry {
                data: checkpoint,
                bytes,
            },
        );
        self.retained_bytes_gauge.set(inner.retained_bytes as i64);
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
    /// Covers a transaction, its effects and its events, none of which carry
    /// a size of their own.
    const BYTES_PER_TRANSACTION: usize = 2048;

    checkpoint
        .transactions
        .iter()
        .map(|tx| {
            // Measured rather than averaged: an object ranges from a few
            // hundred bytes to the move object and package size limits, so an
            // average would let an object-heavy range run far past the budget.
            let objects: usize = tx
                .input_objects
                .iter()
                .chain(tx.output_objects.iter())
                .map(|object| object.object_size_for_gas_metering())
                .sum();
            BYTES_PER_TRANSACTION + objects
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

#[cfg(test)]
mod tests {
    use iota_types::{
        full_checkpoint_content::CheckpointData,
        test_checkpoint_data_builder::TestCheckpointDataBuilder,
    };
    use prometheus_filtered::Registry;

    use super::*;

    fn checkpoint(sequence_number: u64) -> Arc<CheckpointData> {
        Arc::new(
            TestCheckpointDataBuilder::new(sequence_number)
                .start_transaction(0)
                .finish_transaction()
                .build_checkpoint(),
        )
    }

    /// State sync retries a failed batch, so the same checkpoint is offered
    /// again. The second offer replaces the first rather than being charged
    /// on top of it; otherwise the budget leaks until nothing is accepted.
    #[test]
    fn offering_the_same_checkpoint_twice_charges_for_it_once() {
        let cache = CheckpointResultsCache::new(1 << 30, &Registry::new());

        assert!(cache.cache_checkpoint_results(checkpoint(1)));
        let after_first = cache.retained_bytes_for_testing();
        assert!(cache.cache_checkpoint_results(checkpoint(1)));

        assert_eq!(
            cache.retained_bytes_for_testing(),
            after_first,
            "a re-offer of the same checkpoint must not be charged twice"
        );
    }

    /// Taking the results back out releases what they were charged.
    #[test]
    fn taking_results_releases_their_bytes() {
        let cache = CheckpointResultsCache::new(1 << 30, &Registry::new());

        assert!(cache.cache_checkpoint_results(checkpoint(1)));
        assert!(cache.take(1).is_some());

        assert_eq!(cache.retained_bytes_for_testing(), 0);
    }
}
