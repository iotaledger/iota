// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    config::RetentionConfig,
    errors::IndexerError,
    ingestion::primary::prepare::PrimaryWorker,
    metrics::IndexerMetrics,
    models::watermarks::StoredWatermark,
    spawn_monitored_task,
    store::{IndexerStore, PgIndexerStore, pg_partition_manager::PgPartitionManager},
    types::IndexerResult,
};

const UPDATE_WATERMARKS_LOWER_BOUNDS_TASK_INTERVAL: Duration = Duration::from_secs(5);

/// Interval for running the pruning task
const PRUNING_TASK_INTERVAL: Duration = Duration::from_secs(5);

/// Delay between pruning chunks to relieve I/O pressure on the database
const DELAY_BETWEEN_PRUNING_CHUNKS: Duration = Duration::from_millis(100);

pub struct Pruner {
    pub store: PgIndexerStore,
    pub partition_manager: PgPartitionManager,
    pub retention_policies: HashMap<PrunableTable, u64>,
    pub pruning_delay_ms: u64,
    pub pruning_batch_size: u64,
    pub metrics: IndexerMetrics,
}

/// Enum representing tables that the pruner is allowed to prune. This
/// corresponds to table names in the database, and should be used in lieu of
/// string literals. This enum is also meant to facilitate the process of
/// determining which unit (epoch, cp, or tx) should be used for the
/// table's range. Pruner will ignore any table that is not listed here.
#[derive(
    Debug,
    Eq,
    PartialEq,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    strum_macros::AsRefStr,
    Hash,
    Serialize,
    Deserialize,
    Clone,
    Copy,
)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrunableTable {
    Transactions,
    Events,

    EventEmitPackage,
    EventEmitModule,
    EventSenders,
    EventStructInstantiation,
    EventStructModule,
    EventStructName,
    EventStructPackage,

    TxCallsPkg,
    TxCallsMod,
    TxCallsFun,
    TxChangedObjects,
    TxDigests,
    TxInputObjects,
    TxKinds,
    TxRecipients,
    TxSenders,
    TxWrappedOrDeletedObjects,
    TxGlobalOrder,

    Checkpoints,
    PrunerCpWatermark,
    OptimisticTransactions,
    ObjectsBackwardHistory,
}

/// Represents how a table is pruned
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PruningStrategy {
    /// Drop partition by epoch number (for epoch-partitioned tables)
    ByEpochPartition,
    /// Delete rows by checkpoint number
    ByCheckpoint,
    /// Delete rows by transaction sequence number
    ByTransaction,
    /// Delete rows by global sequence number
    ByGlobalSeq,
    /// Delete rows by checkpoint range with a per-statement row limit.
    /// Used for tables with variable rows per checkpoint (e.g. backward
    /// history).
    ByCheckpointWithLimit,
}

impl PruningStrategy {
    /// Whether chunk sizing for this strategy uses the configurable pruning
    /// batch size. `ByEpochPartition` is not batched — partitions are dropped
    /// one at a time.
    fn is_batched(&self) -> bool {
        match self {
            Self::ByEpochPartition => false,
            Self::ByCheckpoint
            | Self::ByCheckpointWithLimit
            | Self::ByTransaction
            | Self::ByGlobalSeq => true,
        }
    }

    /// Exclusive upper bound of the pruning range for this strategy, taken
    /// from the watermark's `min_available_*` columns.
    fn range_end(&self, watermark: &StoredWatermark) -> u64 {
        match self {
            Self::ByEpochPartition => watermark.min_available_epoch as u64,
            Self::ByCheckpoint | Self::ByCheckpointWithLimit => watermark.min_available_cp as u64,
            Self::ByTransaction | Self::ByGlobalSeq => watermark.min_available_tx as u64,
        }
    }
}

