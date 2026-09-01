// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_types::{
    committee::EpochId,
    executable_transaction::VerifiedExecutableTransaction,
    full_checkpoint_content::CheckpointData,
    storage::{ApplyCheckpointResults, error::Error as StorageError},
    transaction::{SenderSignedTransactionAPI, TransactionAPI, VerifiedTransaction},
};
use tracing::debug;

use crate::{
    authority::AuthorityState, execution_cache::ExecutionCacheWrite,
    transaction_outputs::TransactionOutputs,
};

/// Writes the results a checkpoint carries directly to the store, so state sync
/// does not have to wait for the checkpoint executor to reproduce them by
/// executing the transactions.
pub struct CheckpointResultsApplier {
    state: Arc<AuthorityState>,
    cache_writer: Arc<dyn ExecutionCacheWrite>,
}

impl CheckpointResultsApplier {
    pub fn new(state: Arc<AuthorityState>, cache_writer: Arc<dyn ExecutionCacheWrite>) -> Self {
        Self {
            state,
            cache_writer,
        }
    }
}

#[async_trait::async_trait]
impl ApplyCheckpointResults for CheckpointResultsApplier {
    async fn wait_for_epoch(&self, epoch: EpochId) {
        loop {
            // Cloned out of the guard so it is not held across the await.
            let epoch_store = self.state.load_epoch_store_one_call_per_task().clone();
            let current = epoch_store.epoch();
            if current >= epoch {
                return;
            }
            debug!(
                current_epoch = current,
                waiting_for = epoch,
                "pausing archive sync until the node reaches the epoch of the checkpoints it \
                 is applying"
            );
            epoch_store.wait_epoch_terminated().await;
        }
    }

    fn try_apply_checkpoint_results(
        &self,
        checkpoint: &CheckpointData,
    ) -> Result<bool, StorageError> {
        let sequence_number = checkpoint.checkpoint_summary.sequence_number;
        // The summary's own epoch, not `Envelope::epoch()`, which reports the
        // epoch the signatures were made in. The two agree on a verified
        // checkpoint, but it is the summary's epoch that decides which epoch's
        // marker tables these writes belong in.
        let checkpoint_epoch = checkpoint.checkpoint_summary.data().epoch;
        let epoch_store = self.state.load_epoch_store_one_call_per_task();

        // Object markers and shared version assignments are stored per epoch, so
        // a checkpoint's results can only be written while its own epoch is the
        // current one. Callers wait for the epoch with `wait_for_epoch`, so this
        // normally holds; it can still fail for a checkpoint from an epoch the
        // node has already left, which cannot be applied at all.
        if checkpoint_epoch != epoch_store.epoch() {
            debug!(
                ?sequence_number,
                checkpoint_epoch,
                current_epoch = epoch_store.epoch(),
                "leaving checkpoint results to the executor: the checkpoint's epoch is not current"
            );
            return Ok(false);
        }

        // Verify before writing anything, so a checkpoint that fails leaves the
        // store untouched and the executor can still produce the results itself.
        checkpoint.verify_payload_digests()?;

        for tx in &checkpoint.transactions {
            // Reconfiguration stays on the execution path: it is one transaction
            // per epoch and it drives the epoch change itself.
            if tx.transaction.transaction().is_end_of_epoch_tx() {
                continue;
            }

            // Execution assigns shared object versions for every transaction it
            // schedules. Without the same call here the epoch's
            // `next_shared_object_versions` rows would stop advancing.
            let executable = VerifiedExecutableTransaction::new_from_checkpoint(
                VerifiedTransaction::new_unchecked(tx.transaction.clone()),
                checkpoint_epoch,
                sequence_number,
            );
            if executable.contains_shared_object() {
                epoch_store
                    .acquire_shared_version_assignments_from_effects(
                        &executable,
                        &tx.effects,
                        self.state.get_object_cache_reader().as_ref(),
                    )
                    .map_err(StorageError::custom)?;
            }

            let outputs = TransactionOutputs::build_from_checkpoint_transaction(tx);
            self.cache_writer
                .try_write_transaction_outputs(checkpoint_epoch, Arc::new(outputs))
                .map_err(StorageError::custom)?;
        }

        Ok(true)
    }
}

#[cfg(test)]
#[path = "unit_tests/checkpoint_results_applier_tests.rs"]
mod checkpoint_results_applier_tests;
