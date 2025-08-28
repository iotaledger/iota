// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

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
        let blocking_cp = CheckpointHandler::pg_blocking_cp(store.clone()).unwrap();
        let partition_manager = PgPartitionManager::new(blocking_cp.clone())?;
        Ok(Self {
            store,
            partition_manager,
            optimistic_pruner_batch_size,
            metrics,
        })
    }

    pub async fn start(&self, cancel: CancellationToken) -> IndexerResult<()> {
        info!("Starting Optimistic Pruner task...");
        let mut pruning_in_progress = false;
        let mut last_pruned_id = (-1, -1);

        while !cancel.is_cancelled() {
            if !pruning_in_progress {
                // let's not spam the DB if there's no pruning to be done
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            pruning_in_progress = false;

            match self.prune_single_batch(last_pruned_id).await {
                Ok(pruned_to) => {
                    if pruned_to > last_pruned_id {
                        pruning_in_progress = true;
                    }
                    last_pruned_id = pruned_to;
                }
                Err(err) => {
                    warn!("Failed to prune optimistic transaction batch: {err}");
                    continue;
                }
            };
        }

        info!("Optimistic Pruner task cancelled.");
        Ok(())
    }

    async fn prune_single_batch(&self, last_pruned_id: (i64, i64)) -> IndexerResult<(i64, i64)> {
        let current_epoch = self
            .store
            .get_latest_epoch_id_in_blocking_worker()
            .await?
            .unwrap_or(0);
        if current_epoch < EPOCHS_TO_KEEP {
            debug!("No epochs available for pruning");
            return Ok(last_pruned_id);
        }

        let prune_to_epoch = current_epoch.saturating_sub(EPOCHS_TO_KEEP);
        let total_txs = self
            .store
            .get_network_total_transactions_by_end_of_epoch(prune_to_epoch)
            .await?
            .ok_or_else(|| {
                IndexerError::PostgresRead(format!(
                    "No network total transactions found for epoch {prune_to_epoch}"
                ))
            })?;
        let epoch_end_tx = total_txs as i64 - 1;
        let epoch_end_global_order = self
            .store
            .get_global_order_for_tx_seq_in_blocking_worker(epoch_end_tx)
            .await?;
        let prune_to = self
            .store
            .get_nth_smallest_optimistic_tx_global_order_in_blocking_worker(
                self.optimistic_pruner_batch_size - 1,
            )
            .await?;

        debug!(
            "Last tx for epoch {prune_to_epoch} has global order {epoch_end_global_order:?}, \
             next pruning batch ends at {prune_to:?}"
        );

        if let Some(prune_to) = prune_to {
            let prune_to = std::cmp::min(prune_to, epoch_end_global_order);
            if prune_to <= last_pruned_id {
                debug!(
                    "No new optimistic transactions to prune, already pruned to {last_pruned_id:?}"
                );
                return Ok(last_pruned_id);
            }
            let rows_pruned = self
                .store
                .prune_optimistic_transactions_up_to_in_blocking_worker(prune_to)
                .await?;
            self.metrics
                .last_pruned_optimistic_global_seq_num
                .set(prune_to.0);
            info!("Pruned {rows_pruned} optimistic transactions up to global order {prune_to:?}",);
            return Ok(prune_to);
        }

        Ok(last_pruned_id)
    }
}
