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
    spawn_monitored_task,
    store::{IndexerStore, PgIndexerStore, pg_partition_manager::PgPartitionManager},
    types::IndexerResult,
};

const UPDATE_WATERMARKS_LOWER_BOUNDS_TASK_INTERVAL: Duration = Duration::from_secs(5);
/// Delay in milliseconds before pruning data after watermark timestamp.
/// This delay allows in-flight reads that may be accessing data scheduled for
/// pruning to complete or timeout, ensuring safe pruning without affecting
/// active queries.
#[cfg(any(test, feature = "pg_integration", feature = "shared_test_runtime"))]
const PRUNING_DELAY_MS: u64 = 1000; // 1 second for tests

#[cfg(not(any(test, feature = "pg_integration", feature = "shared_test_runtime")))]
const PRUNING_DELAY_MS: u64 = 2 * 60 * 60 * 1000; // 2 hours for production

/// Maximum number of transactions to prune in a single batch for ByTransaction
/// strategy
const MAX_TRANSACTIONS_PER_PRUNE_BATCH: u64 = 1000;

/// Maximum number of checkpoints to prune in a single batch for ByCheckpoint
/// strategy
const MAX_CHECKPOINTS_PER_PRUNE_BATCH: u64 = 1000;

/// Interval for running the pruning task
const PRUNING_TASK_INTERVAL: Duration = Duration::from_secs(5);

/// Delay between pruning chunks to relieve I/O pressure on the database
const DELAY_BETWEEN_PRUNING_CHUNKS: Duration = Duration::from_millis(100);