impl PrunableTable {
    /// Returns the pruning strategy for this table
    pub fn pruning_strategy(&self) -> PruningStrategy {
        match self {
            // Epoch-partitioned tables - pruned by dropping partitions
            // transactions: partitioned by TxSequenceNumber
            // events: partitioned by TxSequenceNumber
            PrunableTable::Transactions | PrunableTable::Events => {
                PruningStrategy::ByEpochPartition
            }

            // Checkpoint-based tables (not partitioned) - pruned by DELETE
            PrunableTable::Checkpoints | PrunableTable::PrunerCpWatermark => {
                PruningStrategy::ByCheckpoint
            }

            // Transaction-based index tables (not partitioned) - pruned by DELETE
            PrunableTable::EventEmitPackage
            | PrunableTable::EventEmitModule
            | PrunableTable::EventSenders
            | PrunableTable::EventStructInstantiation
            | PrunableTable::EventStructModule
            | PrunableTable::EventStructName
            | PrunableTable::EventStructPackage
            | PrunableTable::TxCallsPkg
            | PrunableTable::TxCallsMod
            | PrunableTable::TxCallsFun
            | PrunableTable::TxChangedObjects
            | PrunableTable::TxDigests
            | PrunableTable::TxInputObjects
            | PrunableTable::TxKinds
            | PrunableTable::TxRecipients
            | PrunableTable::TxSenders
            | PrunableTable::TxWrappedOrDeletedObjects
            | PrunableTable::TxGlobalOrder => PruningStrategy::ByTransaction,

            // Optimistic transactions table - pruned by global sequence number
            PrunableTable::OptimisticTransactions => PruningStrategy::ByGlobalSeq,

            // Backward history - pruned by checkpoint with row limit
            PrunableTable::ObjectsBackwardHistory => PruningStrategy::ByCheckpointWithLimit,
        }
    }
}

/// Executes pruning operations for a specific table
pub struct TablePruner<'a> {
    table: PrunableTable,
    store: &'a PgIndexerStore,
    partition_manager: &'a PgPartitionManager,
    pruning_delay_ms: u64,
    pruning_batch_size: u64,
    #[allow(dead_code)]
    metrics: &'a IndexerMetrics,
    cancel: CancellationToken,
}

