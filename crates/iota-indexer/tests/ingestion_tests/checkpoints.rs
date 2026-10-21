// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use iota_indexer::{
    errors::{Context, IndexerError},
    ingestion::OperationalLevel::{
        self, Basic, CombinedEventFilters, FilteredQueries, ObjectsHistory,
    },
    models::checkpoints::StoredCheckpoint,
    pruning::CommitterTables,
    schema::{checkpoints, epochs},
    store::{
        PgIndexerStore, diesel_macro, indexer_store::IndexerStore,
        pg_partition_manager::PgPartitionManager,
    },
};
use rstest::rstest;

use crate::{
    common::{indexer_wait_for_checkpoint, indexer_wait_for_epoch},
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
pub async fn epoch_partitioned_tables() -> Result<(), IndexerError> {
    EpochPartitionedTables
        .run(
            OperationalLevel::CombinedEventFilters,
            "ingestion_epoch_partitioned_tables",
        )
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
        let sim = TestSim::new(level, database_name);
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
        let sim = TestSim::new(level, database_name);
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

        assert_watermarks(&max_committed_cps, CommitterTables::basic(), true);
        assert_watermarks(
            &max_committed_cps,
            CommitterTables::objects_history(),
            level.includes(ObjectsHistory),
        );
        assert_watermarks(
            &max_committed_cps,
            CommitterTables::filtered_queries(),
            level.includes(FilteredQueries),
        );
        assert_watermarks(
            &max_committed_cps,
            CommitterTables::combined_event_filters(),
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

struct EpochPartitionedTables;

impl IngestionTest for EpochPartitionedTables {
    /// The level of the indexer and the number of epochs advanced.
    type ExpectedState = (OperationalLevel, u64);

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        let num_epochs = 4;
        for _i in 0..num_epochs {
            sim.inner.create_checkpoint(); // checkpoint at epoch _i
            sim.inner.advance_epoch(false); // last checkpoint at epoch _i and epoch _i + 1
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
            assert_eq!(
                new_partitions, num_epochs,
                "incorrect partitions for ingested {table}",
            );
        }
        Ok(())
    }
}
