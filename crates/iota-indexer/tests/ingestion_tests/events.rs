// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{QueryDsl, RunQueryDsl};
use iota_indexer::{
    errors::{Context, IndexerError},
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
use iota_sdk_types::Owner;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::effects::TransactionEffectsAPI;
use rstest::rstest;

use crate::{
    common::indexer_wait_for_checkpoint,
    utils::{IngestionTest, TestSim, assert_row_count, read_only_blocking},
};

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
    EventsTable.run(level, "ingestion_events_table").await
}

#[rstest]
#[tokio::test]
pub async fn event_index_tables(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    EventIndexTables(EventsTable)
        .run(level, "ingestion_event_index_tables")
        .await
}

struct EventsTable;

impl IngestionTest for EventsTable {
    type ExpectedState = ();

    const MIN_LEVEL: OperationalLevel = FilteredQueries;

    /// Publishes the `nft` example and then mints an NFT.
    ///
    /// The mint emits one `NFTMinted` event.
    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        let sender = sim.sender();
        let rgp = sim.inner.reference_gas_price();

        let publish_fx = sim.execute_signed(
            TestTransactionBuilder::new(sender, sim.pick_gas(), rgp)
                .publish_examples("nft")
                .build(),
        );
        let package_id = publish_fx
            .created()
            .into_iter()
            .find(|created| matches!(created.owner(), Owner::Immutable))
            .expect("publish must create an immutable package")
            .reference()
            .object_id;

        sim.execute_signed(
            TestTransactionBuilder::new(sender, sim.pick_gas(), rgp)
                .call_nft_create(package_id)
                .build(),
        );
        sim.inner.create_checkpoint();

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;
        (pg_store, ())
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        _state: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        assert_row_count!(pg_store, events, true).await
    }

    async fn assert_not_ingested(&self, pg_store: &PgIndexerStore) -> Result<(), IndexerError> {
        assert_row_count!(pg_store, events, false).await
    }
}

struct EventIndexTables(EventsTable);

impl EventIndexTables {
    async fn assert(pg_store: &PgIndexerStore, ingested: bool) -> Result<(), IndexerError> {
        assert_row_count!(pg_store, event_emit_package, ingested).await?;
        assert_row_count!(pg_store, event_emit_module, ingested).await?;
        assert_row_count!(pg_store, event_senders, ingested).await?;
        assert_row_count!(pg_store, event_struct_package, ingested).await?;
        assert_row_count!(pg_store, event_struct_module, ingested).await?;
        assert_row_count!(pg_store, event_struct_name, ingested).await?;
        assert_row_count!(pg_store, event_struct_instantiation, ingested).await?;
        Ok(())
    }
}

impl IngestionTest for EventIndexTables {
    type ExpectedState = ();

    const MIN_LEVEL: OperationalLevel = CombinedEventFilters;

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
        _state: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        Self::assert(pg_store, true).await
    }

    async fn assert_not_ingested(&self, pg_store: &PgIndexerStore) -> Result<(), IndexerError> {
        Self::assert(pg_store, false).await
    }
}
