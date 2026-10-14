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
    use iota_common::random_util::TempDir;
    use iota_indexer::{
        db::get_pool_connection,
        errors::{Context, IndexerError},
        ingestion::common::prepare::CheckpointObjectChanges,
        insert_or_ignore_into,
        models::{
            checkpoints::StoredCheckpoint,
            obj_indices::StoredObjectVersion,
            objects::{StoredCheckpointedObject, StoredObject},
            transactions::{StoredTransaction, TxGlobalOrder},
        },
        schema::{checkpointed_objects, checkpoints, objects, transactions, tx_global_order},
        store::{PgIndexerStore, diesel_macro::spawn_blocking_task, indexer_store::IndexerStore},
        test_utils::DBInitHook,
        transactional_blocking_with_retry,
        types::{EventIndex, ObjectStatus, TxIndex},
    };
    use iota_sdk_types::{
        Address, Argument, Identifier, ObjectId, ObjectReference, OwnedObjectReference, Owner,
        StructTag, Transaction, TransactionDigest, TransactionEffects, Version,
    };
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        effects::TransactionEffectsAPI,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{CallArg, TransactionAPI, TransactionEnvelope},
    };
    use simulacrum::Simulacrum;

    use crate::common::{
        backward_history::{find_all_entries_at_checkpoint, find_backward_entry},
        indexer_wait_for_checkpoint,
        object_versions::find_object_versions_at_checkpoint,
        start_simulacrum_grpc_with_write_indexer,
    };

    const DB_NAME: &str = "indexer_ingestion_tests_db";

    macro_rules! read_only_blocking {
        ($pool:expr, $query:expr) => {{
            let pool = $pool.clone();
            spawn_blocking_task(move || {
                let mut pg_pool_conn = get_pool_connection(&pool)?;
                pg_pool_conn
                    .build_transaction()
                    .read_only()
                    .run($query)
                    .map_err(|e| {
                        eprintln!("failed to read from PostgresDB: {e:?}");
                        IndexerError::PostgresRead
                    })
            })
            .await
            .expect("failed to join Tokio blocking task")
        }};
    }

    /// Creates a simulacrum that writes checkpoint data to a new temporary
    /// directory.
    fn new_sim() -> (Simulacrum, TempDir) {
        let tmp_dir = iota_common::tempdir();
        let sim = Simulacrum::new();
        sim.set_data_ingestion_path(tmp_dir.path().to_path_buf());
        (sim, tmp_dir)
    }

    /// Starts a writer indexer on [`DB_NAME`] that ingests the checkpoints of
    /// `sim`.
    async fn start_indexer(sim: Simulacrum, data_dir: &TempDir) -> PgIndexerStore {
        start_indexer_with(sim, data_dir, Some(DB_NAME), None).await
    }

    /// Starts a writer indexer that ingests the checkpoints of `sim`.
    async fn start_indexer_with(
        sim: Simulacrum,
        data_dir: &TempDir,
        database_name: Option<&str>,
        db_init_hook: Option<DBInitHook>,
    ) -> PgIndexerStore {
        let (_, pg_store, _) = start_simulacrum_grpc_with_write_indexer(
            Arc::new(sim),
            data_dir.path().to_path_buf(),
            None,
            database_name,
            db_init_hook,
        )
        .await;
        pg_store
    }

    /// Executes a transfer of a new coin to a random address and asserts
    /// success.
    ///
    /// Returns the transaction, its effects, and the reference of the coin
    /// that it creates.
    fn execute_transfer(
        sim: &Simulacrum,
    ) -> (TransactionEnvelope, TransactionEffects, ObjectReference) {
        let (transaction, _) = sim.transfer_txn(Address::random());
        let (effects, err) = sim.execute_transaction(transaction.clone()).unwrap();
        assert!(err.is_none(), "tx failed: {err:?}");
        let created = effects.created();
        assert_eq!(
            created.len(),
            1,
            "transfer_txn should create exactly 1 coin"
        );
        let created_coin = *created[0].reference();
        (transaction, effects, created_coin)
    }

    /// Returns the reference of the object `id` in `refs`.
    fn find_object_ref<'a>(
        refs: impl IntoIterator<Item = &'a ObjectReference>,
        id: ObjectId,
    ) -> ObjectReference {
        *refs
            .into_iter()
            .find(|r| r.object_id == id)
            .unwrap_or_else(|| panic!("object {id:?} not found"))
    }

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
    pub async fn checkpoint_objects_ingestion() -> Result<(), IndexerError> {
        let (sim, tmp_dir) = new_sim();
        let pg_store = start_indexer(sim, &tmp_dir).await;

        let checkpoint_objects = (0..1000)
            .map(|_| CheckpointObjectChanges::random())
            .collect();
        pg_store.persist_objects(checkpoint_objects).await?;
        Ok(())
    }

    #[tokio::test]
    pub async fn transaction_table() -> Result<(), IndexerError> {
        let (sim, tmp_dir) = new_sim();
        let (transaction, effects, _) = execute_transfer(&sim);
        // Create a checkpoint which should include the transaction we executed.
        let checkpoint = sim.create_checkpoint();

        let pg_store = start_indexer(sim, &tmp_dir).await;
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
    pub async fn object_type() -> Result<(), IndexerError> {
        let (sim, tmp_dir) = new_sim();
        let (transaction, ..) = execute_transfer(&sim);
        sim.create_checkpoint();

        let pg_store = start_indexer(sim, &tmp_dir).await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;

        let obj_id = transaction.gas()[0].object_id;

        // Read the object from the database directly.
        let db_object: StoredObject = read_only_blocking!(&pg_store.blocking_cp(), |conn| {
            objects::table
                .filter(objects::object_id.eq(obj_id.as_bytes()))
                .first::<StoredObject>(conn)
        })
        .context("Failed reading object from PostgresDB")?;

        let obj_type_tag = StructTag::new_gas_coin();

        // Check that the different components of the object type were stored correctly.
        assert_eq!(
            db_object.object_type,
            Some(obj_type_tag.to_canonical_string(true))
        );
        assert_eq!(
            db_object.object_type_package,
            Some(Address::FRAMEWORK.as_bytes().to_vec())
        );
        assert_eq!(db_object.object_type_module, Some("coin".to_string()));
        assert_eq!(db_object.object_type_name, Some("Coin".to_string()));
        Ok(())
    }

    #[tokio::test]
    pub async fn tx_global_order_table() -> Result<(), IndexerError> {
        let (sim, tmp_dir) = new_sim();
        let (_, effects, _) = execute_transfer(&sim);
        sim.create_checkpoint();

        let pg_store = start_indexer(sim, &tmp_dir).await;
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
        let (sim, tmp_dir) = new_sim();
        let (_, effects, _) = execute_transfer(&sim);
        sim.create_checkpoint();
        let digest = *effects.transaction_digest();

        let pg_store = start_indexer_with(
            sim,
            &tmp_dir,
            Some(DB_NAME),
            Some(emulate_insertion_order_set_earlier_by_optimistic_indexing(
                digest,
            )),
        )
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
        let (sim, tmp_dir) = new_sim();
        let pg_store =
            start_indexer_with(sim, &tmp_dir, Some("indexer_ingestion_tx_indices_db"), None).await;

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
        let (sim, tmp_dir) = new_sim();
        let pg_store = start_indexer_with(
            sim,
            &tmp_dir,
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
        let (sim, tmp_dir) = new_sim();
        execute_transfer(&sim);
        sim.create_checkpoint();

        let pg_store = start_indexer(sim, &tmp_dir).await;
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
        let (sim, tmp_dir) = new_sim();
        execute_transfer(&sim);
        sim.create_checkpoint(); // checkpoint 1
        sim.advance_epoch(false); // checkpoint 2 and epoch 1
        execute_transfer(&sim);
        sim.create_checkpoint(); // checkpoint 3

        let pg_store = start_indexer_with(sim, &tmp_dir, None, None).await;
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
        let (sim, tmp_dir) = new_sim();

        // Execute several transfers to create mutations on the gas object.
        for _ in 0..3 {
            execute_transfer(&sim);
            sim.create_checkpoint();
        }

        let pg_store = start_indexer(sim, &tmp_dir).await;
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

    /// Asserts that `coin` has a NOT_YET_CREATED entry in
    /// `objects_backward_history` at `cp`, with object_version = lamport - 1
    /// (the version assigned by the create tx, minus one).
    async fn assert_not_yet_created(
        pg_store: &PgIndexerStore,
        coin: ObjectReference,
        cp: i64,
    ) -> Result<(), IndexerError> {
        let entry = find_backward_entry(pg_store, coin.object_id.as_bytes(), cp)
            .await?
            .unwrap_or_else(|| {
                panic!("created coin {coin:?} must have a backward history entry at cp {cp}")
            });
        assert_eq!(entry.object_status, ObjectStatus::NotYetCreated as i16);
        assert_eq!(entry.object_version, coin.version.as_u64() as i64 - 1);
        assert!(entry.serialized_object.is_none());
        assert!(entry.object_digest.is_none());
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
        let (sim, tmp_dir) = new_sim();

        // --- checkpoint 1: TWO transfers in the same checkpoint ---
        // This exercises the case where multiple transactions produce backward
        // history entries for the same checkpoint, and where the same object
        // (the gas coin) is mutated by both transactions within one checkpoint.
        let (tx1, _, created_coin_1) = execute_transfer(&sim);
        let gas_object_id = tx1.gas()[0].object_id;
        let gas_version_before_tx1 = tx1.gas()[0].version;

        // Second transfer in the same checkpoint — uses the same gas object
        // (now at an updated version).
        let (tx2, _, created_coin_2) = execute_transfer(&sim);
        let gas_version_before_tx2 = tx2.gas()[0].version;
        assert!(
            gas_version_before_tx2 > gas_version_before_tx1,
            "gas should have been bumped after tx1"
        );

        // Both transactions land in checkpoint 1.
        sim.create_checkpoint();

        // --- checkpoint 2: one more transfer ---
        let (tx3, _, created_coin_3) = execute_transfer(&sim);
        let gas_version_before_tx3 = tx3.gas()[0].version;
        sim.create_checkpoint();

        let pg_store = start_indexer(sim, &tmp_dir).await;
        indexer_wait_for_checkpoint(&pg_store, 2).await;

        // === checkpoint 1 assertions (two transactions) ===

        assert_not_yet_created(&pg_store, created_coin_1, 1).await?;
        assert_not_yet_created(&pg_store, created_coin_2, 1).await?;

        // The gas object was mutated twice in checkpoint 1 — there should be
        // two ACTIVE entries with different previous versions.
        let gas_entries =
            find_all_entries_at_checkpoint(&pg_store, gas_object_id.as_bytes(), 1).await?;
        assert_eq!(
            gas_entries.len(),
            2,
            "gas object should have 2 backward history entries in cp 1 (one per tx)"
        );
        assert_eq!(gas_entries[0].object_status, ObjectStatus::Active as i16);
        assert_eq!(
            gas_entries[0].object_version,
            gas_version_before_tx1.as_u64() as i64
        );
        assert!(gas_entries[0].serialized_object.is_some());
        assert!(gas_entries[0].object_digest.is_some());
        assert!(gas_entries[0].owner_type.is_some());

        assert_eq!(gas_entries[1].object_status, ObjectStatus::Active as i16);
        assert_eq!(
            gas_entries[1].object_version,
            gas_version_before_tx2.as_u64() as i64
        );
        assert!(gas_entries[1].serialized_object.is_some());

        // === checkpoint 2 assertions (single transaction) ===

        assert_not_yet_created(&pg_store, created_coin_3, 2).await?;

        let entry = find_backward_entry(&pg_store, gas_object_id.as_bytes(), 2)
            .await?
            .expect("gas object must have a backward history entry at cp 2");
        assert_eq!(entry.object_status, ObjectStatus::Active as i16);
        assert_eq!(entry.object_version, gas_version_before_tx3.as_u64() as i64);
        assert!(entry.serialized_object.is_some());

        Ok(())
    }

    /// Executes a transfer and returns the rows that it must add to
    /// `objects_version` at checkpoint `cp`.
    fn execute_transfer_version_rows(sim: &Simulacrum, cp: i64) -> [StoredObjectVersion; 2] {
        let (tx, effects, coin) = execute_transfer(sim);
        let gas = find_object_ref(
            effects
                .mutated()
                .iter()
                .map(OwnedObjectReference::reference),
            tx.gas()[0].object_id,
        );
        [gas, coin].map(|r| StoredObjectVersion {
            object_id: r.object_id.as_bytes().to_vec(),
            object_version: r.version.as_u64() as i64,
            cp_sequence_number: cp,
        })
    }

    /// Verify that `objects_version` is populated correctly: one row per output
    /// object per transaction. Tests repeated mutations of the same object in a
    /// single checkpoint.
    #[tokio::test]
    pub async fn object_versions_ingestion() -> Result<(), IndexerError> {
        let (sim, tmp_dir) = new_sim();

        // checkpoint 1: gas mutated twice + two coins created
        let mut cp1_expected = [
            execute_transfer_version_rows(&sim, 1),
            execute_transfer_version_rows(&sim, 1),
        ]
        .concat();
        sim.create_checkpoint();

        // checkpoint 2: gas mutated once + one coin created
        let mut cp2_expected = execute_transfer_version_rows(&sim, 2).to_vec();
        sim.create_checkpoint();

        let pg_store = start_indexer(sim, &tmp_dir).await;
        indexer_wait_for_checkpoint(&pg_store, 2).await;

        cp1_expected.sort();
        assert_eq!(
            find_object_versions_at_checkpoint(&pg_store, 1).await?,
            cp1_expected
        );
        cp2_expected.sort();
        assert_eq!(
            find_object_versions_at_checkpoint(&pg_store, 2).await?,
            cp2_expected
        );

        Ok(())
    }

    /// Executes transaction in simulacrum, asserts success and returns effects.
    fn execute_signed(sim: &Simulacrum, tx: Transaction) -> TransactionEffects {
        let (sender, key) = sim.with_keystore(|ks| {
            let (s, k) = ks.accounts().next().unwrap();
            (*s, k.clone())
        });
        assert_eq!(tx.sender(), sender);
        let tx = TransactionEnvelope::from_data_and_signer(tx, vec![&key]);
        let (effects, err) = sim.execute_transaction(tx).unwrap();
        assert!(err.is_none(), "tx failed: {err:?}");
        effects
    }

    /// Returns the reference of a gas coin that `sender` owns.
    fn pick_gas(sim: &Simulacrum, sender: Address) -> ObjectReference {
        sim.with_store(|s| {
            s.owned_objects(sender)
                .find(|o| o.is_gas_coin())
                .unwrap()
                .object_ref()
        })
    }

    /// Adds a call to `function` of the `example` module in `package_id`.
    fn move_call(
        builder: &mut ProgrammableTransactionBuilder,
        package_id: ObjectId,
        function: &'static str,
        arguments: Vec<Argument>,
    ) -> Argument {
        builder.programmable_move_call(
            package_id,
            Identifier::from_static("example"),
            Identifier::from_static(function),
            vec![],
            arguments,
        )
    }

    /// Adds the owned object `object` as an input.
    fn owned_input(
        builder: &mut ProgrammableTransactionBuilder,
        object: ObjectReference,
    ) -> Argument {
        builder.obj(CallArg::ImmutableOrOwned(object)).unwrap()
    }

    /// Asserts that `objects_version` has a row for `id` at `version` in
    /// checkpoint `cp`.
    async fn assert_present(
        pg_store: &PgIndexerStore,
        cp: i64,
        id: ObjectId,
        version: Version,
        label: &str,
    ) -> Result<(), IndexerError> {
        let rows = find_object_versions_at_checkpoint(pg_store, cp).await?;
        assert!(
            rows.iter().any(|r| r.object_id == id.as_bytes().to_vec()
                && r.object_version == version.as_u64() as i64),
            "expected {label} in cp{cp}; rows: {rows:?}",
        );
        Ok(())
    }

    /// Verify `objects_version` ingestion for every wrap/removal effect,
    /// namely: `wrapped`, `unwrapped`, `deleted`, and
    /// `unwrapped_then_deleted`.
    #[tokio::test]
    pub async fn objects_version_ingestion_wraps_and_removals() -> Result<(), IndexerError> {
        let (sim, tmp_dir) = new_sim();
        let sender = sim.with_keystore(|ks| *ks.accounts().next().unwrap().0);
        let rgp = sim.reference_gas_price();

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
            .find(|created| matches!(created.owner(), Owner::Immutable))
            .expect("publish must create an immutable package")
            .reference()
            .object_id;
        sim.create_checkpoint();

        // mint a Sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let strength = builder.pure(42u8).unwrap();
        let sword_arg = move_call(&mut builder, package_id, "new_sword", vec![strength]);
        builder.transfer_arg(sender, sword_arg);
        let mint_sword_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let sword_v0 = *mint_sword_fx
            .created()
            .into_iter()
            .find(|created| matches!(created.owner(), Owner::Address(_)))
            .expect("mint must create the sword")
            .reference();
        sim.create_checkpoint();

        // cp3: mint a Warrior + equip sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_arg = move_call(&mut builder, package_id, "new_warrior", vec![]);
        let sword_input = owned_input(&mut builder, sword_v0);
        move_call(
            &mut builder,
            package_id,
            "equip",
            vec![warrior_arg, sword_input],
        );
        builder.transfer_arg(sender, warrior_arg);
        let equip_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let warrior_v0 = *equip_fx
            .created()
            .into_iter()
            .find(|created| matches!(created.owner(), Owner::Address(_)))
            .expect("equip tx must create the warrior")
            .reference();
        let sword_wrapped_version =
            find_object_ref(&equip_fx.wrapped(), sword_v0.object_id).version;
        sim.create_checkpoint();

        // cp4: unequip sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = owned_input(&mut builder, warrior_v0);
        let unwrapped_sword_arg =
            move_call(&mut builder, package_id, "unequip", vec![warrior_input]);
        builder.transfer_arg(sender, unwrapped_sword_arg);
        let unequip_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let sword_unwrapped_ref = find_object_ref(
            unequip_fx
                .unwrapped()
                .iter()
                .map(OwnedObjectReference::reference),
            sword_v0.object_id,
        );
        let warrior_after_unequip = find_object_ref(
            unequip_fx
                .mutated()
                .iter()
                .map(OwnedObjectReference::reference),
            warrior_v0.object_id,
        );
        sim.create_checkpoint();

        // cp5: equip sword
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = owned_input(&mut builder, warrior_after_unequip);
        let sword_input = owned_input(&mut builder, sword_unwrapped_ref);
        move_call(
            &mut builder,
            package_id,
            "equip",
            vec![warrior_input, sword_input],
        );
        let reequip_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let sword_rewrapped_version =
            find_object_ref(&reequip_fx.wrapped(), sword_v0.object_id).version;
        let warrior_after_reequip = find_object_ref(
            reequip_fx
                .mutated()
                .iter()
                .map(OwnedObjectReference::reference),
            warrior_v0.object_id,
        );
        sim.create_checkpoint();

        // cp6: destroy warrior
        let gas = pick_gas(&sim, sender);
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = owned_input(&mut builder, warrior_after_reequip);
        move_call(
            &mut builder,
            package_id,
            "destroy_warrior",
            vec![warrior_input],
        );
        let destroy_fx = execute_signed(
            &sim,
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let warrior_deleted_version =
            find_object_ref(&destroy_fx.deleted(), warrior_v0.object_id).version;
        let sword_unwrapped_deleted_version =
            find_object_ref(&destroy_fx.unwrapped_then_deleted(), sword_v0.object_id).version;
        sim.create_checkpoint();

        // spin up the indexer
        let pg_store = start_indexer(sim, &tmp_dir).await;
        indexer_wait_for_checkpoint(&pg_store, 6).await;

        // cp3: warrior created (output) + sword wrapped (removed)
        assert_present(
            &pg_store,
            3,
            warrior_v0.object_id,
            warrior_v0.version,
            "created warrior",
        )
        .await?;
        assert_present(
            &pg_store,
            3,
            sword_v0.object_id,
            sword_wrapped_version,
            "wrapped sword",
        )
        .await?;
        // cp4: sword unwrapped (output)
        assert_present(
            &pg_store,
            4,
            sword_v0.object_id,
            sword_unwrapped_ref.version,
            "unwrapped sword",
        )
        .await?;
        // cp5: sword re-wrapped (removed)
        assert_present(
            &pg_store,
            5,
            sword_v0.object_id,
            sword_rewrapped_version,
            "re-wrapped sword",
        )
        .await?;
        // cp6: warrior deleted + sword unwrapped_then_deleted (both removed)
        assert_present(
            &pg_store,
            6,
            warrior_v0.object_id,
            warrior_deleted_version,
            "deleted warrior",
        )
        .await?;
        assert_present(
            &pg_store,
            6,
            sword_v0.object_id,
            sword_unwrapped_deleted_version,
            "unwrapped_then_deleted sword",
        )
        .await?;

        Ok(())
    }
}
