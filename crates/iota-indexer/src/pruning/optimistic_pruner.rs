// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{
    errors::IndexerError,
    ingestion::primary::prepare::PrimaryWorker,
    metrics::IndexerMetrics,
    store::{
        IndexerStore, PgIndexerStore, TxGlobalOrderCursor, pg_partition_manager::PgPartitionManager,
    },
    types::IndexerResult,
};

// Keeping current and previous epoch ensures we will not prune just executed
// txs on epoch boundary
const EPOCHS_TO_KEEP: u64 = 2;

// short epoch change detection time for testing
#[cfg(any(feature = "shared_test_runtime", feature = "pg_integration"))]
const CHECK_EPOCH_CHANGE_INTERVAL: Duration = Duration::from_secs(3);

// longer epoch change detection time for production builds
#[cfg(not(any(feature = "shared_test_runtime", feature = "pg_integration")))]
const CHECK_EPOCH_CHANGE_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour

const DELAY_BETWEEN_BATCHES: Duration = Duration::from_millis(200);

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
        let blocking_cp = PrimaryWorker::pg_blocking_cp(store.clone())?;
        let partition_manager = PgPartitionManager::new(blocking_cp)?;
        Ok(Self {
            store,
            partition_manager,
            optimistic_pruner_batch_size,
            metrics,
        })
    }

    pub async fn start(&self, cancel: CancellationToken) {
        info!("Starting Optimistic Pruner task...");
        let mut last_processed_epoch = None;

        while !cancel.is_cancelled() {
            tokio::time::sleep(CHECK_EPOCH_CHANGE_INTERVAL).await;

            let current_epoch = match self.get_current_epoch().await {
                Ok(epoch) => epoch,
                Err(err) => {
                    warn!("failed to get current epoch: {err}");
                    continue;
                }
            };

            if last_processed_epoch != Some(current_epoch) {
                info!("Epoch change detected: {last_processed_epoch:?} -> {current_epoch}");

                if let Err(err) = self.prune_up_to_epoch(current_epoch).await {
                    warn!("failed to prune up to epoch {current_epoch}: {err}");
                    continue;
                }

                last_processed_epoch = Some(current_epoch);
            }
        }

        info!("Optimistic Pruner task cancelled.");
    }

    /// Prunes optimistic transactions up to the given epoch if needed.
    /// Returns Ok(()) if pruning was completed or skipped successfully.
    async fn prune_up_to_epoch(&self, current_epoch: u64) -> IndexerResult<()> {
        match self.get_pruning_threshold(current_epoch).await? {
            Some(epoch_end_global_order) => {
                info!(
                    "Starting pruning for epoch {current_epoch} up to global order {epoch_end_global_order:?}"
                );

                self.prune_in_batches(epoch_end_global_order).await?;
            }
            None => {
                info!("No pruning needed for epoch {current_epoch}");
            }
        }
        Ok(())
    }

    /// Prunes optimistic transactions in batches until there are no more rows
    /// to delete.
    async fn prune_in_batches(
        &self,
        epoch_end_global_order: TxGlobalOrderCursor,
    ) -> IndexerResult<()> {
        loop {
            let rows_deleted = self.prune_single_batch(epoch_end_global_order).await?;
            if rows_deleted == 0 {
                info!(
                    "Finished pruning optimistic transactions in batches up to {epoch_end_global_order:?}"
                );
                break;
            }
            // brief pause to relieve the I/O pressure on the DB
            tokio::time::sleep(DELAY_BETWEEN_BATCHES).await;
        }
        Ok(())
    }

    /// Gets the current epoch from the database.
    async fn get_current_epoch(&self) -> IndexerResult<u64> {
        Ok(self
            .store
            .get_latest_epoch_id_in_blocking_worker()
            .await?
            .unwrap_or(0))
    }

    /// Calculates the pruning threshold for a given epoch.
    /// Returns None if no pruning should occur for this epoch.
    async fn get_pruning_threshold(
        &self,
        current_epoch: u64,
    ) -> IndexerResult<Option<TxGlobalOrderCursor>> {
        if current_epoch < EPOCHS_TO_KEEP {
            info!("No epochs available for optimistic pruning");
            return Ok(None);
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

        Ok(Some(epoch_end_global_order))
    }

    /// Deletes a single batch of optimistic transactions up to the given global
    /// order. Returns the number of deleted rows.
    async fn prune_single_batch(
        &self,
        epoch_end_global_order: TxGlobalOrderCursor,
    ) -> IndexerResult<u64> {
        let whole_batch_timer = self.metrics.optimistic_pruner_batch_duration.start_timer();

        let rows_pruned = self
            .store
            .prune_optimistic_transactions_up_to_in_blocking_worker(
                epoch_end_global_order,
                self.optimistic_pruner_batch_size as i64,
            )
            .await?;

        self.metrics
            .optimistic_pruner_total_rows_pruned
            .inc_by(rows_pruned as u64);

        let elapsed = whole_batch_timer.stop_and_record();
        info!(
            "Pruned {rows_pruned} optimistic transactions with limit at {epoch_end_global_order:?} in {elapsed:?} seconds"
        );

        Ok(rows_pruned as u64)
    }
}
