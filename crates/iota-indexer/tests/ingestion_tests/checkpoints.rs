// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use iota_indexer::{
    errors::{Context, IndexerError},
    ingestion::OperationalLevel::{
        self, Basic, CombinedEventFilters, FilteredQueries, ObjectsHistory,
    },
    models::checkpoints::StoredCheckpoint,
    schema::{checkpoints, epochs},
    store::PgIndexerStore,
};
use rstest::rstest;

use crate::{
    common::indexer_wait_for_checkpoint,
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
