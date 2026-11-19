// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use iota_indexer::{
    errors::{Context, IndexerError},
    ingestion::OperationalLevel::{
        self, Basic, CombinedEventFilters, FilteredQueries, ObjectsHistory,
    },
    models::{checkpoints::StoredCheckpoint, watermarks::WatermarkLowerBounds},
    pruning::CommitterTables,
    schema::{checkpoints, epochs},
    store::{
        PgIndexerStore, diesel_macro, indexer_store::IndexerStore,
        pg_partition_manager::PgPartitionManager,
    },
};
use rstest::rstest;

use crate::{
    common::{indexer_wait_for_checkpoint, indexer_wait_for_epoch, retry_with_timeout},
    utils::{IngestionTest, TestSim, read_only_blocking},
};

#[rstest]
#[tokio::test]
pub async fn test_checkpoints_at_epoch_boundary(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    CheckpointsAtEpochBoundary
        .run(level, "ingestion_checkpoints_at_epoch_boundary")
        .await
}

#[rstest]
#[tokio::test]
pub async fn test_epochs(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    Epochs(CheckpointsAtEpochBoundary)
        .run(level, "ingestion_epochs")
        .await
}

#[rstest]
#[tokio::test]
pub async fn watermarks_table(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    WatermarksTable
        .run(level, "ingestion_watermarks_table")
        .await
}

#[rstest]
#[tokio::test]
pub async fn watermarks_lower_bound_after_level_change(
    #[values(ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    WatermarksLowerBoundAfterLevelChange
        .run(level, "ingestion_watermarks_lower_bound_after_level_change")
        .await
}

#[rstest]
#[tokio::test]
pub async fn epoch_partitioned_tables(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    EpochPartitionedTables
        .run(level, "ingestion_epoch_partitioned_tables")
        .await
}

struct CheckpointsAtEpochBoundary;

impl IngestionTest for CheckpointsAtEpochBoundary {
    type ExpectedState = ();

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let mut sim = TestSim::new(level, database_name);
        sim.execute_transfer();
        sim.inner.create_checkpoint(); // checkpoint 1
        sim.inner.advance_epoch(false); // checkpoint 2 and epoch 1
        sim.execute_transfer();
        sim.inner.create_checkpoint(); // checkpoint 3

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 3).await;
        (pg_store, ())
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        _: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        let db_checkpoint = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            checkpoints::table
                .order(checkpoints::sequence_number.desc())
                .first::<StoredCheckpoint>(conn)
        })
        .context("failed to read checkpoint")?;
        assert_eq!(db_checkpoint.sequence_number, 3);
        assert_eq!(db_checkpoint.epoch, 1);
        Ok(())
    }
}

struct Epochs(CheckpointsAtEpochBoundary);

impl IngestionTest for Epochs {
    type ExpectedState = ();

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        self.0.setup(level, database_name).await
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        _: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        let last_checkpoint_ids = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            epochs::table
                .select(epochs::last_checkpoint_id)
                .order(epochs::epoch.asc())
                .load::<Option<i64>>(conn)
        })
        .context("failed to read epochs")?;
        assert_eq!(last_checkpoint_ids.len(), 2);
        assert!(
            last_checkpoint_ids[0].is_some(),
            "epoch 0 should have a last_checkpoint_id"
        );
        Ok(())
    }
}

struct WatermarksTable;

impl IngestionTest for WatermarksTable {
    /// The level of the indexer.
    type ExpectedState = OperationalLevel;

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let mut sim = TestSim::new(level, database_name);
        sim.execute_transfer();
        sim.inner.create_checkpoint(); // checkpoint 1

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;
        (pg_store, level)
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        level: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        let (watermarks, _) = pg_store.get_watermarks().await?;
        let max_committed_cps = watermarks
            .into_iter()
            .map(|watermark| (watermark.entity, watermark.max_committed_cp))
            .collect::<HashMap<_, _>>();

        assert_watermarks(&max_committed_cps, CommitterTables::BASIC, true);
        assert_watermarks(
            &max_committed_cps,
            CommitterTables::OBJECTS_HISTORY,
            level.includes(ObjectsHistory),
        );
        assert_watermarks(
            &max_committed_cps,
            CommitterTables::FILTERED_QUERIES,
            level.includes(FilteredQueries),
        );
        assert_watermarks(
            &max_committed_cps,
            CommitterTables::COMBINED_EVENT_FILTERS,
            level.includes(CombinedEventFilters),
        );
        Ok(())
    }
}

/// Asserts that the `max_committed_cp` of each table is above 0 if `ingested`
/// is `true`, or that it is absent or 0 if it is `false`.
fn assert_watermarks(
    max_committed_cps: &HashMap<String, i64>,
    tables: impl IntoIterator<Item = CommitterTables>,
    ingested: bool,
) {
    for table in tables {
        let max_committed_cp = max_committed_cps.get(table.as_ref()).copied().unwrap_or(0);
        if ingested {
            assert!(max_committed_cp > 0, "{table} has no watermark");
        } else {
            assert_eq!(max_committed_cp, 0, "{table} has a watermark");
        }
    }
}

struct WatermarksLowerBoundAfterLevelChange;

impl IngestionTest for WatermarksLowerBoundAfterLevelChange {
    /// The expected lower bound of `objects_backward_history`.
    type ExpectedState = WatermarkLowerBounds;

