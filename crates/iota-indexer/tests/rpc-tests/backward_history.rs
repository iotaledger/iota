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
        let gas_id = gas.0;
        indexer_wait_for_object(client, gas.0, gas.1).await;

        // --- Publish the test package ---
        let ((package_id, _, _), publish_resp) =
            publish_test_move_package(client, address, &keypair, "backward_history_test").await?;
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

        let entry = find_backward_entry(store, &item_id.to_vec(), create_cp)?
            .expect("item should have backward history at create checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::NotYetCreated as i16
        );
        assert_eq!(entry.object_version, -1);
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

        let entry = find_backward_entry(store, &item_id.to_vec(), mutate_cp)?
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
        let entry = find_backward_entry(store, &item_id.to_vec(), wrap_cp)?
            .expect("item should have backward history at wrap checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert!(entry.serialized_object.is_some());

        // Box was created → NOT_YET_CREATED.
        let entry = find_backward_entry(store, &box_id.to_vec(), wrap_cp)?
            .expect("box should have backward history at wrap checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::NotYetCreated as i16
        );
        assert_eq!(entry.object_version, -1);

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
        let entry = find_backward_entry(store, &item_id.to_vec(), unwrap_cp)?
            .expect("item should have backward history at unwrap checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::WrappedOrDeleted as i16
        );
        assert_eq!(
            entry.object_version,
            item_unwrap_version.value() as i64 - 1,
            "unwrapped entry should have lamport version - 1"
        );
        assert!(entry.serialized_object.is_none());
        assert!(entry.object_digest.is_none());

        // Box was deleted → ACTIVE backward entry with data.
        let entry = find_backward_entry(store, &box_id.to_vec(), unwrap_cp)?
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
        let entry = find_backward_entry(store, &item_id.to_vec(), delete_cp)?
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
        let entry = find_backward_entry(store, &box2_id.to_vec(), unwrap_delete_cp)?
            .expect("box2 should have backward history at unwrap_and_delete checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::Active as i16
        );
        assert!(entry.serialized_object.is_some());

        // Item inside was unwrapped-then-deleted → WRAPPED_OR_DELETED.
        let item2_utd_version = unwrapped_then_deleted_version(&resp, item2_id);
        let entry = find_backward_entry(store, &item2_id.to_vec(), unwrap_delete_cp)?
            .expect("item2 should have backward history at unwrap_and_delete checkpoint");
        assert_eq!(
            entry.object_status,
            BackwardHistoryObjectStatus::WrappedOrDeleted as i16
        );
        assert_eq!(
            entry.object_version,
            item2_utd_version.value() as i64 - 1,
            "unwrapped-then-deleted entry should have lamport version - 1"
        );
        assert!(entry.serialized_object.is_none());
        assert!(entry.object_digest.is_none());

        // ================================================================
        // Verify full history chain for the first item.
        // ================================================================
        let all_entries = find_all_entries_for_object(store, &item_id.to_vec())?;
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
