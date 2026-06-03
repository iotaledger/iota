// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `checkpointed_objects` verifying that wrapped,
//! deleted, and unwrapped objects are stored with the correct `object_status`.

use std::str::FromStr;

use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};
use iota_indexer::{
    errors::IndexerError, models::objects::StoredCheckpointedObject, schema::checkpointed_objects,
    store::PgIndexerStore, types::ObjectStatus,
};
use iota_json::call_args;
use iota_types::crypto::{AccountKeyPair, IotaKeyPair, get_key_pair};

use crate::{
    backward_history::{call_test_fn, first_created_id},
    common::{
        ApiTestSetup, indexer_wait_for_object, indexer_wait_for_transaction,
        publish_test_move_package,
    },
};

fn find_checkpointed_object(
    store: &PgIndexerStore,
    object_id: &[u8],
) -> Result<Option<StoredCheckpointedObject>, IndexerError> {
    iota_indexer::read_only_blocking!(&store.blocking_cp(), |conn| {
        checkpointed_objects::table
            .filter(checkpointed_objects::object_id.eq(object_id))
            .select(StoredCheckpointedObject::as_select())
            .first::<StoredCheckpointedObject>(conn)
            .optional()
    })
}

#[test]
fn checkpointed_objects_wrap_delete_unwrap_lifecycle() -> Result<(), anyhow::Error> {
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

        // Step 1: CREATE — item should be Active in checkpointed_objects
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

        let entry = find_checkpointed_object(store, item_id.as_bytes())?
            .expect("item should exist in checkpointed_objects after creation");
        assert_eq!(
            entry.object_status,
            ObjectStatus::Active as i16,
            "newly created item should be Active"
        );
        assert!(entry.object_digest.is_some());
        assert!(entry.owner_type.is_some());
        assert!(entry.serialized_object.is_some());

        // Step 2: WRAP — item should become WrappedOrDeleted
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
        let box_id = first_created_id(&resp);

        let entry = find_checkpointed_object(store, item_id.as_bytes())?
            .expect("wrapped item should still exist in checkpointed_objects as tombstone");
        assert_eq!(
            entry.object_status,
            ObjectStatus::WrappedOrDeleted as i16,
            "wrapped item should be WrappedOrDeleted"
        );
        assert!(entry.object_digest.is_none());
        assert!(entry.owner_type.is_none());
        assert!(entry.serialized_object.is_none());

        // Box should be Active.
        let entry = find_checkpointed_object(store, box_id.as_bytes())?
            .expect("box should exist in checkpointed_objects");
        assert_eq!(
            entry.object_status,
            ObjectStatus::Active as i16,
            "box should be Active"
        );

        // Step 3: UNWRAP — item should become Active again
        let _resp = call_test_fn(
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

        let entry = find_checkpointed_object(store, item_id.as_bytes())?
            .expect("unwrapped item should exist in checkpointed_objects");
        assert_eq!(
            entry.object_status,
            ObjectStatus::Active as i16,
            "unwrapped item should be Active again"
        );
        assert!(entry.object_digest.is_some());
        assert!(entry.owner_type.is_some());
        assert!(entry.serialized_object.is_some());

        // Box should be WrappedOrDeleted (it was consumed by unwrap).
        let entry = find_checkpointed_object(store, box_id.as_bytes())?
            .expect("deleted box should still exist as tombstone");
        assert_eq!(
            entry.object_status,
            ObjectStatus::WrappedOrDeleted as i16,
            "deleted box should be WrappedOrDeleted"
        );

        // Step 4: DELETE — item should become WrappedOrDeleted
        let _resp = call_test_fn(
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

        let entry = find_checkpointed_object(store, item_id.as_bytes())?
            .expect("deleted item should still exist as tombstone");
        assert_eq!(
            entry.object_status,
            ObjectStatus::WrappedOrDeleted as i16,
            "deleted item should be WrappedOrDeleted"
        );
        assert!(entry.object_digest.is_none());
        assert!(entry.owner_type.is_none());
        assert!(entry.serialized_object.is_none());

        Ok(())
    })
}
