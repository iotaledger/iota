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
    schema::checkpoints,
    store::PgIndexerStore,
};
use rstest::rstest;

use crate::{
    common::indexer_wait_for_checkpoint,
    utils::{IngestionTest, TestSim, read_only_blocking},
};

#[rstest]
#[tokio::test]
pub async fn test_epoch_boundary(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    EpochBoundary.run(level, "ingestion_epoch_boundary").await
}

struct EpochBoundary;

impl IngestionTest for EpochBoundary {
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
