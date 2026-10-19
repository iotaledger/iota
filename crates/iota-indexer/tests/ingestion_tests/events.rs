// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_indexer::{
    errors::IndexerError,
    ingestion::OperationalLevel::{
        self, Basic, CombinedEventFilters, FilteredQueries, ObjectsHistory,
    },
    schema::{
        event_emit_module, event_emit_package, event_senders, event_struct_instantiation,
        event_struct_module, event_struct_name, event_struct_package, events,
    },
    store::{PgIndexerStore, indexer_store::IndexerStore},
    types::EventIndex,
};
use rstest::rstest;

use crate::utils::{TestSim, assert_row_count, start_indexer_at_genesis};

/// This test case verifies that pg_store.persist_tx_indices correctly
/// splits large vectors of EventIndex into smaller chunks of size
/// PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX.
///
/// This prevents the Postgres error:
/// ```text
/// "error encoding message to server: value too large to transmit"
/// ```
#[tokio::test]
pub async fn test_insert_large_batch_event_indices() -> Result<(), IndexerError> {
    let sim = TestSim::new(
        OperationalLevel::default(),
        "indexer_ingestion_event_indices_db",
    );
    let pg_store = sim.start_indexer(None).await;

    // By creating 2000 random event indices we ensure that the
    // persist_event_indices function will flatten each EventIndex by
    // extracting all field data and collecting each field type
    // into its own separate vector (each field has one element). This will
    // result in having n vectors with length of 2000, which will
    // trigger the large vectors to be split into chunks of
    // PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX.
    let event_indices = std::iter::repeat_with(EventIndex::random)
        .take(2000)
        .collect();
    pg_store.persist_event_indices(event_indices).await?;
    Ok(())
}

#[rstest]
#[tokio::test]
pub async fn events_table(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    let pg_store = start_indexer_at_genesis(level, "ingestion_events_table").await;
    assert_row_count!(&pg_store, events, level.includes(FilteredQueries)).await
}

#[rstest]
#[tokio::test]
pub async fn event_index_tables(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    let pg_store = start_indexer_at_genesis(level, "ingestion_event_index_tables").await;
    assert_count_event_index(&pg_store, level.includes(CombinedEventFilters)).await
}

async fn assert_count_event_index(
    pg_store: &PgIndexerStore,
    ingested: bool,
) -> Result<(), IndexerError> {
    assert_row_count!(pg_store, event_emit_package, ingested).await?;
    assert_row_count!(pg_store, event_emit_module, ingested).await?;
    assert_row_count!(pg_store, event_senders, ingested).await?;
    assert_row_count!(pg_store, event_struct_package, ingested).await?;
    assert_row_count!(pg_store, event_struct_module, ingested).await?;
    assert_row_count!(pg_store, event_struct_name, ingested).await?;
    assert_row_count!(pg_store, event_struct_instantiation, ingested).await?;
    Ok(())
}
