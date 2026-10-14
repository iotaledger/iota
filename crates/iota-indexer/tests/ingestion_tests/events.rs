// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_indexer::{errors::IndexerError, store::indexer_store::IndexerStore, types::EventIndex};

use crate::utils::TestSim;

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
    let sim = TestSim::new();
    let pg_store = sim
        .start_indexer_with(Some("indexer_ingestion_event_indices_db"), None)
        .await;

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
