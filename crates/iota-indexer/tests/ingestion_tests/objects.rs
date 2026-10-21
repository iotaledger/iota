// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
use iota_indexer::{
    errors::{Context, IndexerError},
    ingestion::{
        OperationalLevel::{self, Basic, CombinedEventFilters, FilteredQueries, ObjectsHistory},
        common::prepare::CheckpointObjectChanges,
    },
    models::{
        obj_indices::StoredObjectVersion,
        objects::{StoredCheckpointedObject, StoredObject},
    },
    schema::{checkpointed_objects, display, objects, objects_backward_history, packages},
    store::{PgIndexerStore, indexer_store::IndexerStore},
    types::ObjectStatus,
};
use iota_sdk_types::{
    Address, Argument, Identifier, ObjectId, ObjectReference, OwnedObjectReference, Owner,
    StructTag, Version,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    effects::TransactionEffectsAPI,
    programmable_transaction_builder::ProgrammableTransactionBuilder, transaction::CallArg,
};
use rstest::rstest;

use crate::{
    common::{
        backward_history::{find_all_entries_at_checkpoint, find_backward_entry},
        indexer_wait_for_checkpoint,
        object_versions::find_object_versions_at_checkpoint,
    },
    utils::{
        IngestionTest, TestSim, assert_row_count, find_object_ref, read_only_blocking,
        start_indexer_at_genesis,
    },
};

#[tokio::test]
pub async fn checkpoint_objects_ingestion() -> Result<(), IndexerError> {
    let sim = TestSim::new(OperationalLevel::default(), "ingestion_checkpoint_objects");
    let pg_store = sim.start_indexer(None).await;

    let checkpoint_objects = (0..1000)
        .map(|_| CheckpointObjectChanges::random())
        .collect();
    pg_store.persist_objects(checkpoint_objects).await?;
    Ok(())
}

#[rstest]
#[tokio::test]
pub async fn object_type(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    ObjectType.run(level, "ingestion_object_type").await
}

#[rstest]
#[tokio::test]
pub async fn checkpoint_objects_are_finalized(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    CheckpointObjectsAreFinalized
        .run(level, "ingestion_checkpoint_objects_are_finalized")
        .await
}

/// Verify that `checkpointed_objects` matches `objects` (minus
/// `finalized_in_cp`) after checkpoint ingestion with mutations and
/// deletions.
#[rstest]
#[tokio::test]
pub async fn checkpointed_objects_match_objects(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    CheckpointedObjectsMatchObjects
        .run(level, "ingestion_checkpointed_objects_match_objects")
        .await
}

#[rstest]
#[tokio::test]
pub async fn backward_history_ingestion(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    BackwardHistoryIngestion
        .run(level, "ingestion_backward_history")
        .await
}

/// Verify that `objects_version` is populated correctly: one row per output
/// object per transaction. Tests repeated mutations of the same object in a
/// single checkpoint.
#[rstest]
#[tokio::test]
pub async fn object_versions_ingestion(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    ObjectVersionsIngestion
        .run(level, "ingestion_object_versions")
        .await
}

/// Verify `objects_version` ingestion for every wrap/removal effect,
/// namely: `wrapped`, `unwrapped`, `deleted`, and
/// `unwrapped_then_deleted`.
#[rstest]
#[tokio::test]
pub async fn objects_version_ingestion_wraps_and_removals(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    ObjectsVersionIngestionWrapsAndRemovals
        .run(level, "ingestion_objects_version_wraps_and_removals")
        .await
}

#[rstest]
#[tokio::test]
pub async fn displays_and_packages(
    #[values(Basic, ObjectsHistory, FilteredQueries, CombinedEventFilters)] level: OperationalLevel,
) -> Result<(), IndexerError> {
    let pg_store = start_indexer_at_genesis(level, "ingestion_displays_and_packages").await;
    assert_row_count!(&pg_store, display, true).await?;
    assert_row_count!(&pg_store, packages, true).await
}

struct ObjectType;

impl IngestionTest for ObjectType {
    type ExpectedState = ObjectId;

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        let (transaction, ..) = sim.execute_transfer();
        sim.inner.create_checkpoint();

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;
        (pg_store, transaction.gas()[0].object_id)
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        obj_id: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
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
}

struct CheckpointObjectsAreFinalized;

impl IngestionTest for CheckpointObjectsAreFinalized {
    type ExpectedState = ();

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        sim.execute_transfer();
        sim.inner.create_checkpoint();

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 1).await;
        (pg_store, ())
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        _: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        let max_cp = IndexerStore::get_latest_checkpoint_sequence_number(pg_store)
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
}

struct CheckpointedObjectsMatchObjects;

impl IngestionTest for CheckpointedObjectsMatchObjects {
    type ExpectedState = ();

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        // Execute several transfers to create mutations on the gas object.
        for _ in 0..3 {
            sim.execute_transfer();
            sim.inner.create_checkpoint();
        }

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 3).await;
        (pg_store, ())
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        _: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
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
}