pub struct Pruner {
    pub store: PgIndexerStore,
    pub partition_manager: PgPartitionManager,
    pub retention_policies: HashMap<PrunableTable, u64>,
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
    ObjectsHistory,
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

/// Represents a specific chunk of data to be pruned
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PruningChunk {
    /// Prune an entire epoch partition
    Epoch(u64),
    /// Prune a range of checkpoints [start..=end] inclusive
    CheckpointRange(u64, u64),
    /// Prune a range of transactions [start..=end] inclusive
    TransactionRange(u64, u64),
    /// Prune by global_sequence_number range [start..=end] inclusive
    GlobalSeqRange(u64, u64),
    /// Prune by checkpoint range [start..=end] with row-limited deletes
    CheckpointRangeWithLimit(u64, u64),
}

impl PruningChunk {
    /// Returns the start of the next chunk to prune (used for updating
    /// pruner_hi watermark)
    fn next_chunk_start(&self) -> u64 {
        match self {
            PruningChunk::Epoch(epoch) => epoch + 1,
            PruningChunk::CheckpointRange(_, end) => end + 1,
            PruningChunk::TransactionRange(_, end) => end + 1,
            PruningChunk::GlobalSeqRange(_, end) => end + 1,
            PruningChunk::CheckpointRangeWithLimit(_, end) => end + 1,
        }
    }
}

impl PrunableTable {
    /// Returns the pruning strategy for this table
    pub fn pruning_strategy(&self) -> PruningStrategy {
        match self {
            // Epoch-partitioned tables - pruned by dropping partitions
            // objects_history: partitioned by CheckpointSequenceNumber
            // transactions: partitioned by TxSequenceNumber
            // events: partitioned by TxSequenceNumber
            PrunableTable::ObjectsHistory | PrunableTable::Transactions | PrunableTable::Events => {
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
    #[allow(dead_code)]
    metrics: &'a IndexerMetrics,
    cancel: CancellationToken,
}

impl<'a> TablePruner<'a> {
    pub fn new(
        table: PrunableTable,
        store: &'a PgIndexerStore,
        partition_manager: &'a PgPartitionManager,
        metrics: &'a IndexerMetrics,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            table,
            store,
            partition_manager,
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

        for pruning_chunk in pruning_chunks {
            info!(
                "Pruning table {} for {:?}",
                self.table.as_ref(),
                pruning_chunk
            );
            if let Err(err) = self.prune_by_chunk(pruning_chunk).await {
                error!(
                    "failed to prune table {} for {:?}: {err}",
                    self.table.as_ref(),
                    pruning_chunk
                );
                break;
            }

            // Update lowest_unpruned_key to the next chunk to prune
            let next_chunk_start = pruning_chunk.next_chunk_start();
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

    /// Creates an iterator of pruning chunks based on the watermark and pruning
    /// strategy
    fn create_pruning_chunks(
        &self,
        watermark: &crate::models::watermarks::StoredWatermark,
    ) -> Box<dyn Iterator<Item = PruningChunk> + Send> {
        let min_available_epoch = watermark.min_available_epoch as u64;
        let lowest_unpruned_key = watermark.lowest_unpruned_key as u64;
        let min_available_cp = watermark.min_available_cp as u64;
        let min_available_tx = watermark.min_available_tx as u64;

        match self.table.pruning_strategy() {
            PruningStrategy::ByEpochPartition => {
                let range_end = min_available_epoch;
                info!(
                    "pruning table {} in epoch range: [{lowest_unpruned_key}..{range_end})",
                    self.table.as_ref()
                );
                Box::new((lowest_unpruned_key..range_end).map(PruningChunk::Epoch))
            }
            PruningStrategy::ByCheckpoint => {
                let range_end = min_available_cp;
                info!(
                    "pruning table {} in checkpoint range: [{lowest_unpruned_key}..{range_end})",
                    self.table.as_ref()
                );
                Box::new(
                    (lowest_unpruned_key..range_end)
                        .step_by(MAX_CHECKPOINTS_PER_PRUNE_BATCH as usize)
                        .map(move |start| {
                            let end = (start + MAX_CHECKPOINTS_PER_PRUNE_BATCH).min(range_end);
                            PruningChunk::CheckpointRange(start, end - 1)
                        }),
                )
            }
            PruningStrategy::ByTransaction => {
                let range_end = min_available_tx;
                info!(
                    "pruning table {} in transaction range: [{lowest_unpruned_key}..{range_end})",
                    self.table.as_ref()
                );
                Box::new(
                    (lowest_unpruned_key..range_end)
                        .step_by(MAX_TRANSACTIONS_PER_PRUNE_BATCH as usize)
                        .map(move |start| {
                            let end = (start + MAX_TRANSACTIONS_PER_PRUNE_BATCH).min(range_end);
                            PruningChunk::TransactionRange(start, end - 1)
                        }),
                )
            }
            PruningStrategy::ByGlobalSeq => {
                let range_end = min_available_tx;
                info!(
                    "pruning table {} by global_sequence_number in range: [{lowest_unpruned_key}..{range_end})",
                    self.table.as_ref()
                );
                Box::new(
                    (lowest_unpruned_key..range_end)
                        .step_by(MAX_TRANSACTIONS_PER_PRUNE_BATCH as usize)
                        .map(move |start| {
                            let end = (start + MAX_TRANSACTIONS_PER_PRUNE_BATCH).min(range_end);
                            PruningChunk::GlobalSeqRange(start, end - 1)
                        }),
                )
            }
            PruningStrategy::ByCheckpointWithLimit => {
                let range_end = min_available_cp;
                info!(
                    "pruning table {} in checkpoint range (with limit): [{lowest_unpruned_key}..{range_end})",
                    self.table.as_ref()
                );
                Box::new(
                    (lowest_unpruned_key..range_end)
                        .step_by(MAX_CHECKPOINTS_PER_PRUNE_BATCH as usize)
                        .map(move |start| {
                            let end = (start + MAX_CHECKPOINTS_PER_PRUNE_BATCH).min(range_end);
                            PruningChunk::CheckpointRangeWithLimit(start, end - 1)
                        }),
                )
            }
        }
    }

    /// Waits for the pruning delay to ensure in-flight reads complete or
    /// timeout
    async fn wait_for_pruning_delay(&self, watermark_timestamp_ms: i64) -> IndexerResult<()> {
        // The watermark timestamp indicates when data was marked for pruning.
        // We delay pruning to allow any reads accessing this data to complete or
        // timeout.
        let pruning_allowed_timestamp_ms = watermark_timestamp_ms as u64 + PRUNING_DELAY_MS;
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
                PRUNING_DELAY_MS
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

    /// Prune data based on the specified chunk
    async fn prune_by_chunk(&self, chunk: PruningChunk) -> Result<(), IndexerError> {
        match chunk {
            PruningChunk::Epoch(epoch) => {
                // Drop the partition for this epoch
                self.partition_manager
                    .drop_table_partition(self.table.as_ref().to_string(), epoch)?;
                info!(
                    "dropped epoch {epoch} partition for table {}",
                    self.table.as_ref()
                );
            }

            PruningChunk::CheckpointRange(start, end) => {
                // Prune by checkpoint range
                if let Err(e) = self
                    .store
                    .prune_table_by_checkpoint_range(&self.table, start, end)
                    .await
                {
                    error!(
                        "failed to prune table {} for checkpoint range [{start}..={end}]: {e}",
                        self.table.as_ref(),
                    );
                }
                info!(
                    "pruned table {} for checkpoint range [{start}..={end}]",
                    self.table.as_ref(),
                );
            }

            PruningChunk::TransactionRange(start, end) => {
                // Prune by transaction range
                if let Err(e) = self
                    .store
                    .prune_table_by_tx_range(&self.table, start, end)
                    .await
                {
                    error!(
                        "failed to prune table {} for transaction range [{start}..={end}]: {e}",
                        self.table.as_ref(),
                    );
                }
                info!(
                    "pruned table {} for transaction range [{start}..={end}]",
                    self.table.as_ref(),
                );
            }

            PruningChunk::GlobalSeqRange(start, end) => {
                self.prune_by_global_seq_with_limit(start, end).await?;
            }

            PruningChunk::CheckpointRangeWithLimit(start, end) => {
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
        loop {
            let deleted = self
                .store
                .prune_table_by_global_seq_with_limit(
                    &self.table,
                    start,
                    end,
                    MAX_TRANSACTIONS_PER_PRUNE_BATCH as i64,
                )
                .await?;

            if deleted < MAX_TRANSACTIONS_PER_PRUNE_BATCH as usize {
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
        loop {
            let deleted = self
                .store
                .prune_table_by_checkpoint_with_limit(
                    &self.table,
                    start,
                    end,
                    MAX_CHECKPOINTS_PER_PRUNE_BATCH as i64,
                )
                .await?;

            if deleted < MAX_CHECKPOINTS_PER_PRUNE_BATCH as usize {
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
        metrics: IndexerMetrics,
    ) -> Result<Self, IndexerError> {
        let blocking_cp = PrimaryWorker::pg_blocking_cp(store.clone()).unwrap();
        let partition_manager = PgPartitionManager::new(blocking_cp)?;
        let retention_policies = retention_config.retention_policies();

        Ok(Self {
            store,
            partition_manager,
            retention_policies,
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
            let metrics_clone = self.metrics.clone();
            let cancel_clone = cancel.clone();

            spawn_monitored_task!(async move {
                let table_pruner = TablePruner::new(
                    table,
                    &store_clone,
                    &partition_manager_clone,
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