    /// Starts the indexer on a level higher than `Basic` and then
    /// restarts the same database two times, with the levels
    /// `Basic`, and `ObjectsHistory`.
    ///
    /// Before each start, the simulator creates the following checkpoints:
    ///
    /// | Start | Level            | Checkpoints | `objects_backward_history` |
    /// |-------|------------------|-------------|----------------------------|
    /// | 1     | `level`          | 0-1         | new row                    |
    /// | 2     | `Basic`          | 2-3         | not written                |
    /// | 3     | `ObjectsHistory` | 4-5         | gap before checkpoint 4    |
    ///
    /// The lower bound of `objects_backward_history` must move from genesis to
    /// checkpoint 4. The lower bound of `checkpoints` must stay at genesis.
    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let mut sim = TestSim::new(level, database_name);

        sim.execute_transfer();
        let checkpoint = sim.inner.create_checkpoint();
        let mut pg_store = sim.start_indexer(None).await;
        wait_for_checkpoints_watermark(&pg_store, checkpoint.sequence_number as i64).await;

        let mut lower_bound = WatermarkLowerBounds {
            min_available_epoch: checkpoint.epoch as i64,
            min_available_cp: checkpoint.sequence_number as i64,
            min_available_tx: 0,
        };
        let mut network_total_transactions = checkpoint.network_total_transactions;
        for restart_level in [Basic, ObjectsHistory] {
            sim.execute_transfer();
            let first = sim.inner.create_checkpoint();
            sim.execute_transfer();
            let last = sim.inner.create_checkpoint();
            lower_bound = WatermarkLowerBounds {
                min_available_epoch: first.epoch as i64,
                min_available_cp: first.sequence_number as i64,
                min_available_tx: network_total_transactions as i64,
            };
            network_total_transactions = last.network_total_transactions;

            pg_store = sim.restart_indexer(restart_level).await;
            wait_for_checkpoints_watermark(&pg_store, last.sequence_number as i64).await;
        }

        (pg_store, lower_bound)
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        expected: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        let (watermarks, _) = pg_store.get_watermarks().await?;
        let lower_bounds = watermarks
            .into_iter()
            .map(|w| {
                let lower_bound = WatermarkLowerBounds {
                    min_available_epoch: w.min_available_epoch,
                    min_available_cp: w.min_available_cp,
                    min_available_tx: w.min_available_tx,
                };
                (w.entity, lower_bound)
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(
            lower_bounds.get(CommitterTables::ObjectsBackwardHistory.as_ref()),
            Some(&expected),
            "wrong lower bound for objects_backward_history"
        );
        let genesis = WatermarkLowerBounds {
            min_available_epoch: 0,
            min_available_cp: 0,
            min_available_tx: 0,
        };
        assert_eq!(
            lower_bounds.get(CommitterTables::Checkpoints.as_ref()),
            Some(&genesis),
            "wrong lower bound for checkpoints"
        );
        Ok(())
    }
}

/// Waits until the watermark of `checkpoints` reaches `checkpoint`.
///
/// The writer updates the watermarks after it writes the `checkpoints` rows, so
/// [`indexer_wait_for_checkpoint`] can return before the watermarks change.
async fn wait_for_checkpoints_watermark(pg_store: &PgIndexerStore, checkpoint: i64) {
    retry_with_timeout(Duration::from_secs(30), || async move {
        let (watermarks, _) = pg_store.get_watermarks().await.unwrap();
        watermarks.into_iter().find(|watermark| {
            watermark.entity == CommitterTables::Checkpoints.as_ref()
                && watermark.max_committed_cp >= checkpoint
        })
    })
    .await
    .expect("timeout waiting for the watermark of checkpoints");
}

struct EpochPartitionedTables;

impl IngestionTest for EpochPartitionedTables {
    /// The level of the indexer and the number of epochs advanced.
    type ExpectedState = (OperationalLevel, u64);

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let mut sim = TestSim::new(level, database_name);
        let num_epochs = 4;
        for _i in 0..num_epochs {
            sim.inner.create_checkpoint(); // checkpoint at epoch _i
            sim.inner.advance_epoch(false); // last checkpoint at epoch _i, starts epoch _i + 1
        }
        sim.inner.create_checkpoint(); // checkpoint at epoch 4
        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_epoch(&pg_store, num_epochs).await;
        (pg_store, (level, num_epochs))
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        (indexer_level, num_epochs): Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        let connection_pool = pg_store.blocking_cp();
        let table_partitions = diesel_macro::spawn_blocking_task(|| {
            PgPartitionManager::new(connection_pool).get_table_partitions()
        })
        .await
        .map_err(IndexerError::from)??;

        let partition_manager = PgPartitionManager::new(pg_store.blocking_cp());
        for (table, (first, last)) in table_partitions {
            if !partition_manager
                .get_strategy(table.as_ref())
                .is_epoch_partitioned()
            {
                continue;
            }
            let new_partitions = last - first;
            let committer_table: CommitterTables = table.parse().unwrap();
            if indexer_level.includes(committer_table.level()) {
                assert_eq!(
                    new_partitions, num_epochs,
                    "incorrect partitions for ingested {table}",
                );
            } else {
                assert_eq!(
                    new_partitions, 0,
                    "added new partitions for non-ingested {table}",
                );
            }
        }
        Ok(())
    }
}
