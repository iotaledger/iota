// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use super::checkpoint_handler::CheckpointHandler;
use crate::{
    errors::IndexerError,
    metrics::IndexerMetrics,
    store::{IndexerStore, PgIndexerStore, pg_partition_manager::PgPartitionManager},
    types::IndexerResult,
};

pub struct Pruner {
    pub store: PgIndexerStore,
    pub partition_manager: PgPartitionManager,
    pub epochs_to_keep: u64,
    pub metrics: IndexerMetrics,
}

impl Pruner {
    pub fn new(
        store: PgIndexerStore,
        epochs_to_keep: u64,
        metrics: IndexerMetrics,
    ) -> Result<Self, IndexerError> {
        let blocking_cp = CheckpointHandler::pg_blocking_cp(store.clone()).unwrap();
        let partition_manager = PgPartitionManager::new(blocking_cp.clone())?;
        Ok(Self {
            store,
            partition_manager,
            epochs_to_keep,
            metrics,
        })
    }

    pub async fn start(&self, cancel: CancellationToken) -> IndexerResult<()> {
        let mut last_seen_max_epoch = 0;
        // The first epoch that has not yet been pruned.
        let mut next_prune_epoch = None;
        while !cancel.is_cancelled() {
            let (min_epoch, max_epoch) = self.store.get_available_epoch_range().await?;
            if max_epoch == last_seen_max_epoch {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            last_seen_max_epoch = max_epoch;

            // Not all partitioned tables are epoch-partitioned, so we need to filter them
            // out.
            let table_partitions: HashMap<_, _> = self
                .partition_manager
                .get_table_partitions()?
                .into_iter()
                .filter(|(table_name, _)| {
                    self.partition_manager
                        .get_strategy(table_name)
                        .is_epoch_partitioned()
                })
                .collect();

            let prune_to_epoch = last_seen_max_epoch.saturating_sub(self.epochs_to_keep - 1);

            for (table_name, (min_partition, max_partition)) in &table_partitions {
                if last_seen_max_epoch != *max_partition {
                    error!(
                        "Epochs are out of sync for table {table_name}: max_epoch={last_seen_max_epoch}, max_partition={max_partition}",
                    );
                }
                for epoch in *min_partition..prune_to_epoch {
                    if cancel.is_cancelled() {
                        info!("Pruner task cancelled.");
                        return Ok(());
                    }
                    self.partition_manager
                        .drop_table_partition(table_name.clone(), epoch)?;
                    info!(
                        "Batch dropped table partition {} epoch {}",
                        table_name, epoch
                    );
                }
            }

            let prune_start_epoch = next_prune_epoch.unwrap_or(min_epoch);

            for epoch in prune_start_epoch..prune_to_epoch {
                if cancel.is_cancelled() {
                    info!("Pruner task cancelled.");
                    return Ok(());
                }
                info!("Pruning epoch {}", epoch);
                if let Err(err) = self.store.prune_epoch(epoch).await {
                    error!("Failed to prune epoch {epoch}: {err}");
                    break;
                };
                self.metrics.last_pruned_epoch.set(epoch as i64);
                info!("Pruned epoch {}", epoch);
                next_prune_epoch = Some(epoch + 1);
            }
        }
        info!("Pruner task cancelled.");
        Ok(())
    }
}
