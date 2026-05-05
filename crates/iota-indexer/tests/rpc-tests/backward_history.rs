// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `objects_backward_history` ingestion covering all
//! object lifecycle events: create, mutate, wrap, unwrap, delete, and
//! unwrap-then-delete.

use std::str::FromStr;

use iota_indexer::{models::objects::BackwardHistoryObjectStatus, store::PgIndexerStore};
use iota_json::{IotaJsonValue, call_args};
use iota_json_rpc_api::ReadApiClient;
use iota_json_rpc_types::{
    IotaTransactionBlockEffectsAPI, IotaTransactionBlockResponseOptions, ObjectChange,
};
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    crypto::{AccountKeyPair, IotaKeyPair, get_key_pair},
};
use jsonrpsee::http_client::HttpClient;

use crate::{
    coin_api::execute_move_call,
    common::{
        ApiTestSetup,
        backward_history::{find_all_entries_for_object, find_backward_entry},
        indexer_wait_for_object, indexer_wait_for_transaction, publish_test_move_package,
    },
};

/// Helper to call a function from the backward_history_test package, wait for
/// indexer to catch up, and return the response (with checkpoint populated).
pub async fn call_test_fn(
    client: &HttpClient,
    store: &PgIndexerStore,
    sender: iota_types::base_types::IotaAddress,
    keypair: &IotaKeyPair,
    package_id: ObjectID,
    function: &str,
    arguments: Vec<IotaJsonValue>,
    gas: Option<ObjectID>,
) -> iota_json_rpc_types::IotaTransactionBlockResponse {
    let resp = execute_move_call(
        client,
        sender,
        keypair,
        package_id,
        "backward_history_test".to_string(),
        function.to_string(),
        vec![],
        arguments,
        gas,
    )
    .await
    .unwrap();

    assert_eq!(
        resp.status_ok(),
        Some(true),
        "move call `{function}` failed: {resp:?}"
    );

    // Wait for the indexer to process this transaction, then re-fetch it from
    // the indexer so that the checkpoint field is populated.
    indexer_wait_for_transaction(resp.digest, store, client).await;
    client
        .get_transaction_block(
            resp.digest,
            Some(
                IotaTransactionBlockResponseOptions::new()
                    .with_object_changes()
                    .with_effects(),
            ),
        )
        .await
        .unwrap()
}

/// Extract the first created object ID from a transaction response.
pub fn first_created_id(resp: &iota_json_rpc_types::IotaTransactionBlockResponse) -> ObjectID {
    resp.object_changes
        .as_ref()
        .unwrap()
        .iter()
        .find_map(|c| match c {
            ObjectChange::Created { object_id, .. } => Some(*object_id),
            _ => None,
        })
        .expect("expected a created object")
}

/// Collect all created object IDs from a transaction response.
fn created_ids(resp: &iota_json_rpc_types::IotaTransactionBlockResponse) -> Vec<ObjectID> {
    resp.object_changes
        .as_ref()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            ObjectChange::Created { object_id, .. } => Some(*object_id),
            _ => None,
        })
        .collect()
}

/// Extract the version of an unwrapped object from a transaction response.
fn unwrapped_version(
    resp: &iota_json_rpc_types::IotaTransactionBlockResponse,
    object_id: ObjectID,
) -> SequenceNumber {
    resp.object_changes
        .as_ref()
        .unwrap()
        .iter()
        .find_map(|c| match c {
            ObjectChange::Unwrapped {
                object_id: id,
                version,
                ..
            } if *id == object_id => Some(*version),
            _ => None,
        })
        .expect("expected an unwrapped object")
}

/// Extract the version of an unwrapped-then-deleted object from effects.
fn unwrapped_then_deleted_version(
    resp: &iota_json_rpc_types::IotaTransactionBlockResponse,
    object_id: ObjectID,
) -> SequenceNumber {
    resp.effects
        .as_ref()
        .unwrap()
        .unwrapped_then_deleted()
        .iter()
        .find(|r| r.object_id == object_id)
        .expect("expected an unwrapped-then-deleted object")
        .version
}

