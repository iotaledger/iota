// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[cfg(feature = "pg_integration")]
mod common;
#[cfg(feature = "pg_integration")]
mod ingestion_tests {
    use std::sync::Arc;

    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
    use iota_indexer::{
        db::get_pool_connection,
        errors::{Context, IndexerError},
        models::{objects::StoredObject, transactions::StoredTransaction},
        schema::{objects, transactions},
    };
    use iota_types::{
        IOTA_FRAMEWORK_PACKAGE_ID, base_types::IotaAddress, effects::TransactionEffectsAPI,
        gas_coin::GasCoin,
    };
    use simulacrum::Simulacrum;
    use tempfile::tempdir;

    use crate::common::{
        indexer_wait_for_checkpoint, start_simulacrum_rest_api_with_write_indexer,
    };

    macro_rules! read_only_blocking {
        ($pool:expr, $query:expr) => {{
            let mut pg_pool_conn = get_pool_connection($pool)?;
            pg_pool_conn
                .build_transaction()
                .read_only()
                .run($query)
                .map_err(|e| IndexerError::PostgresRead(e.to_string()))
        }};
    }

    #[tokio::test]
    pub async fn test_transaction_table() -> Result<(), IndexerError> {
        let mut sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().into_path();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random_for_testing_only();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        // Create a checkpoint which should include the transaction we executed.
        let checkpoint = sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_rest_api_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let digest = effects.transaction_digest();

        // Read the transaction from the database directly.
        let db_txn: StoredTransaction = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            transactions::table
                .filter(transactions::transaction_digest.eq(digest.inner().to_vec()))
                .first::<StoredTransaction>(conn)
        })
        .context("Failed reading transaction from PostgresDB")?;

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

    #[tokio::test]
    pub async fn test_object_type() -> Result<(), IndexerError> {
        let mut sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().into_path();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random_for_testing_only();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (_, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        // Create a checkpoint which should include the transaction we executed.
        let _ = sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_rest_api_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let obj_id = transaction.gas()[0].0;

        // Read the transaction from the database directly.
        let db_object: StoredObject = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            objects::table
                .filter(objects::object_id.eq(obj_id.to_vec()))
                .first::<StoredObject>(conn)
        })
        .context("Failed reading object from PostgresDB")?;

        let obj_type_tag = GasCoin::type_();

        // Check that the different components of the event type were stored correctly.
        assert_eq!(
            db_object.object_type,
            Some(obj_type_tag.to_canonical_string(true))
        );
        assert_eq!(
            db_object.object_type_package,
            Some(IOTA_FRAMEWORK_PACKAGE_ID.to_vec())
        );
        assert_eq!(db_object.object_type_module, Some("coin".to_string()));
        assert_eq!(db_object.object_type_name, Some("Coin".to_string()));
        Ok(())
    }
}
