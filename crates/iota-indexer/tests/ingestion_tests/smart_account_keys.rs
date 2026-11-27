// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Ingestion of `smart_account_keys`: one row per public key attached to a
//! `SmartAccount`, so an account can be found by the `key_id` of its key.
//!
//! A `SmartAccount` can only change its key by sending a transaction itself,
//! which Simulacrum does not support, so the key rotation and removal cases
//! run on a test cluster.

use std::time::Duration;

use diesel::{QueryDsl, RunQueryDsl, connection::BoxableConnection};
use iota_indexer::{
    errors::{Context, IndexerError},
    models::smart_account_keys::StoredSmartAccountKey,
    schema::smart_account_keys,
    store::{PgIndexerStore, diesel_macro::spawn_blocking_task},
    transactional_blocking_with_retry,
};
use iota_sdk_crypto::simple::SimpleKeypair;
use iota_sdk_types::{Identifier, ObjectId, ObjectReference, Owner, Transaction};
use iota_test_transaction_builder::{TestTransactionBuilder, publish_package};
use iota_types::{
    effects::TransactionEffectsAPI,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI},
};
use test_cluster::smart_account::{
    CUSTOM_AUTHENTICATOR_FUNCTION, CUSTOM_AUTHENTICATOR_MODULE, chain_identifier,
    create_account_tx, created_account, created_key_field, custom_authenticator_package_path,
    ed25519_key, key_id, mutated_object, public_key_argument, rotate_public_key,
    rotate_to_custom_authenticator, send_from_account,
};

use crate::{
    common::{
        indexer_wait_for_checkpoint, indexer_wait_for_transaction,
        start_test_cluster_with_read_write_indexer,
    },
    utils::{TestSim, read_only_blocking},
};

/// Creating a `SmartAccount` with a key attached adds the row of its key field.
#[tokio::test]
pub async fn key_of_a_new_smart_account_is_indexed() -> Result<(), IndexerError> {
    let sim = TestSim::new("ingestion_smart_account_keys");
    let key = ed25519_key(1);
    let effects = sim.execute_signed(create_account_tx(
        &sim.chain_identifier(),
        sim.sender(),
        sim.pick_gas(),
        sim.inner.reference_gas_price(),
        &key,
    ));
    let account = created_account(&effects);
    let field = created_key_field(&effects, account.object_id);
    sim.inner.create_checkpoint();

    let pg_store = sim.start_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;
    assert_eq!(
        stored_keys(&pg_store).await?,
        vec![expected_row(field, account.object_id, &key)]
    );
    Ok(())
}

/// A key field attached to an object that is not a `SmartAccount` gets no row.
#[tokio::test]
pub async fn key_under_another_object_is_not_indexed() -> Result<(), IndexerError> {
    let sim = TestSim::new("ingestion_smart_account_keys_other_parent");
    create_holder_with_key(&sim, &ed25519_key(2));
    sim.inner.create_checkpoint();

    let pg_store = sim.start_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;
    assert_eq!(stored_keys(&pg_store).await?, vec![]);
    Ok(())
}

/// Filling the table from `objects`, as the snapshot restore does, gives the
/// same rows as ingestion.
#[tokio::test]
pub async fn keys_filled_from_objects_match_ingestion() -> Result<(), IndexerError> {
    let sim = TestSim::new("ingestion_smart_account_keys_fill_from_objects");
    let key = ed25519_key(5);
    let effects = sim.execute_signed(create_account_tx(
        &sim.chain_identifier(),
        sim.sender(),
        sim.pick_gas(),
        sim.inner.reference_gas_price(),
        &key,
    ));
    let account = created_account(&effects);
    let field = created_key_field(&effects, account.object_id);
    create_holder_with_key(&sim, &ed25519_key(6));
    sim.inner.create_checkpoint();

    let pg_store = sim.start_indexer(None).await;
    indexer_wait_for_checkpoint(&pg_store, 1).await;
    let ingested = stored_keys(&pg_store).await?;
    assert_eq!(ingested, vec![expected_row(field, account.object_id, &key)]);

    delete_all_keys(&pg_store).await?;
    pg_store.fill_smart_account_keys_from_objects().await?;
    assert_eq!(stored_keys(&pg_store).await?, ingested);
    Ok(())
}