impl<'a> TablePruner<'a> {
    pub fn new(
        table: PrunableTable,
        store: &'a PgIndexerStore,
        partition_manager: &'a PgPartitionManager,
        pruning_delay_ms: u64,
        pruning_batch_size: u64,
        metrics: &'a IndexerMetrics,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            table,
            store,
            partition_manager,
            pruning_delay_ms,
            pruning_batch_size,
            metrics,
            cancel,
        }
    }

    /// Runs the persistent pruning task for this executor's table
    pub async fn run_pruning_task(self) -> IndexerResult<()> {
        info!(
            "Starting persistent pruning task for table {}",
            self.table.as_ref()
        );

        let mut interval = tokio::time::interval(PRUNING_TASK_INTERVAL);

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Pruning task for table {} cancelled.", self.table.as_ref());
                    return Ok(());
                }
                _ = interval.tick() => {
                    if let Err(e) = self.check_and_prune().await {
                        error!("error pruning table {}: {e}", self.table.as_ref());
                    }
                }
            }
        }
    }

    /// Checks if pruning is needed and prunes data for this executor's table if
    /// conditions are met
    async fn check_and_prune(&self) -> IndexerResult<()> {
        // Fetch watermark for this specific table
        let watermark = match self
            .store
            .get_watermark_by_entity(self.table.as_ref().to_string())
            .await
        {
            Ok(Some(w)) => w,
            Ok(None) => {
                // No watermark entry yet, skip
                tracing::warn!("no watermark entry found for table {}", self.table.as_ref());
                return Ok(());
            }
            Err(e) => {
                error!(
                    "failed to fetch watermark for table {}: {e}",
                    self.table.as_ref()
                );
                return Err(e);
            }
        };
        // Wait for in-flight reads to timeout before pruning
        self.wait_for_pruning_delay(watermark.min_bounds_updated_at_timestamp_ms)
            .await?;

        let pruning_chunks = self.create_pruning_chunks(&watermark);

        for (start, end) in pruning_chunks {
            info!(
                "Pruning table {} in range [{start}..={end}]",
                self.table.as_ref(),
            );
            if let Err(err) = self.prune_by_chunk(start, end).await {
                error!(
                    "failed to prune table {} in range [{start}..={end}]: {err}",
                    self.table.as_ref(),
                );
                break;
            }

            // Update lowest_unpruned_key to the next chunk to prune
            let next_chunk_start = end + 1;
            if let Err(err) = self
                .store
                .update_watermark_lowest_unpruned_key(&self.table, next_chunk_start)
                .await
            {
                error!(
                    "failed to update lowest_unpruned_key for table {} to next chunk {}: {err}",
                    self.table.as_ref(),
                    next_chunk_start
                );
            }

            // Brief pause to relieve the I/O pressure on the DB
            tokio::time::sleep(DELAY_BETWEEN_PRUNING_CHUNKS).await;
        }

        self.metrics
            .last_pruned_epoch
            .set(watermark.min_available_epoch - 1);

        Ok(())
    }

    /// Creates an iterator of `(start, end)` inclusive ranges to prune,
    /// derived from the watermark and the table's pruning strategy.
    fn create_pruning_chunks(
        &self,
        watermark: &StoredWatermark,
    ) -> impl Iterator<Item = (u64, u64)> + Send {
        let strategy = self.table.pruning_strategy();
        let lowest_unpruned_key = watermark.lowest_unpruned_key as u64;
        let range_end = strategy.range_end(watermark);
        let batch_size = if strategy.is_batched() {
            self.pruning_batch_size
        } else {
            1
        };
        info!(
            "pruning table {} ({strategy:?}) in range: [{lowest_unpruned_key}..{range_end})",
            self.table.as_ref(),
        );
        (lowest_unpruned_key..range_end)
            .step_by(batch_size as usize)
            .map(move |start| {
                let end = (start + batch_size).min(range_end).saturating_sub(1);
                (start, end)
            })
    }

    /// Waits for the pruning delay to ensure in-flight reads complete or
    /// timeout
    async fn wait_for_pruning_delay(&self, watermark_timestamp_ms: i64) -> IndexerResult<()> {
        // The watermark timestamp indicates when data was marked for pruning.
        // We delay pruning to allow any reads accessing this data to complete or
        // timeout.
        let pruning_allowed_timestamp_ms = watermark_timestamp_ms as u64 + self.pruning_delay_ms;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if now_ms < pruning_allowed_timestamp_ms {
            let wait_duration = Duration::from_millis(pruning_allowed_timestamp_ms - now_ms);
            info!(
                "waiting {}ms for in-flight reads to timeout before pruning table {} (watermark timestamp: {}, delay: {}ms)",
                wait_duration.as_millis(),
                self.table.as_ref(),
                watermark_timestamp_ms,
                self.pruning_delay_ms,
            );

            self.cancel
                .run_until_cancelled(tokio::time::sleep(wait_duration))
                .await
                .ok_or_else(|| {
                    info!(
                        "pruning task for table {} cancelled during delay",
                        self.table.as_ref()
                    );
                    IndexerError::Generic("Pruning task cancelled".to_string())
                })?;
        }

        Ok(())
    }

    /// Prune the inclusive range `[start..=end]` using the table's pruning
    /// strategy.
    async fn prune_by_chunk(&self, start: u64, end: u64) -> Result<(), IndexerError> {
        match self.table.pruning_strategy() {
            PruningStrategy::ByEpochPartition => {
                for epoch in start..=end {
                    self.partition_manager
                        .drop_table_partition(self.table.as_ref().to_string(), epoch)?;
                    info!(
                        "dropped epoch {epoch} partition for table {}",
                        self.table.as_ref()
                    );
                }
            }

            PruningStrategy::ByCheckpoint => {
                self.store
                    .prune_table_by_checkpoint_range(&self.table, start, end)
                    .await?;
                info!(
                    "pruned table {} for checkpoint range [{start}..={end}]",
                    self.table.as_ref(),
                );
            }

            PruningStrategy::ByTransaction => {
                self.store
                    .prune_table_by_tx_range(&self.table, start, end)
                    .await?;
                info!(
                    "pruned table {} for transaction range [{start}..={end}]",
                    self.table.as_ref(),
                );
            }

            PruningStrategy::ByGlobalSeq => {
                self.prune_by_global_seq_with_limit(start, end).await?;
            }

            PruningStrategy::ByCheckpointWithLimit => {
                self.prune_by_checkpoint_with_limit(start, end).await?;
            }
        }
        Ok(())
    }

    /// Prune table by global_sequence_number range with LIMIT
    /// Keeps deleting batches until no more rows are returned in the range
    async fn prune_by_global_seq_with_limit(
        &self,
        start: u64,
        end: u64,
    ) -> Result<(), IndexerError> {
        let row_limit = self.pruning_batch_size;
        loop {
            let deleted = self
                .store
                .prune_table_by_global_seq_with_limit(&self.table, start, end, row_limit as i64)
                .await?;

            if deleted < row_limit as usize {
                info!(
                    "finished pruning table {} for global_seq range [{start}..={end}]",
                    self.table.as_ref(),
                );
                break;
            }

            info!(
                "pruned {deleted} rows from table {} (global_seq range [{start}..={end}])",
                self.table.as_ref(),
            );

            // Brief pause between batches
            tokio::time::sleep(DELAY_BETWEEN_PRUNING_CHUNKS).await;
        }
        Ok(())
    }

    /// Prune table by checkpoint range with row-limited deletes.
    /// Keeps deleting batches until no more rows remain in the range.
    async fn prune_by_checkpoint_with_limit(
        &self,
        start: u64,
        end: u64,
    ) -> Result<(), IndexerError> {
        let row_limit = self.pruning_batch_size;
        loop {
            let deleted = self
                .store
                .prune_table_by_checkpoint_with_limit(&self.table, start, end, row_limit as i64)
                .await?;

            if deleted < row_limit as usize {
                info!(
                    "finished pruning table {} for checkpoint range [{start}..={end}]",
                    self.table.as_ref(),
                );
                break;
            }

            info!(
                "pruned {deleted} rows from table {} (checkpoint range [{start}..={end}])",
                self.table.as_ref(),
            );

            tokio::time::sleep(DELAY_BETWEEN_PRUNING_CHUNKS).await;
        }
        Ok(())
    }
}