#[test]
fn backward_history_all_lifecycle_events() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        // --- Set up a funded address ---
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let keypair = IotaKeyPair::Ed25519(keypair);
        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000_000),
                address,
            )
            .await;
        let gas_id = gas.object_id;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

        // --- Publish the test package ---
        let (package_ref, publish_resp) =
            publish_test_move_package(client, address, &keypair, "backward_history_test").await?;
        let package_id = package_ref.object_id;
        indexer_wait_for_transaction(publish_resp.digest, store, client).await;

        // ================================================================
        // Step 1: CREATE — create a new Item
        // ================================================================
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "create",
            call_args![42u64]?,
            Some(gas_id),
        )
        .await;
        let item_id = first_created_id(&resp);
        let create_cp = resp.checkpoint.unwrap() as i64;

        let entry = find_backward_entry(store, item_id.as_bytes(), create_cp)?
            .expect("item should have backward history at create checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::NotYetCreated as i16
        );
        // NotYetCreated rows are anchored at `lamport - 1` of the create tx.
        assert!(entry.object_version >= 0);
        assert!(entry.serialized_object.is_none());
        assert!(entry.object_digest.is_none());

        // ================================================================
        // Step 2: MUTATE — change the item's value
        // ================================================================
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "mutate",
            call_args![item_id, 99u64]?,
            Some(gas_id),
        )
        .await;
        let mutate_cp = resp.checkpoint.unwrap() as i64;

        let entry = find_backward_entry(store, item_id.as_bytes(), mutate_cp)?
            .expect("item should have backward history at mutate checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert!(
            entry.serialized_object.is_some(),
            "ACTIVE entry must have data"
        );
        assert!(entry.object_digest.is_some());
        assert!(entry.owner_type.is_some());
        assert!(entry.object_version > 0);

        // ================================================================
        // Step 3: WRAP — wrap the item inside a Box
        // ================================================================
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "wrap",
            call_args![item_id]?,
            Some(gas_id),
        )
        .await;
        let wrap_cp = resp.checkpoint.unwrap() as i64;
        let box_id = first_created_id(&resp);

        // Item was wrapped → ACTIVE backward entry with previous data.
        let entry = find_backward_entry(store, item_id.as_bytes(), wrap_cp)?
            .expect("item should have backward history at wrap checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert!(entry.serialized_object.is_some());

        // Box was created → NOT_YET_CREATED at `lamport - 1`.
        let entry = find_backward_entry(store, box_id.as_bytes(), wrap_cp)?
            .expect("box should have backward history at wrap checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::NotYetCreated as i16
        );
        assert!(entry.object_version >= 0);

        // ================================================================
        // Step 4: UNWRAP — unwrap the item from the Box
        // ================================================================
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "unwrap",
            call_args![box_id]?,
            Some(gas_id),
        )
        .await;
        let unwrap_cp = resp.checkpoint.unwrap() as i64;
        let item_unwrap_version = unwrapped_version(&resp, item_id);

        // Item was unwrapped → WRAPPED_OR_DELETED (no data available).
        // Version should be lamport - 1 (the output version minus one).
        let entry = find_backward_entry(store, item_id.as_bytes(), unwrap_cp)?
            .expect("item should have backward history at unwrap checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::WrappedOrDeleted as i16
        );
        assert_eq!(
            entry.object_version,
            item_unwrap_version.as_u64() as i64 - 1,
            "unwrapped entry should have lamport version - 1"
        );
        assert!(entry.serialized_object.is_none());
        assert!(entry.object_digest.is_none());

        // Box was deleted → ACTIVE backward entry with data.
        let entry = find_backward_entry(store, box_id.as_bytes(), unwrap_cp)?
            .expect("box should have backward history at unwrap checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert!(entry.serialized_object.is_some());

        // ================================================================
        // Step 5: DELETE — delete the item directly
        // ================================================================
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "delete",
            call_args![item_id]?,
            Some(gas_id),
        )
        .await;
        let delete_cp = resp.checkpoint.unwrap() as i64;

        // Item was deleted → ACTIVE backward entry with previous data.
        let entry = find_backward_entry(store, item_id.as_bytes(), delete_cp)?
            .expect("item should have backward history at delete checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert!(entry.serialized_object.is_some());

        // ================================================================
        // Step 6: UNWRAP-THEN-DELETE
        // ================================================================

        // 6a. Create a new item.
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "create",
            call_args![7u64]?,
            Some(gas_id),
        )
        .await;
        let item2_id = first_created_id(&resp);

        // 6b. Wrap it.
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "wrap",
            call_args![item2_id]?,
            Some(gas_id),
        )
        .await;
        let box2_id = first_created_id(&resp);

        // 6c. Unwrap-then-delete.
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "unwrap_and_delete",
            call_args![box2_id]?,
            Some(gas_id),
        )
        .await;
        let unwrap_delete_cp = resp.checkpoint.unwrap() as i64;

        // Box was deleted → ACTIVE backward entry.
        let entry = find_backward_entry(store, box2_id.as_bytes(), unwrap_delete_cp)?
            .expect("box2 should have backward history at unwrap_and_delete checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert!(entry.serialized_object.is_some());

        // Item inside was unwrapped-then-deleted → WRAPPED_OR_DELETED.
        let item2_utd_version = unwrapped_then_deleted_version(&resp, item2_id);
        let entry = find_backward_entry(store, item2_id.as_bytes(), unwrap_delete_cp)?
            .expect("item2 should have backward history at unwrap_and_delete checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::WrappedOrDeleted as i16
        );
        assert_eq!(
            entry.object_version,
            item2_utd_version.as_u64() as i64 - 1,
            "unwrapped-then-deleted entry should have lamport version - 1"
        );
        assert!(entry.serialized_object.is_none());
        assert!(entry.object_digest.is_none());

        // ================================================================
        // Verify full history chain for the first item.
        // ================================================================
        let all_entries = find_all_entries_for_object(store, item_id.as_bytes())?;
        assert_eq!(
            all_entries.len(),
            5,
            "item should have 5 backward history entries: create, mutate, wrap, unwrap, delete"
        );
        let statuses: Vec<i16> = all_entries.iter().map(|e| e.object_status).collect();
        assert_eq!(
            statuses,
            vec![
                BackwardHistoryObjectStatus::NotYetCreated as i16, // create
                BackwardHistoryObjectStatus::Active as i16,        // mutate
                BackwardHistoryObjectStatus::Active as i16,        // wrap
                BackwardHistoryObjectStatus::WrappedOrDeleted as i16, // unwrap
                BackwardHistoryObjectStatus::Active as i16,        // delete
            ]
        );

        Ok(())
    })
}