/// Publishes the test package and creates an object of it that holds `key`
/// in a public key dynamic field.
fn create_holder_with_key(sim: &TestSim, key: &SimpleKeypair) {
    let sender = sim.sender();
    let rgp = sim.inner.reference_gas_price();
    let publish = sim.execute_signed(
        TestTransactionBuilder::new(sender, sim.pick_gas(), rgp)
            .publish(custom_authenticator_package_path())
            .build(),
    );
    let package_id = publish
        .created()
        .iter()
        .map(|created| created.reference().object_id)
        .find(|id| {
            sim.inner.with_store(|store| {
                store
                    .get_object(id)
                    .unwrap()
                    .data
                    .opt_object_type()
                    .is_none()
            })
        })
        .expect("the transaction publishes a package");

    let mut builder = ProgrammableTransactionBuilder::new();
    let public_key = public_key_argument(&mut builder, key);
    builder.programmable_move_call(
        package_id,
        Identifier::from_static(CUSTOM_AUTHENTICATOR_MODULE),
        Identifier::from_static("create_holder_with_key"),
        vec![],
        vec![public_key],
    );
    let effects = sim.execute_signed(Transaction::new_programmable(
        sender,
        vec![sim.pick_gas()],
        builder.finish(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    ));
    assert!(
        effects
            .created()
            .iter()
            .any(|created| matches!(created.owner(), Owner::Object(_))),
        "the holder gets a key field"
    );
}

/// A transaction the account sends leaves the row of its key field as it is,
/// rotating the key replaces the row, and detaching the key removes it.
#[tokio::test]
pub async fn key_rotation_and_removal_are_indexed() -> Result<(), IndexerError> {
    let (cluster, pg_store, client) = start_test_cluster_with_read_write_indexer(
        "ingestion_smart_account_keys_cluster",
        None,
        None,
    )
    .await;
    let chain_identifier = chain_identifier(&cluster);
    let sender = cluster.get_address_0();
    let rgp = cluster.get_reference_gas_price().await;

    let key = ed25519_key(3);
    let gas = cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let effects = cluster
        .execute_transaction(cluster.sign_transaction(&create_account_tx(
            &chain_identifier,
            sender,
            gas,
            rgp,
            &key,
        )))
        .await;
    let account = created_account(&effects);
    let field = created_key_field(&effects, account.object_id);
    indexer_wait_for_transaction(*effects.transaction_digest(), &pg_store, &client).await;
    let row = expected_row(field, account.object_id, &key);
    assert_eq!(stored_keys(&pg_store).await?, vec![row.clone()]);

    // The account is mutated by every transaction it sends, its key field is
    // not.
    let effects = send_from_account(&cluster, account, &key).await;
    indexer_wait_for_transaction(*effects.transaction_digest(), &pg_store, &client).await;
    assert_eq!(stored_keys(&pg_store).await?, vec![row]);

    let new_key = ed25519_key(4);
    let effects = rotate_public_key(&cluster, &chain_identifier, account, &key, &new_key).await;
    let field = mutated_object(&effects, field.object_id);
    indexer_wait_for_transaction(*effects.transaction_digest(), &pg_store, &client).await;
    assert_eq!(
        stored_keys(&pg_store).await?,
        vec![expected_row(field, account.object_id, &new_key)]
    );

    let package_id = publish_package(&cluster.wallet, custom_authenticator_package_path())
        .await
        .object_id;
    let effects = rotate_to_custom_authenticator(
        &cluster,
        account,
        &new_key,
        package_id,
        CUSTOM_AUTHENTICATOR_MODULE,
        CUSTOM_AUTHENTICATOR_FUNCTION,
    )
    .await;
    indexer_wait_for_transaction(*effects.transaction_digest(), &pg_store, &client).await;
    assert_eq!(stored_keys(&pg_store).await?, vec![]);
    Ok(())
}

/// Returns all rows of `smart_account_keys`.
async fn stored_keys(
    pg_store: &PgIndexerStore,
) -> Result<Vec<StoredSmartAccountKey>, IndexerError> {
    read_only_blocking!(&pg_store.blocking_cp(), |conn| {
        smart_account_keys::table
            .order(smart_account_keys::object_id)
            .load::<StoredSmartAccountKey>(conn)
    })
    .context("Failed reading smart account keys from PostgresDB")
}

/// Removes all rows of `smart_account_keys`.
async fn delete_all_keys(pg_store: &PgIndexerStore) -> Result<(), IndexerError> {
    let pool = pg_store.blocking_cp();
    spawn_blocking_task(move || {
        transactional_blocking_with_retry!(
            &pool,
            |conn| {
                diesel::delete(smart_account_keys::table).execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            Duration::from_secs(60)
        )
    })
    .await
    .expect("failed to join Tokio blocking task")
}

/// The row for the key field `field` of `account`, holding `key`.
fn expected_row(
    field: ObjectReference,
    account: ObjectId,
    key: &SimpleKeypair,
) -> StoredSmartAccountKey {
    StoredSmartAccountKey {
        object_id: field.object_id.as_bytes().to_vec(),
        object_version: field.version.as_u64() as i64,
        account_id: account.as_bytes().to_vec(),
        key_id: key_id(&key.public_key()).to_vec(),
    }
}
