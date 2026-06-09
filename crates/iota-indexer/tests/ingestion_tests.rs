// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[expect(dead_code)]
#[cfg(feature = "pg_integration")]
mod common;
#[cfg(feature = "pg_integration")]
mod ingestion_tests {
    use std::{sync::Arc, time::Duration};

    use diesel::{
        BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper,
        connection::BoxableConnection,
    };
    use iota_indexer::{
        db::get_pool_connection,
        errors::{Context, IndexerError},
        ingestion::common::prepare::CheckpointObjectChanges,
        insert_or_ignore_into,
        models::{
            checkpoints::StoredCheckpoint,
            obj_indices::StoredObjectVersion,
            objects::{BackwardHistoryObjectStatus, StoredCheckpointedObject, StoredObject},
            transactions::{StoredTransaction, TxGlobalOrder},
            tx_indices::StoredTxDigest,
        },
        schema::{
            checkpointed_objects, checkpoints, objects, transactions, tx_digests, tx_global_order,
        },
        store::{PgIndexerStore, indexer_store::IndexerStore},
        transactional_blocking_with_retry,
        types::{EventIndex, ObjectStatus, TxIndex},
    };
    use iota_sdk_types::{Identifier, ObjectId, Owner, StructTag};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::{IotaAddress, ObjectRef, SequenceNumber},
        crypto::KeypairTraits,
        effects::{TransactionEffects, TransactionEffectsAPI},
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{CallArg, Transaction, TransactionData, TransactionDataAPI},
    };
    use simulacrum::Simulacrum;
    use tempfile::tempdir;

    use crate::common::{
        backward_history::{find_all_entries_at_checkpoint, find_backward_entry},
        indexer_wait_for_checkpoint,
        object_versions::find_object_versions_at_checkpoint,
        start_simulacrum_grpc_with_write_indexer,
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
    pub async fn checkpoint_objects_ingestion() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        let checkpoint_objects = (0..1000)
            .map(|_| CheckpointObjectChanges::random())
            .collect();
        pg_store.persist_objects(checkpoint_objects).await?;
        Ok(())
    }

    #[tokio::test]
    pub async fn transaction_table() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        // Create a checkpoint which should include the transaction we executed.
        let checkpoint = sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
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
        .context("failed reading transaction from PostgresDB")?;

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
    pub async fn object_type() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (_, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        // Create a checkpoint which should include the transaction we executed.
        let _ = sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let obj_id = transaction.gas()[0].object_id;

        // Read the transaction from the database directly.
        let db_object: StoredObject = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            objects::table
                .filter(objects::object_id.eq(obj_id.as_bytes()))
                .first::<StoredObject>(conn)
        })
        .context("Failed reading object from PostgresDB")?;

        let obj_type_tag = StructTag::new_gas_coin();

        // Check that the different components of the event type were stored correctly.
        assert_eq!(
            db_object.object_type,
            Some(obj_type_tag.to_canonical_string(true))
        );
        assert_eq!(
            db_object.object_type_package,
            Some(IotaAddress::FRAMEWORK.as_bytes().to_vec())
        );
        assert_eq!(db_object.object_type_module, Some("coin".to_string()));
        assert_eq!(db_object.object_type_name, Some("Coin".to_string()));
        Ok(())
    }

    #[tokio::test]
    pub async fn tx_global_order_table() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        // Create a checkpoint which should include the transaction we executed.
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let digest = effects.transaction_digest();

        let stored_tx_digest = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            tx_digests::table
                .filter(tx_digests::tx_digest.eq(digest.inner().to_vec()))
                .select(StoredTxDigest::as_select())
                .first::<StoredTxDigest>(conn)
        })
        .context("failed reading `tx_global_order` from PostgresDB")?;

        let stored_global_order = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            tx_global_order::table
                .filter(tx_global_order::tx_digest.eq(digest.inner().to_vec()))
                .select(TxGlobalOrder::as_select())
                .first::<TxGlobalOrder>(conn)
        })
        .context("failed reading `tx_global_order` from PostgresDB")?;

        assert_eq!(
            stored_global_order.global_sequence_number,
            stored_tx_digest.tx_sequence_number
        );
        let expected_optimistic_sequence_number = -1;
        assert_eq!(
            stored_global_order.optimistic_sequence_number,
            Some(expected_optimistic_sequence_number)
        );
        Ok(())
    }

    #[tokio::test]
    pub async fn tx_global_order_table_on_conflict_do_nothing() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute a simple transaction.
        let transfer_recipient = IotaAddress::random();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());
        // Create a checkpoint which should include the transaction we executed.
        sim.create_checkpoint();
        let digest = *effects.transaction_digest();

        let global_sequence_number = 123;
        let emulate_insertion_order_set_earlier_by_optimistic_indexing =
            move |pg_store: &PgIndexerStore| {
                transactional_blocking_with_retry!(
                    &pg_store.blocking_cp(),
                    |conn| {
                        let insertable = TxGlobalOrder {
                            tx_digest: digest.inner().to_vec(),
                            global_sequence_number,
                            optimistic_sequence_number: None,
                            chk_tx_sequence_number: None,
                        };
                        insert_or_ignore_into!(tx_global_order::table, insertable, conn);
                        Ok::<(), IndexerError>(())
                    },
                    Duration::from_secs(60)
                )
                .unwrap()
            };

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
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
        let stored = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            tx_global_order::table
                .filter(tx_global_order::tx_digest.eq(digest.inner().to_vec()))
                .select(TxGlobalOrder::as_select())
                .first::<TxGlobalOrder>(conn)
        })
        .context("failed reading `tx_global_order` from PostgresDB")?;

        assert_eq!(stored.global_sequence_number, global_sequence_number);
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
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tx_indices_db"),
            None,
        )
        .await;

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
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_event_indices_db"),
            None,
        )
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

    #[tokio::test]
    pub async fn checkpoint_objects_are_finalized() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        let transfer_recipient = IotaAddress::random();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (_, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let max_cp = IndexerStore::get_latest_checkpoint_sequence_number(&pg_store)
            .await?
            .unwrap() as i64;
        let non_finalized_count: i64 = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            objects::table
                .filter(
                    objects::finalized_in_cp
                        .is_not_null()
                        .and(objects::finalized_in_cp.gt(max_cp)),
                )
                .count()
                .get_result::<i64>(conn)
        })
        .context("Failed reading objects from PostgresDB")?;

        assert_eq!(
            non_finalized_count, 0,
            "All objects should be finalized after checkpoint indexing"
        );
        Ok(())
    }

    #[tokio::test]
    pub async fn test_epoch_boundary() -> Result<(), IndexerError> {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        let data_ingestion_path = tmp_dir.path().to_path_buf();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        let transfer_recipient = IotaAddress::random();
        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (_, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none());

        sim.create_checkpoint(); // checkpoint 1
        sim.advance_epoch(); // checkpoint 2 and epoch 1

        let (transaction, _) = sim.transfer_txn(transfer_recipient);
        let (_, err) = sim.execute_transaction(transaction.clone()).unwrap();
        sim.create_checkpoint(); // checkpoint 3
        assert!(err.is_none());

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            None,
            None,
        )
        .await;
        indexer_wait_for_checkpoint(&pg_store, 3).await;
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

    /// Verify that `checkpointed_objects` matches `objects` (minus
    /// `finalized_in_cp`) after checkpoint ingestion with mutations and
    /// deletions.
    #[tokio::test]
    pub async fn checkpointed_objects_match_objects() -> Result<(), IndexerError> {
        let sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().keep();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // Execute several transfers to create mutations.
        let recipient1 = IotaAddress::random();
        let recipient2 = IotaAddress::random();

        let (tx1, _) = sim.transfer_txn(recipient1);
        let (_, err) = sim.execute_transaction(tx1).unwrap();
        assert!(err.is_none());
        sim.create_checkpoint();

        let (tx2, _) = sim.transfer_txn(recipient2);
        let (_, err) = sim.execute_transaction(tx2).unwrap();
        assert!(err.is_none());
        sim.create_checkpoint();

        // One more transfer to trigger further mutations on gas objects.
        let (tx3, _) = sim.transfer_txn(recipient1);
        let (_, err) = sim.execute_transaction(tx3).unwrap();
        assert!(err.is_none());
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 3).await;

        // Read all rows from both tables.
        let all_objects: Vec<StoredObject> = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            objects::table
                .order(objects::object_id.asc())
                .load::<StoredObject>(conn)
        })
        .context("failed reading objects")?;

        let all_checkpointed: Vec<StoredCheckpointedObject> =
            read_only_blocking!(&pg_store.blocking_cp(), |conn| {
                checkpointed_objects::table
                    .order(checkpointed_objects::object_id.asc())
                    .select(StoredCheckpointedObject::as_select())
                    .load::<StoredCheckpointedObject>(conn)
            })
            .context("failed reading checkpointed_objects")?;

        assert!(
            !all_objects.is_empty(),
            "objects table should not be empty after ingestion"
        );

        // Filter to active checkpointed objects only for comparison with `objects`.
        let active_checkpointed: Vec<_> = all_checkpointed
            .iter()
            .filter(|cp| cp.object_status == ObjectStatus::Active as i16)
            .collect();

        assert_eq!(
            all_objects.len(),
            active_checkpointed.len(),
            "objects and active checkpointed_objects should have the same number of rows"
        );

        // Compare each active row field by field (minus finalized_in_cp,
        // object_status, checkpoint_sequence_number).
        for (obj, cp_obj) in all_objects.iter().zip(active_checkpointed.iter()) {
            assert_eq!(obj.object_id, cp_obj.object_id, "object_id mismatch");
            assert_eq!(
                obj.object_version, cp_obj.object_version,
                "object_version mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                Some(&obj.object_digest),
                cp_obj.object_digest.as_ref(),
                "object_digest mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                Some(obj.owner_type),
                cp_obj.owner_type,
                "owner_type mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.owner_id, cp_obj.owner_id,
                "owner_id mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.object_type, cp_obj.object_type,
                "object_type mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.object_type_package, cp_obj.object_type_package,
                "object_type_package mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.object_type_module, cp_obj.object_type_module,
                "object_type_module mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.object_type_name, cp_obj.object_type_name,
                "object_type_name mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                Some(&obj.serialized_object),
                cp_obj.serialized_object.as_ref(),
                "serialized_object mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.coin_type, cp_obj.coin_type,
                "coin_type mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.coin_balance, cp_obj.coin_balance,
                "coin_balance mismatch for {:?}",
                obj.object_id
            );
            assert_eq!(
                obj.df_kind, cp_obj.df_kind,
                "df_kind mismatch for {:?}",
                obj.object_id
            );
        }

        Ok(())
    }

    /// Verify that `objects_backward_history` is populated correctly, including
    /// when multiple transactions land in the same checkpoint.
    ///
    /// `transfer_txn` splits a coin from the gas object and transfers it.
    /// Each such transaction produces:
    ///   - a CREATED coin  → NOT_YET_CREATED backward entry (version=lamport-1)
    ///   - a MUTATED gas   → ACTIVE backward entry with previous version/data
    #[tokio::test]
    pub async fn backward_history_ingestion() -> Result<(), IndexerError> {
        let sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().keep();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // --- checkpoint 1: TWO transfers in the same checkpoint ---
        // This exercises the case where multiple transactions produce backward
        // history entries for the same checkpoint, and where the same object
        // (the gas coin) is mutated by both transactions within one checkpoint.
        let recipient1 = IotaAddress::random();
        let (tx1, _) = sim.transfer_txn(recipient1);
        let gas_ref_tx1 = tx1.gas()[0];
        let gas_object_id = gas_ref_tx1.object_id;
        let gas_version_before_tx1 = gas_ref_tx1.version;
        let (effects1, err) = sim.execute_transaction(tx1).unwrap();
        assert!(err.is_none());
        let created1: Vec<_> = effects1
            .created()
            .into_iter()
            .map(|(r, _)| (r.object_id, r.version))
            .collect();
        assert_eq!(
            created1.len(),
            1,
            "transfer_txn should create exactly 1 coin"
        );
        let (created_coin_1, created_coin_1_version) = created1[0];

        // Second transfer in the same checkpoint — uses the same gas object
        // (now at an updated version).
        let recipient2 = IotaAddress::random();
        let (tx2, _) = sim.transfer_txn(recipient2);
        let gas_version_before_tx2 = tx2.gas()[0].version;
        assert!(
            gas_version_before_tx2 > gas_version_before_tx1,
            "gas should have been bumped after tx1"
        );
        let (effects2, err) = sim.execute_transaction(tx2).unwrap();
        assert!(err.is_none());
        let created2: Vec<_> = effects2
            .created()
            .into_iter()
            .map(|(r, _)| (r.object_id, r.version))
            .collect();
        assert_eq!(created2.len(), 1);
        let (created_coin_2, created_coin_2_version) = created2[0];

        // Both transactions land in checkpoint 1.
        sim.create_checkpoint();

        // --- checkpoint 2: one more transfer ---
        let recipient3 = IotaAddress::random();
        let (tx3, _) = sim.transfer_txn(recipient3);
        let gas_version_before_tx3 = tx3.gas()[0].version;
        let (effects3, err) = sim.execute_transaction(tx3).unwrap();
        assert!(err.is_none());
        let created3: Vec<_> = effects3
            .created()
            .into_iter()
            .map(|(r, _)| (r.object_id, r.version))
            .collect();
        assert_eq!(created3.len(), 1);
        let (created_coin_3, created_coin_3_version) = created3[0];
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 2).await;

        // === checkpoint 1 assertions (two transactions) ===

        // Both created coins should have NOT_YET_CREATED entries, with
        // object_version = lamport - 1 (the version assigned by the create tx,
        // minus one).
        let entry = find_backward_entry(&pg_store, created_coin_1.as_bytes(), 1)?
            .expect("created coin 1 must have a backward history entry at cp 1");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::NotYetCreated as i16
        );
        assert_eq!(
            entry.object_version,
            created_coin_1_version.as_u64() as i64 - 1
        );
        assert!(entry.serialized_object.is_none());
        assert!(entry.object_digest.is_none());

        let entry = find_backward_entry(&pg_store, created_coin_2.as_bytes(), 1)?
            .expect("created coin 2 must have a backward history entry at cp 1");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::NotYetCreated as i16
        );
        assert_eq!(
            entry.object_version,
            created_coin_2_version.as_u64() as i64 - 1
        );

        // The gas object was mutated twice in checkpoint 1 — there should be
        // two ACTIVE entries with different previous versions.
        let gas_entries = find_all_entries_at_checkpoint(&pg_store, gas_object_id.as_bytes(), 1)?;
        assert_eq!(
            gas_entries.len(),
            2,
            "gas object should have 2 backward history entries in cp 1 (one per tx)"
        );
        assert_eq!(
            gas_entries[0].object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert_eq!(
            gas_entries[0].object_version,
            gas_version_before_tx1.as_u64() as i64
        );
        assert!(gas_entries[0].serialized_object.is_some());
        assert!(gas_entries[0].object_digest.is_some());
        assert!(gas_entries[0].owner_type.is_some());

        assert_eq!(
            gas_entries[1].object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert_eq!(
            gas_entries[1].object_version,
            gas_version_before_tx2.as_u64() as i64
        );
        assert!(gas_entries[1].serialized_object.is_some());

        // === checkpoint 2 assertions (single transaction) ===

        let entry = find_backward_entry(&pg_store, created_coin_3.as_bytes(), 2)?
            .expect("created coin 3 must have a backward history entry at cp 2");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::NotYetCreated as i16
        );
        assert_eq!(
            entry.object_version,
            created_coin_3_version.as_u64() as i64 - 1
        );

        let entry = find_backward_entry(&pg_store, gas_object_id.as_bytes(), 2)?
            .expect("gas object must have a backward history entry at cp 2");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert_eq!(entry.object_version, gas_version_before_tx3.as_u64() as i64);
        assert!(entry.serialized_object.is_some());

        Ok(())
    }

    /// Verify that `objects_version` is populated correctly: one row per output
    /// object per transaction. Tests repeated mutations of the same object in a
    /// single checkpoint.
    #[tokio::test]
    pub async fn object_versions_ingestion() -> Result<(), IndexerError> {
        let sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().keep();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        // two transfers using the same gas object
        let (tx1, _) = sim.transfer_txn(IotaAddress::random());
        let gas_object_id = tx1.gas()[0].object_id;
        let (effects1, err) = sim.execute_transaction(tx1).unwrap();
        assert!(err.is_none());
        let gas_after_tx1 = effects1
            .mutated()
            .into_iter()
            .find(|(r, _)| r.object_id == gas_object_id)
            .expect("gas must be mutated by tx1")
            .0
            .version;
        let created_coin_1 = effects1.created()[0].0;

        let (tx2, _) = sim.transfer_txn(IotaAddress::random());
        let (effects2, err) = sim.execute_transaction(tx2).unwrap();
        assert!(err.is_none());
        let gas_after_tx2 = effects2
            .mutated()
            .into_iter()
            .find(|(r, _)| r.object_id == gas_object_id)
            .expect("gas must be mutated by tx2")
            .0
            .version;
        let created_coin_2 = effects2.created()[0].0;

        sim.create_checkpoint();

        // one more transfer, separate checkpoint
        let (tx3, _) = sim.transfer_txn(IotaAddress::random());
        let (effects3, err) = sim.execute_transaction(tx3).unwrap();
        assert!(err.is_none());
        let gas_after_tx3 = effects3
            .mutated()
            .into_iter()
            .find(|(r, _)| r.object_id == gas_object_id)
            .expect("gas must be mutated by tx3")
            .0
            .version;
        let created_coin_3 = effects3.created()[0].0;
        sim.create_checkpoint();

        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;

        indexer_wait_for_checkpoint(&pg_store, 2).await;

        let make_version_row =
            |id: ObjectId, version: SequenceNumber, cp: i64| StoredObjectVersion {
                object_id: id.as_bytes().to_vec(),
                object_version: version.as_u64() as i64,
                cp_sequence_number: cp,
            };

        // checkpoint 1: gas mutated twice + two coins created
        let mut cp1_expected = vec![
            make_version_row(gas_object_id, gas_after_tx1, 1),
            make_version_row(gas_object_id, gas_after_tx2, 1),
            make_version_row(created_coin_1.object_id, created_coin_1.version, 1),
            make_version_row(created_coin_2.object_id, created_coin_2.version, 1),
        ];
        cp1_expected.sort();
        assert_eq!(
            find_object_versions_at_checkpoint(&pg_store, 1)?,
            cp1_expected
        );

        // checkpoint 2: gas mutated once + one coin created
        let mut cp2_expected = vec![
            make_version_row(gas_object_id, gas_after_tx3, 2),
            make_version_row(created_coin_3.object_id, created_coin_3.version, 2),
        ];
        cp2_expected.sort();
        assert_eq!(
            find_object_versions_at_checkpoint(&pg_store, 2)?,
            cp2_expected
        );

        Ok(())
    }

    /// Executes transaction in simulacrum, asserts success and returns effects.
    fn execute_signed(sim: &Simulacrum, tx_data: TransactionData) -> TransactionEffects {
        let (sender, key) = sim.with_keystore(|ks| {
            let (s, k) = ks.accounts().next().unwrap();
            (*s, k.copy())
        });
        assert_eq!(tx_data.sender(), sender);
        let tx = Transaction::from_data_and_signer(tx_data, vec![&key]);
        let (effects, err) = sim.execute_transaction(tx).unwrap();
        assert!(err.is_none(), "tx failed: {err:?}");
        effects
    }

    fn pick_gas(sim: &Simulacrum, sender: IotaAddress) -> ObjectRef {
        sim.with_store(|s| {
            s.owned_objects(sender)
                .find(|o| o.is_gas_coin())
                .unwrap()
                .object_ref()
        })
    }

    /// Verify `objects_version` ingestion for every wrap/removal effect,
    /// namely: `wrapped`, `unwrapped`, `deleted`, and
    /// `unwrapped_then_deleted`.
    #[tokio::test]
    pub async fn objects_version_ingestion_wraps_and_removals() -> Result<(), IndexerError> {
        let sim = Simulacrum::new();
        let data_ingestion_path = tempdir().unwrap().keep();
        sim.set_data_ingestion_path(data_ingestion_path.clone());

        let sender = sim.with_keystore(|ks| *ks.accounts().next().unwrap().0);
        let rgp = sim.reference_gas_price();
        let module = Identifier::from_static("example");

        // publish package
        let gas = pick_gas(&sim, sender);
        let publish_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .publish_examples("simple_warrior")
                .build(),
        );
        let package_id = publish_fx
            .created()
            .into_iter()
            .find(|(_, owner)| matches!(owner, Owner::Immutable))
            .expect("publish must create an immutable package")
            .0
            .object_id;
        sim.create_checkpoint();

        // mint a Sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let strength = builder.pure(42u8).unwrap();
        let sword_arg = builder.programmable_move_call(
            package_id,
            module.clone(),
            Identifier::from_static("new_sword"),
            vec![],
            vec![strength],
        );
        builder.transfer_arg(sender, sword_arg);
        let mint_sword_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let sword_v0 = mint_sword_fx
            .created()
            .into_iter()
            .find(|(_, owner)| matches!(owner, Owner::Address(_)))
            .expect("mint must create the sword")
            .0;
        sim.create_checkpoint();

        // cp3: mint a Warrior + equip sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_arg = builder.programmable_move_call(
            package_id,
            module.clone(),
            Identifier::from_static("new_warrior"),
            vec![],
            vec![],
        );
        let sword_input = builder.obj(CallArg::ImmutableOrOwned(sword_v0)).unwrap();
        builder.programmable_move_call(
            package_id,
            module.clone(),
            Identifier::from_static("equip"),
            vec![],
            vec![warrior_arg, sword_input],
        );
        builder.transfer_arg(sender, warrior_arg);
        let equip_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let warrior_v0 = equip_fx
            .created()
            .into_iter()
            .find(|(_, owner)| matches!(owner, Owner::Address(_)))
            .expect("equip tx must create the warrior")
            .0;
        let sword_wrapped_version = equip_fx
            .wrapped()
            .into_iter()
            .find(|r| r.object_id == sword_v0.object_id)
            .expect("sword must be wrapped in the equip tx")
            .version;
        sim.create_checkpoint();

        // cp4: unequip sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = builder.obj(CallArg::ImmutableOrOwned(warrior_v0)).unwrap();
        let unwrapped_sword_arg = builder.programmable_move_call(
            package_id,
            module.clone(),
            Identifier::from_static("unequip"),
            vec![],
            vec![warrior_input],
        );
        builder.transfer_arg(sender, unwrapped_sword_arg);
        let unequip_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let sword_unwrapped_ref = unequip_fx
            .unwrapped()
            .into_iter()
            .find(|(r, _)| r.object_id == sword_v0.object_id)
            .expect("sword must be unwrapped in the unequip tx")
            .0;
        let warrior_after_unequip = unequip_fx
            .mutated()
            .into_iter()
            .find(|(r, _)| r.object_id == warrior_v0.object_id)
            .expect("warrior must be mutated by unequip")
            .0;
        sim.create_checkpoint();

        // cp5: equip sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = builder
            .obj(CallArg::ImmutableOrOwned(warrior_after_unequip))
            .unwrap();
        let sword_input = builder
            .obj(CallArg::ImmutableOrOwned(sword_unwrapped_ref))
            .unwrap();
        builder.programmable_move_call(
            package_id,
            module.clone(),
            Identifier::from_static("equip"),
            vec![],
            vec![warrior_input, sword_input],
        );
        let reequip_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let sword_rewrapped_version = reequip_fx
            .wrapped()
            .into_iter()
            .find(|r| r.object_id == sword_v0.object_id)
            .expect("sword must be wrapped in the re-equip tx")
            .version;
        let warrior_after_reequip = reequip_fx
            .mutated()
            .into_iter()
            .find(|(r, _)| r.object_id == warrior_v0.object_id)
            .expect("warrior must be mutated by re-equip")
            .0;
        sim.create_checkpoint();

        // cp6: destroy warrior
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = builder
            .obj(CallArg::ImmutableOrOwned(warrior_after_reequip))
            .unwrap();
        builder.programmable_move_call(
            package_id,
            module.clone(),
            Identifier::from_static("destroy_warrior"),
            vec![],
            vec![warrior_input],
        );
        let destroy_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let warrior_deleted_version = destroy_fx
            .deleted()
            .into_iter()
            .find(|r| r.object_id == warrior_v0.object_id)
            .expect("warrior must be deleted in destroy_warrior")
            .version;
        let sword_unwrapped_deleted_version = destroy_fx
            .unwrapped_then_deleted()
            .into_iter()
            .find(|r| r.object_id == sword_v0.object_id)
            .expect("sword must be unwrapped_then_deleted in destroy_warrior")
            .version;
        sim.create_checkpoint();

        // spin up the indexer
        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_ingestion_path,
            None,
            Some("indexer_ingestion_tests_db"),
            None,
        )
        .await;
        indexer_wait_for_checkpoint(&pg_store, 6).await;

        let assert_present =
            |rows: &[StoredObjectVersion], id: ObjectId, version: SequenceNumber, label: &str| {
                assert!(
                    rows.iter().any(|r| r.object_id == id.as_bytes().to_vec()
                        && r.object_version == version.as_u64() as i64),
                    "expected {label}; rows: {rows:?}",
                );
            };

        // cp3: warrior created (output) + sword wrapped (removed)
        let cp3 = find_object_versions_at_checkpoint(&pg_store, 3)?;
        assert_present(
            &cp3,
            warrior_v0.object_id,
            warrior_v0.version,
            "created warrior in cp3",
        );
        assert_present(
            &cp3,
            sword_v0.object_id,
            sword_wrapped_version,
            "wrapped sword in cp3",
        );

        // cp4: sword unwrapped (output)
        let cp4 = find_object_versions_at_checkpoint(&pg_store, 4)?;
        assert_present(
            &cp4,
            sword_v0.object_id,
            sword_unwrapped_ref.version,
            "unwrapped sword in cp4",
        );

        // cp5: sword re-wrapped (removed)
        let cp5 = find_object_versions_at_checkpoint(&pg_store, 5)?;
        assert_present(
            &cp5,
            sword_v0.object_id,
            sword_rewrapped_version,
            "re-wrapped sword in cp5",
        );

        // cp6: warrior deleted + sword unwrapped_then_deleted (both removed)
        let cp6 = find_object_versions_at_checkpoint(&pg_store, 6)?;
        assert_present(
            &cp6,
            warrior_v0.object_id,
            warrior_deleted_version,
            "deleted warrior in cp6",
        );
        assert_present(
            &cp6,
            sword_v0.object_id,
            sword_unwrapped_deleted_version,
            "unwrapped_then_deleted sword in cp6",
        );

        Ok(())
    }
}