struct BackwardHistoryIngestion;

struct BackwardHistoryExpected {
    /// The gas object that all transfers use.
    gas_object_id: ObjectId,
    /// The input version of the gas object in each transfer.
    gas_input_versions: [Version; 3],
    /// The coin that each transfer creates.
    created_coins: [ObjectReference; 3],
}

impl IngestionTest for BackwardHistoryIngestion {
    const MIN_LEVEL: OperationalLevel = ObjectsHistory;

    type ExpectedState = BackwardHistoryExpected;

    /// Executes three transfers: two in checkpoint 1 and one in checkpoint 2.
    ///
    /// Each transfer splits a coin from the gas object and transfers it, so
    /// it adds two entries to `objects_backward_history`:
    ///   - the created coin → a NOT_YET_CREATED entry (version = lamport - 1)
    ///   - the mutated gas object → an ACTIVE entry with the previous version and data
    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        // --- checkpoint 1: TWO transfers in the same checkpoint ---
        // This exercises the case where multiple transactions produce backward
        // history entries for the same checkpoint, and where the same object
        // (the gas coin) is mutated by both transactions within one checkpoint.
        let (tx1, _, created_coin_1) = sim.execute_transfer();
        let gas_object_id = tx1.gas()[0].object_id;
        let gas_version_before_tx1 = tx1.gas()[0].version;

        // Second transfer in the same checkpoint — uses the same gas object
        // (now at an updated version).
        let (tx2, _, created_coin_2) = sim.execute_transfer();
        let gas_version_before_tx2 = tx2.gas()[0].version;
        assert!(
            gas_version_before_tx2 > gas_version_before_tx1,
            "gas should have been bumped after tx1"
        );

        // Both transactions land in checkpoint 1.
        sim.inner.create_checkpoint();

        // --- checkpoint 2: one more transfer ---
        let (tx3, _, created_coin_3) = sim.execute_transfer();
        let gas_version_before_tx3 = tx3.gas()[0].version;
        sim.inner.create_checkpoint();

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 2).await;
        (
            pg_store,
            BackwardHistoryExpected {
                gas_object_id,
                gas_input_versions: [
                    gas_version_before_tx1,
                    gas_version_before_tx2,
                    gas_version_before_tx3,
                ],
                created_coins: [created_coin_1, created_coin_2, created_coin_3],
            },
        )
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        expected: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        let BackwardHistoryExpected {
            gas_object_id,
            gas_input_versions:
                [
                    gas_version_before_tx1,
                    gas_version_before_tx2,
                    gas_version_before_tx3,
                ],
            created_coins: [created_coin_1, created_coin_2, created_coin_3],
        } = expected;

        // === checkpoint 1 assertions (two transactions) ===

        assert_not_yet_created(pg_store, created_coin_1, 1).await?;
        assert_not_yet_created(pg_store, created_coin_2, 1).await?;

        // The gas object was mutated twice in checkpoint 1 — there should be
        // two ACTIVE entries with different previous versions.
        let gas_entries =
            find_all_entries_at_checkpoint(pg_store, gas_object_id.as_bytes(), 1).await?;
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

        assert_not_yet_created(pg_store, created_coin_3, 2).await?;

        let entry = find_backward_entry(pg_store, gas_object_id.as_bytes(), 2)
            .await?
            .expect("gas object must have a backward history entry at cp 2");
        assert_eq!(entry.object_status, ObjectStatus::Active as i16);
        assert_eq!(entry.object_version, gas_version_before_tx3.as_u64() as i64);
        assert!(entry.serialized_object.is_some());

        Ok(())
    }

    async fn assert_not_ingested(&self, pg_store: &PgIndexerStore) -> Result<(), IndexerError> {
        assert_row_count!(pg_store, objects_backward_history, false).await
    }
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

struct ObjectVersionsIngestion;

impl IngestionTest for ObjectVersionsIngestion {
    /// The sorted rows of checkpoints 1 and 2.
    type ExpectedState = (Vec<StoredObjectVersion>, Vec<StoredObjectVersion>);

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        // checkpoint 1: gas mutated twice + two coins created
        let mut cp1_expected = [
            sim.execute_transfer_version_rows(1),
            sim.execute_transfer_version_rows(1),
        ]
        .concat();
        sim.inner.create_checkpoint();

        // checkpoint 2: gas mutated once + one coin created
        let mut cp2_expected = sim.execute_transfer_version_rows(2).to_vec();
        sim.inner.create_checkpoint();

        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 2).await;

        cp1_expected.sort();
        cp2_expected.sort();
        (pg_store, (cp1_expected, cp2_expected))
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        (cp1_expected, cp2_expected): Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        assert_eq!(
            find_object_versions_at_checkpoint(pg_store, 1).await?,
            cp1_expected
        );
        assert_eq!(
            find_object_versions_at_checkpoint(pg_store, 2).await?,
            cp2_expected
        );
        Ok(())
    }
}

