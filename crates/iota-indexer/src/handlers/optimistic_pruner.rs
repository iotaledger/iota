// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::checkpoint_handler::CheckpointHandler;
use crate::{
    errors::IndexerError,
    metrics::IndexerMetrics,
    store::{IndexerStore, PgIndexerStore, pg_partition_manager::PgPartitionManager},
    types::IndexerResult,
};

// Keeping current and previous epoch ensures we will not prune just executed
// txs on epoch boundary
const EPOCHS_TO_KEEP: u64 = 2;
const PRUNING_NOT_IN_PROGRESS_DELAY: Duration = Duration::from_secs(3);
const DELETE_ROWS_DELAY: Duration = Duration::from_millis(200);

pub struct OptimisticPruner {
    pub store: PgIndexerStore,
    pub partition_manager: PgPartitionManager,
    pub optimistic_pruner_batch_size: u64,
    pub metrics: IndexerMetrics,
}

impl OptimisticPruner {
    pub fn new(
        store: PgIndexerStore,
        optimistic_pruner_batch_size: u64,
        metrics: IndexerMetrics,
    ) -> Result<Self, IndexerError> {
        let blocking_cp = CheckpointHandler::pg_blocking_cp(store.clone())?;
        let partition_manager = PgPartitionManager::new(blocking_cp.clone())?;
        Ok(Self {
            store,
            partition_manager,
            optimistic_pruner_batch_size,
            metrics,
        })
    }

    pub async fn start(&self, cancel: CancellationToken) {
        info!("Starting Optimistic Pruner task...");
        let mut pruning_in_progress = false;

        while !cancel.is_cancelled() {
            if !pruning_in_progress {
                // let's not spam the DB if there's no pruning to be done
                tokio::time::sleep(PRUNING_NOT_IN_PROGRESS_DELAY).await;
            }

            match self.prune_single_batch().await {
                Ok(pruning_occured) => {
                    pruning_in_progress = pruning_occured;
                }
                Err(err) => {
                    pruning_in_progress = false;
                    warn!("Failed to prune optimistic transaction batch: {err}");
                    continue;
                }
            };
        }

        info!("Optimistic Pruner task cancelled.");
    }

    async fn prune_single_batch(&self) -> IndexerResult<bool> {
        let whole_batch_timer = self.metrics.optimistic_pruner_batch_duration.start_timer();

        let current_epoch = self
            .store
            .get_latest_epoch_id_in_blocking_worker()
            .await?
            .unwrap_or(0);
        if current_epoch < EPOCHS_TO_KEEP {
            info!("No epochs available for optimistic pruning");
            return Ok(false);
        }

        let prune_to_epoch = current_epoch.saturating_sub(EPOCHS_TO_KEEP);
        let total_txs = self
            .store
            .get_network_total_transactions_by_end_of_epoch(prune_to_epoch)
            .await?
            .ok_or_else(|| {
                IndexerError::PostgresRead(format!(
                    "no network total transactions found for epoch {prune_to_epoch}"
                ))
            })?;
        let epoch_end_tx = total_txs as i64 - 1;
        let epoch_end_global_order = self
            .store
            .get_global_order_for_tx_seq_in_blocking_worker(epoch_end_tx)
            .await?;

        let rows_pruned = {
            let _delete_timer = self
                .metrics
                .optimistic_pruner_delete_query_duration
                .start_timer();
            self.store
                .prune_optimistic_transactions_up_to_in_blocking_worker(
                    epoch_end_global_order,
                    self.optimistic_pruner_batch_size as i64,
                )
                .await?
        };
        self.metrics
            .optimistic_pruner_total_rows_pruned
            .inc_by(rows_pruned as u64);
        let elapsed = whole_batch_timer.stop_and_record();
        info!(
            "Pruned {rows_pruned} optimistic transactions with limit at {epoch_end_global_order:?} in {elapsed:?} seconds"
        );

        // brief pause to give DB time to vacuum deleted rows
        tokio::time::sleep(DELETE_ROWS_DELAY).await;
        Ok(rows_pruned > 0)
    }
}