/// Exercises the case where a dynamic-object-field's underlying Field object
/// is created, deleted, and re-created with the same derived id within the
/// visible history window. Because the Field-object id is
/// `derive_dynamic_field_id(parent, type, name)` (deterministic), the
/// recreate produces the same id as the deletion targets.
///
/// We assert that F1's three backward-history rows carry distinct,
/// monotonically increasing `superseded_at_tx_sequence_number` values. That
/// finer time axis is what the version-pinned dynamic-field reader uses to
/// disambiguate intra-checkpoint transitions of the same id.
#[test]
fn backward_history_dof_delete_then_recreate() -> Result<(), anyhow::Error> {
    let ApiTestSetup {
        runtime,
        cluster,
        store,
        client,
    } = ApiTestSetup::get_or_init();

    runtime.block_on(async move {
        let (address, keypair): (_, AccountKeyPair) = get_key_pair();
        let keypair = IotaKeyPair::Ed25519(keypair);
        let gas = cluster
            .fund_address_and_return_gas(
                cluster.get_reference_gas_price().await,
                Some(500_000_000_000),
                address,
            )
            .await;
        let gas_id = gas.object_id;
        indexer_wait_for_object(client, gas.object_id, gas.version).await;

        let (package_ref, publish_resp) =
            publish_test_move_package(client, address, &keypair, "backward_history_test").await?;
        let package_id = package_ref.object_id;
        indexer_wait_for_transaction(publish_resp.digest, store, client).await;

        // 1. Create the parent.
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "create_parent",
            call_args![]?,
            Some(gas_id),
        )
        .await;
        let parent_id = first_created_id(&resp);

        // 2. Add DOF named 42 → creates Field F1 (derived id) and Child1.
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "add_dof",
            call_args![parent_id, 42u64, 1u64]?,
            Some(gas_id),
        )
        .await;
        let add1_created: Vec<ObjectID> = created_ids(&resp);

        // 3. Remove DOF 42 → deletes both F1 and Child1.
        let _ = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "remove_dof",
            call_args![parent_id, 42u64]?,
            Some(gas_id),
        )
        .await;

        // 4. Add DOF 42 again → re-creates F1 with the same derived id and a fresh
        //    Child2.
        let resp = call_test_fn(
            client,
            store,
            address,
            &keypair,
            package_id,
            "add_dof",
            call_args![parent_id, 42u64, 2u64]?,
            Some(gas_id),
        )
        .await;
        let add2_created: Vec<ObjectID> = created_ids(&resp);

        // 5. F1 = the id created by both add_dof calls (Child ids differ because they
        //    come from `object::new`).
        let f1_id: ObjectID = add1_created
            .iter()
            .find(|id| add2_created.contains(id))
            .copied()
            .expect("add_dof should re-create the same Field id on re-add");

        // 6. F1 has three rows in backward_history (NotYetCreated from the first
        //    create, Active from the remove tx's input, NotYetCreated from the
        //    recreate). Assert their `superseded_at_tx_sequence_number` values are
        //    distinct and monotonically increasing.
        let entries = find_all_entries_for_object(store, f1_id.as_bytes())?;

        assert_eq!(
            entries.len(),
            3,
            "F1 should have 3 backward-history rows (create, remove, recreate); got {:#?}",
            entries
                .iter()
                .map(|e| (
                    e.object_status,
                    e.object_version,
                    e.superseded_at_checkpoint,
                    e.superseded_at_tx_sequence_number,
                ))
                .collect::<Vec<_>>(),
        );

        let tx_seqs: Vec<i64> = entries
            .iter()
            .map(|e| e.superseded_at_tx_sequence_number)
            .collect();
        assert!(
            tx_seqs.windows(2).all(|w| w[0] < w[1]),
            "F1's superseded_at_tx_sequence_number values must be strictly increasing \
             across (create, remove, recreate); got {tx_seqs:?}",
        );

        // Sanity: row order matches the lifecycle — NotYetCreated, Active,
        // NotYetCreated.
        let statuses: Vec<i16> = entries.iter().map(|e| e.object_status).collect();
        assert_eq!(
            statuses,
            vec![
                BackwardHistoryObjectStatus::NotYetCreated as i16, // first create
                BackwardHistoryObjectStatus::Active as i16,        // remove (prior input)
                BackwardHistoryObjectStatus::NotYetCreated as i16, // recreate
            ]
        );

        // The recreate's NotYetCreated row (anchored at `lamport_recreate - 1`)
        // must sit strictly above the prior Active row's `object_version`.
        let active_version = entries
            .iter()
            .find(|e| e.object_status == BackwardHistoryObjectStatus::Active as i16)
            .expect("F1 should have an Active prior-state row from the remove tx")
            .object_version;
        let recreate_nyc_version = entries
            .last()
            .expect("F1 should have a recreate NotYetCreated row")
            .object_version;
        assert!(
            recreate_nyc_version > active_version,
            "recreate-NYC's object_version (v={recreate_nyc_version}) must be strictly above \
             the prior Active row's object_version (v={active_version}) — lamport monotonicity.",
        );

        Ok(())
    })
}