pub(crate) struct ObjectsVersionIngestionWrapsAndRemovals;

impl IngestionTest for ObjectsVersionIngestionWrapsAndRemovals {
    /// The `objects_version` rows to find, as the checkpoint, the object, its
    /// version and a label for the assert message.
    type ExpectedState = Vec<(i64, ObjectId, Version, &'static str)>;

    async fn setup(
        &self,
        level: OperationalLevel,
        database_name: &str,
    ) -> (PgIndexerStore, Self::ExpectedState) {
        let sim = TestSim::new(level, database_name);
        let sender = sim.sender();
        let rgp = sim.inner.reference_gas_price();

        // publish package
        let gas = sim.pick_gas();
        let publish_fx = sim.execute_signed(
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
        sim.inner.create_checkpoint();

        // mint a Sword
        let gas = sim.pick_gas();
        let mut builder = ProgrammableTransactionBuilder::new();
        let strength = builder.pure(42u8).unwrap();
        let sword_arg = move_call(&mut builder, package_id, "new_sword", vec![strength]);
        builder.transfer_arg(sender, sword_arg);
        let mint_sword_fx = sim.execute_signed(
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
        sim.inner.create_checkpoint();

        // cp3: mint a Warrior + equip sword
        let gas = sim.pick_gas();
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
        let equip_fx = sim.execute_signed(
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
        sim.inner.create_checkpoint();

        // cp4: unequip sword
        let gas = sim.pick_gas();
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = owned_input(&mut builder, warrior_v0);
        let unwrapped_sword_arg =
            move_call(&mut builder, package_id, "unequip", vec![warrior_input]);
        builder.transfer_arg(sender, unwrapped_sword_arg);
        let unequip_fx = sim.execute_signed(
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
        sim.inner.create_checkpoint();

        // cp5: equip sword
        let gas = sim.pick_gas();
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = owned_input(&mut builder, warrior_after_unequip);
        let sword_input = owned_input(&mut builder, sword_unwrapped_ref);
        move_call(
            &mut builder,
            package_id,
            "equip",
            vec![warrior_input, sword_input],
        );
        let reequip_fx = sim.execute_signed(
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
        sim.inner.create_checkpoint();

        // cp6: destroy warrior
        let gas = sim.pick_gas();
        let mut builder = ProgrammableTransactionBuilder::new();
        let warrior_input = owned_input(&mut builder, warrior_after_reequip);
        move_call(
            &mut builder,
            package_id,
            "destroy_warrior",
            vec![warrior_input],
        );
        let destroy_fx = sim.execute_signed(
            TestTransactionBuilder::new(sender, gas, rgp)
                .programmable(builder.finish())
                .build(),
        );
        let warrior_deleted_version =
            find_object_ref(&destroy_fx.deleted(), warrior_v0.object_id).version;
        let sword_unwrapped_deleted_version =
            find_object_ref(&destroy_fx.unwrapped_then_deleted(), sword_v0.object_id).version;
        sim.inner.create_checkpoint();

        // spin up the indexer
        let pg_store = sim.start_indexer(None).await;
        indexer_wait_for_checkpoint(&pg_store, 6).await;

        let expected = vec![
            // cp3: warrior created (output) + sword wrapped (removed)
            (
                3,
                warrior_v0.object_id,
                warrior_v0.version,
                "created warrior",
            ),
            (
                3,
                sword_v0.object_id,
                sword_wrapped_version,
                "wrapped sword",
            ),
            // cp4: sword unwrapped (output)
            (
                4,
                sword_v0.object_id,
                sword_unwrapped_ref.version,
                "unwrapped sword",
            ),
            // cp5: sword re-wrapped (removed)
            (
                5,
                sword_v0.object_id,
                sword_rewrapped_version,
                "re-wrapped sword",
            ),
            // cp6: warrior deleted + sword unwrapped_then_deleted (both removed)
            (
                6,
                warrior_v0.object_id,
                warrior_deleted_version,
                "deleted warrior",
            ),
            (
                6,
                sword_v0.object_id,
                sword_unwrapped_deleted_version,
                "unwrapped_then_deleted sword",
            ),
        ];
        (pg_store, expected)
    }

    async fn assert_ingested(
        &self,
        pg_store: &PgIndexerStore,
        expected: Self::ExpectedState,
    ) -> Result<(), IndexerError> {
        for (cp, id, version, label) in expected {
            assert_present(pg_store, cp, id, version, label).await?;
        }
        Ok(())
    }
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
fn owned_input(builder: &mut ProgrammableTransactionBuilder, object: ObjectReference) -> Argument {
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
