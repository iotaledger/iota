// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
#[allow(dead_code)]
#[cfg(feature = "pg_integration")]
mod common;
#[cfg(feature = "pg_integration")]
mod ingestion_tests {
    use std::sync::Arc;

    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
    use iota_indexer::{
        db::get_pg_pool_connection,
        errors::{Context, IndexerError},
        models::transactions::StoredTransaction,
        schema::transactions,
    };
    use iota_types::{base_types::IotaAddress, effects::TransactionEffectsAPI};
    use simulacrum::Simulacrum;

    use crate::common::{
        indexer_wait_for_checkpoint, start_simulacrum_rest_api_with_write_indexer,
    };

    macro_rules! read_only_blocking {
        ($pool:expr, $query:expr) => {{
            let mut pg_pool_conn = get_pg_pool_connection($pool)?;
            pg_pool_conn
                .build_transaction()
                .read_only()
                .run($query)
                .map_err(|e| IndexerError::PostgresReadError(e.to_string()))
        }};
    }

    #[tokio::test]
    pub async fn test_transaction_table() -> Result<(), IndexerError> {
        let mut sim = Simulacrum::new();

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random_for_testing_only();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        // Create a checkpoint which should include the transaction we executed.
        let checkpoint = sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_rest_api_with_write_indexer(Arc::new(sim)).await;

        // Wait for the indexer to catch up to the checkpoint.
        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let digest = effects.transaction_digest();

        // Read the transaction from the database directly.
        let db_txn: StoredTransaction = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            transactions::table
                .filter(transactions::transaction_digest.eq(digest.inner().to_vec()))
                .first::<StoredTransaction>(conn)
        })
        .context("Failed reading latest checkpoint sequence number from PostgresDB")?;

        // Check that the transaction was stored correctly.
        assert_eq!(db_txn.tx_sequence_number, 1);
        assert_eq!(db_txn.transaction_digest, digest.inner().to_vec());
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
}
