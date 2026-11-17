// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use diesel::{
    ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper, connection::BoxableConnection,
};
use iota_indexer::{
    errors::{Context, IndexerError},
    insert_or_ignore_into,
    models::transactions::{StoredTransaction, TxGlobalOrder},
    schema::{transactions, tx_global_order},
    store::{PgIndexerStore, indexer_store::IndexerStore},
    test_utils::DBInitHook,
    transactional_blocking_with_retry,
    types::TxIndex,
};
use iota_sdk_types::TransactionDigest;
use iota_types::effects::TransactionEffectsAPI;

use crate::{
    common::indexer_wait_for_checkpoint,
    utils::{TestSim, read_only_blocking},
};

async fn read_tx_global_order(
    pg_store: &PgIndexerStore,
    digest: TransactionDigest,
) -> Result<TxGlobalOrder, IndexerError> {
    read_only_blocking!(&pg_store.blocking_cp(), |conn| {
        tx_global_order::table
            .filter(tx_global_order::tx_digest.eq(digest.bytes().to_vec()))
            .select(TxGlobalOrder::as_select())
            .first::<TxGlobalOrder>(conn)
    })
    .context("failed reading `tx_global_order` from PostgresDB")
}

/// Returns a hook that inserts the `tx_global_order` row of `digest`
/// without sequence numbers, as optimistic indexing does before the
/// checkpoint arrives.
fn emulate_insertion_order_set_earlier_by_optimistic_indexing(
    digest: TransactionDigest,
) -> DBInitHook {
    Box::new(move |pg_store: &PgIndexerStore| {
        transactional_blocking_with_retry!(
            &pg_store.blocking_cp(),
            |conn| {
                let insertable = TxGlobalOrder {
                    tx_digest: digest.bytes().to_vec(),
                    optimistic_sequence_number: None,
                    tx_sequence_number: None,
                };
                insert_or_ignore_into!(tx_global_order::table, insertable, conn);
                Ok::<(), IndexerError>(())
            },
            Duration::from_secs(60)
        )
        .unwrap()
    })
}

#[tokio::test]
pub async fn transaction_table() -> Result<(), IndexerError> {
    let sim = TestSim::new("ingestion_transaction_table");
    let (transaction, effects, _) = sim.execute_transfer();
    // Create a checkpoint which should include the transaction we executed.
    let checkpoint = sim.inner.create_checkpoint();

    let pg_store = sim.start_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let digest = *effects.transaction_digest();

    // Read the transaction from the database directly.
    let db_txn: StoredTransaction = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
        transactions::table
            .filter(transactions::transaction_digest.eq(digest.bytes().to_vec()))
            .first::<StoredTransaction>(conn)
    })
    .context("failed reading transaction from PostgresDB")?;

    // Check that the transaction was stored correctly.
    assert_eq!(db_txn.tx_sequence_number, 1);
    assert_eq!(db_txn.transaction_digest, digest.bytes().to_vec());
    assert_eq!(
        db_txn.raw_transaction,
        bcs::to_bytes(&transaction.data()).unwrap()
    );
    assert_eq!(db_txn.raw_effects, bcs::to_bytes(&effects).unwrap());
    assert_eq!(db_txn.timestamp_ms, checkpoint.timestamp_ms as i64);
    assert_eq!(db_txn.checkpoint_sequence_number, 1);
    assert_eq!(db_txn.transaction_kind, 1);
    assert_eq!(db_txn.success_command_count, 2); // split coin + transfer
    Ok(())
}

#[tokio::test]
pub async fn tx_global_order_table() -> Result<(), IndexerError> {
    let sim = TestSim::new("ingestion_tx_global_order");
    let (_, effects, _) = sim.execute_transfer();
    sim.inner.create_checkpoint();

    let pg_store = sim.start_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let stored = read_tx_global_order(&pg_store, *effects.transaction_digest()).await?;

    assert!(stored.tx_sequence_number.is_some());
    let expected_optimistic_sequence_number = -1;
    assert_eq!(
        stored.optimistic_sequence_number,
        Some(expected_optimistic_sequence_number)
    );
    Ok(())
}

#[tokio::test]
pub async fn tx_global_order_table_on_conflict_do_nothing() -> Result<(), IndexerError> {
    let sim = TestSim::new("ingestion_tx_global_order_on_conflict_do_nothing");
    let (_, effects, _) = sim.execute_transfer();
    sim.inner.create_checkpoint();
    let digest = *effects.transaction_digest();

    let pg_store = sim
        .start_indexer(Some(
            emulate_insertion_order_set_earlier_by_optimistic_indexing(digest),
        ))
        .await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;

    let stored = read_tx_global_order(&pg_store, digest).await?;

    // The checkpoint upsert must fill in the sequence number without
    // replacing the row inserted by the optimistic path.
    assert!(stored.tx_sequence_number.is_some());
    let expected_optimistic_sequence_number = 1;
    assert_eq!(
        stored.optimistic_sequence_number,
        Some(expected_optimistic_sequence_number)
    );
    Ok(())
}

/// This test case verifies that pg_store.persist_tx_indices correctly
/// splits large vectors of TxIndex into smaller chunks of size
/// PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX.
///
/// This prevents the Postgres error:
/// ```text
/// "error encoding message to server: value too large to transmit"
/// ```
#[tokio::test]
pub async fn test_insert_large_batch_tx_indices() -> Result<(), IndexerError> {
    let sim = TestSim::new("indexer_ingestion_tx_indices_db");
    let pg_store = sim.start_indexer(None).await;

    // By creating 1000 random tx indices we ensure that the
    // persist_event_indices function will flatten each TxIndex by
    // extracting all field data and collecting each field type
    // into its own separate vector (each field has one element or more
    // elements). This will result in having n vectors with length
    // greater than PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX, which will
    // trigger the large vectors to be split into chunks of
    // PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX.
    let tx_indices = std::iter::repeat_with(TxIndex::random).take(1000).collect();
    pg_store.persist_tx_indices(tx_indices).await?;
    Ok(())
}
