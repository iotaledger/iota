// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[cfg(feature = "pg_integration")]
mod common;
#[cfg(feature = "pg_integration")]
mod ingestion_tests {
    use std::{sync::Arc, time::Duration};

    use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, connection::BoxableConnection};
    use iota_indexer::{
        db::get_pool_connection,
        errors::{Context, IndexerError},
        insert_or_ignore_into,
        models::{
            objects::{StoredObject, StoredObjectSnapshot},
            transactions::StoredTransaction,
        },
        schema::{objects, objects_snapshot, transactions, tx_insertion_order},
        store::PgIndexerStore,
        transactional_blocking_with_retry,
    };
    use iota_types::{
        IOTA_FRAMEWORK_PACKAGE_ID, base_types::IotaAddress, effects::TransactionEffectsAPI,
        gas_coin::GasCoin,
    };
    use simulacrum::Simulacrum;
    use tempfile::tempdir;

    use crate::common::{
        indexer_wait_for_checkpoint, start_simulacrum_rest_api_with_write_indexer,
        wait_for_objects_snapshot,
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
            None,
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
            None,
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

    #[tokio::test]
    pub async fn test_objects_snapshot() -> Result<(), IndexerError> {
        let tempdir = tempdir().unwrap();
        let mut sim = Simulacrum::new();
        let data_ingestion_path = tempdir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Run 10 transfer transactions and create 10 checkpoints
        let mut last_transaction = None;
        let total_checkpoint_sequence_number = 7usize;
        for _ in 0..total_checkpoint_sequence_number {
            let transfer_recipient = IotaAddress::random_for_testing_only();
            let (transaction, _) = sim.transfer_txn(transfer_recipient);
            let (_, err) = sim.execute_transaction(transaction.clone()).unwrap();
            assert!(err.is_none());
            last_transaction = Some(transaction);
            let _ = sim.create_checkpoint();
        }

        let (_, pg_store, _) = start_simulacrum_rest_api_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        // Wait for objects snapshot at checkpoint
        // max_expected_checkpoint_sequence_number
        let max_expected_checkpoint_sequence_number = total_checkpoint_sequence_number - 5;
        wait_for_objects_snapshot(&pg_store, max_expected_checkpoint_sequence_number as u64)
            .await?;

        // Get max checkpoint_sequence_number from objects_snapshot table and assert
        // it's expected
        let max_checkpoint_sequence_number = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            objects_snapshot::table
                .select(objects_snapshot::checkpoint_sequence_number)
                .order(objects_snapshot::checkpoint_sequence_number.desc())
                .limit(1)
                .first::<i64>(conn)
        })
        .context("Failed reading max checkpoint_sequence_number from PostgresDB")?;

        assert_eq!(
            max_checkpoint_sequence_number,
            max_expected_checkpoint_sequence_number as i64
        );

        // Get the object state at max_expected_checkpoint_sequence_number and assert.
        let last_tx = last_transaction.unwrap();
        let obj_id = last_tx.gas()[0].0;
        let gas_owner_id = last_tx.sender_address();

        let snapshot_object = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            objects_snapshot::table
                .filter(objects_snapshot::object_id.eq(obj_id.to_vec()))
                .filter(
                    objects_snapshot::checkpoint_sequence_number
                        .eq(max_expected_checkpoint_sequence_number as i64),
                )
                .first::<StoredObjectSnapshot>(conn)
        })
        .context("Failed reading snapshot object from PostgresDB")?;
        // Assert that the object state is as expected at checkpoint
        // max_expected_checkpoint_sequence_number
        assert_eq!(snapshot_object.object_id, obj_id.to_vec());
        assert_eq!(
            snapshot_object.checkpoint_sequence_number,
            max_expected_checkpoint_sequence_number as i64
        );
        assert_eq!(snapshot_object.owner_type, Some(1));
        assert_eq!(snapshot_object.owner_id, Some(gas_owner_id.to_vec()));
        Ok(())
    }

    #[tokio::test]
    #[ignore = "https://github.com/iotaledger/iota/issues/6668"]
    pub async fn test_tx_insertion_order_table() -> Result<(), IndexerError> {
        let mut sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().into_path();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random_for_testing_only();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        // Create a checkpoint which should include the transaction we executed.
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_rest_api_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let digest = effects.transaction_digest();

        // Read the transaction from the database directly.
        let actual_insertion_order = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            tx_insertion_order::table
                .filter(tx_insertion_order::tx_digest.eq(digest.inner().to_vec()))
                .select(tx_insertion_order::insertion_order)
                .first::<i64>(conn)
        })
        .context("Failed reading tx insertion order from PostgresDB")?;

        assert_eq!(actual_insertion_order, 2);
        Ok(())
    }

    #[tokio::test]
    #[ignore = "https://github.com/iotaledger/iota/issues/6668"]
    pub async fn test_tx_insertion_order_table_for_existing_digest() -> Result<(), IndexerError> {
        let mut sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().into_path();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random_for_testing_only();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());
        // Create a checkpoint which should include the transaction we executed.
        sim.create_checkpoint();
        let digest = *effects.transaction_digest();

        let pre_existing_insertion_order = 123;
        let emulate_insertion_order_set_earlier_by_optimistic_indexing =
            move |pg_store: &PgIndexerStore| {
                transactional_blocking_with_retry!(
                    &pg_store.blocking_cp(),
                    |conn| {
                        insert_or_ignore_into!(
                            tx_insertion_order::table,
                            (
                                tx_insertion_order::dsl::tx_digest.eq(digest.inner().to_vec()),
                                tx_insertion_order::dsl::insertion_order
                                    .eq(pre_existing_insertion_order),
                            ),
                            conn
                        );
                        Ok::<(), IndexerError>(())
                    },
                    Duration::from_secs(60)
                )
                .unwrap()
            };

        let (_, pg_store, _) = start_simulacrum_rest_api_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            Some(Box::new(
                emulate_insertion_order_set_earlier_by_optimistic_indexing,
            )),
        )
        .await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;

        // Read the transaction from the database directly.
        let actual_insertion_order = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            tx_insertion_order::table
                .filter(tx_insertion_order::tx_digest.eq(digest.inner().to_vec()))
                .select(tx_insertion_order::insertion_order)
                .first::<i64>(conn)
        })
        .context("Failed reading tx insertion order from PostgresDB")?;

        assert_eq!(actual_insertion_order, pre_existing_insertion_order);
        Ok(())
    }
}
