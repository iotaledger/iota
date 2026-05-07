// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tracing::info;

use crate::{
    authority::authority_store_tables::AuthorityPerpetualTables, checkpoints::CheckpointStore,
};

/// Shared progress tracker for checkpoint operations. Updated by the
/// checkpoint executor and pruner, periodically logs a one-line summary.
///
/// Passed as `Option<Arc<CheckpointProgressTracker>>` — callers that don't need
/// progress reporting (CLI tools, tests) simply pass `None`.
///
/// All values are accumulated and reset on each logging tick.
pub struct CheckpointProgressTracker {
    /// Accumulated checkpoint execution time in nanoseconds.
    execution_time_ns: AtomicU64,
    /// Accumulated object pruning time in nanoseconds.
    object_pruning_time_ns: AtomicU64,
    /// Accumulated checkpoint/effects pruning time in nanoseconds.
    checkpoint_pruning_time_ns: AtomicU64,
}

impl CheckpointProgressTracker {
    pub fn new() -> Self {
        Self {
            execution_time_ns: AtomicU64::new(0),
            object_pruning_time_ns: AtomicU64::new(0),
            checkpoint_pruning_time_ns: AtomicU64::new(0),
        }
    }

    pub fn add_execution_time(&self, duration: Duration) {
        self.execution_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    pub fn add_object_pruning_time(&self, duration: Duration) {
        self.object_pruning_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    pub fn add_checkpoint_pruning_time(&self, duration: Duration) {
        self.checkpoint_pruning_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Spawns a periodic logging task that prints a one-line checkpoint
    /// progress summary every second (only when there is actual progress).
    pub fn spawn_logging_task(
        self: &Arc<Self>,
        checkpoint_store: Arc<CheckpointStore>,
        perpetual_db: Arc<AuthorityPerpetualTables>,
    ) {
        let tracker = self.clone();
        tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            let mut prev_executed: u64 = 0;
            let mut prev_total_tx: u64 = 0;
            let mut prev_obj_pruned: u64 = 0;
            let mut prev_ckpt_pruned: u64 = 0;

            loop {
                interval.tick().await;

                let highest_executed_checkpoint = checkpoint_store
                    .get_highest_executed_checkpoint()
                    .ok()
                    .flatten();
                let epoch = highest_executed_checkpoint
                    .as_ref()
                    .map(|c| c.epoch())
                    .unwrap_or(0);
                let highest_executed_seq_number = highest_executed_checkpoint
                    .as_ref()
                    .map(|c| *c.sequence_number())
                    .unwrap_or(0);
                let total_tx = highest_executed_checkpoint
                    .as_ref()
                    .map(|c| c.network_total_transactions)
                    .unwrap_or(0);
                let synced_seq_number = checkpoint_store
                    .get_highest_synced_checkpoint_seq_number()
                    .ok()
                    .flatten()
                    .unwrap_or(0);
                let object_pruned_seq_number = perpetual_db
                    .get_highest_pruned_checkpoint()
                    .ok()
                    .flatten()
                    .unwrap_or(0);
                let checkpoint_pruned_seq_number = checkpoint_store
                    .get_highest_pruned_checkpoint_seq_number()
                    .ok()
                    .flatten()
                    .unwrap_or(0);

                let exec_delta = highest_executed_seq_number.saturating_sub(prev_executed);
                let tx_delta = total_tx.saturating_sub(prev_total_tx);
                let object_prune_delta = object_pruned_seq_number.saturating_sub(prev_obj_pruned);
                let checkpoint_prune_delta =
                    checkpoint_pruned_seq_number.saturating_sub(prev_ckpt_pruned);

                if exec_delta > 0
                    || tx_delta > 0
                    || object_prune_delta > 0
                    || checkpoint_prune_delta > 0
                {
                    let exec_time_delta_ns = tracker.execution_time_ns.swap(0, Ordering::Relaxed);
                    let exec_time_delta = Duration::from_nanos(exec_time_delta_ns);

                    let object_prune_time_delta_ns =
                        tracker.object_pruning_time_ns.swap(0, Ordering::Relaxed);
                    let object_prune_time_delta = Duration::from_nanos(object_prune_time_delta_ns);

                    let checkpoint_prune_time_delta_ns = tracker
                        .checkpoint_pruning_time_ns
                        .swap(0, Ordering::Relaxed);
                    let checkpoint_prune_time_delta =
                        Duration::from_nanos(checkpoint_prune_time_delta_ns);

                    info!(
                        "checkpoint progress [epoch {epoch}]: executed {highest_executed_seq_number}/{synced_seq_number} (+{exec_delta}, {tx_delta} tx/s, {exec_time_delta:.2?}), \
                         objects pruned {object_pruned_seq_number} (+{object_prune_delta}, {object_prune_time_delta:.2?}), \
                         checkpoints pruned {checkpoint_pruned_seq_number} (+{checkpoint_prune_delta}, {checkpoint_prune_time_delta:.2?})",
                    );

                    prev_executed = highest_executed_seq_number;
                    prev_total_tx = total_tx;
                    prev_obj_pruned = object_pruned_seq_number;
                    prev_ckpt_pruned = checkpoint_pruned_seq_number;
                }
            }
        });
    }
}

impl Default for CheckpointProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}