impl Pruner {
    /// Instantiates a pruner with default retention and overrides. Pruner will
    /// finalize the retention policies so there is a value for every
    /// prunable table.
    pub fn new(
        store: PgIndexerStore,
        retention_config: RetentionConfig,
        pruning_delay_ms: u64,
        pruning_batch_size: u64,
        metrics: IndexerMetrics,
    ) -> Result<Self, IndexerError> {
        let blocking_cp = PrimaryWorker::pg_blocking_cp(store.clone()).unwrap();
        let partition_manager = PgPartitionManager::new(blocking_cp)?;
        let retention_policies = retention_config.retention_policies();

        Ok(Self {
            store,
            partition_manager,
            retention_policies,
            pruning_delay_ms,
            pruning_batch_size,
            metrics,
        })
    }

    pub async fn start(&self, cancel: CancellationToken) -> IndexerResult<()> {
        let store_clone = self.store.clone();
        let retention_policies = self.retention_policies.clone();
        let cancel_clone = cancel.clone();
        spawn_monitored_task!(update_watermarks_lower_bounds_task(
            store_clone,
            retention_policies,
            cancel_clone
        ));

        // Spawn one persistent task for each PrunableTable variant
        for table in PrunableTable::iter() {
            let store_clone = self.store.clone();
            let partition_manager_clone = self.partition_manager.clone();
            let pruning_delay_ms = self.pruning_delay_ms;
            let pruning_batch_size = self.pruning_batch_size;
            let metrics_clone = self.metrics.clone();
            let cancel_clone = cancel.clone();

            spawn_monitored_task!(async move {
                let table_pruner = TablePruner::new(
                    table,
                    &store_clone,
                    &partition_manager_clone,
                    pruning_delay_ms,
                    pruning_batch_size,
                    &metrics_clone,
                    cancel_clone,
                );
                table_pruner.run_pruning_task().await
            });
        }

        cancel.cancelled().await;
        info!("Pruner task cancelled.");
        Ok(())
    }
}

/// Task to periodically query the `watermarks` table and update the lower
/// bounds for all watermarks if the entry exceeds epoch-level retention policy.
async fn update_watermarks_lower_bounds_task(
    store: PgIndexerStore,
    retention_policies: HashMap<PrunableTable, u64>,
    cancel: CancellationToken,
) -> IndexerResult<()> {
    let mut interval = tokio::time::interval(UPDATE_WATERMARKS_LOWER_BOUNDS_TASK_INTERVAL);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Pruner watermark lower bound update task cancelled.");
                return Ok(());
            }
            _ = interval.tick() => {
                update_watermarks_lower_bounds(&store, &retention_policies, &cancel).await?;
            }
        }
    }
}

/// Fetches all entries from the `watermarks` table, and updates the
/// `min_available_*` columns for each entry if its epoch range exceeds the
/// respective retention policy.
async fn update_watermarks_lower_bounds(
    store: &PgIndexerStore,
    retention_policies: &HashMap<PrunableTable, u64>,
    cancel: &CancellationToken,
) -> IndexerResult<()> {
    let (watermarks, _) = store.get_watermarks().await?;

    if cancel.is_cancelled() {
        info!("Pruner watermark lower bound update task cancelled.");
        return Ok(());
    }

    let mut lower_bound_updates = vec![];
    for watermark in watermarks.iter() {
        let Some(prunable_table) = watermark.entity() else {
            continue;
        };
        let Some(epochs_to_keep) = retention_policies.get(&prunable_table) else {
            error!("no retention policy found for prunable table {prunable_table}");
            continue;
        };
        if let Some(new_min_available_epoch) = watermark.new_min_available_epoch(*epochs_to_keep) {
            lower_bound_updates.push((prunable_table, new_min_available_epoch));
        };
    }

    if !lower_bound_updates.is_empty() {
        store
            .update_watermarks_lower_bound(lower_bound_updates)
            .await?;
        info!("Finished updating lower bounds for watermarks");
    }

    Ok(())
}
