// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_grpc_client::ReadClient;
use iota_types::{base_types::ObjectID, object::ObjectRead};
use test_cluster::TestCluster;

mod utils;
use utils::setup_test_cluster_and_client;

async fn setup_test_cluster_and_read_client() -> (TestCluster, ReadClient) {
    let (cluster, node_client) = setup_test_cluster_and_client().await;

    let read_client = node_client
        .read_client()
        .expect("Read client should be available");

    (cluster, read_client)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_read_service_get_object() {
    let (cluster, mut read_client) = setup_test_cluster_and_read_client().await;

    // Get a known object (gas object from account)
    let sender = cluster.get_address_0();
    let owned_objects = cluster
        .get_owned_objects(sender, None)
        .await
        .expect("Failed to get owned objects");

    assert!(
        !owned_objects.is_empty(),
        "Should have at least one owned object"
    );

    // Find a gas object (coin type) - just use the first object for simplicity
    let gas_object = &owned_objects[0];
    let object_id = gas_object.data.as_ref().unwrap().object_id;

    let object_read =
        tokio::time::timeout(Duration::from_secs(30), read_client.get_object(object_id))
            .await
            .expect("timeout waiting for object")
            .expect("ReadService get_object should work");

    // Verify ObjectRead::Exists variant
    match object_read {
        ObjectRead::Exists(object_ref, object, _layout) => {
            let expected_object_ref = gas_object.data.as_ref().unwrap().object_ref();

            assert_eq!(
                object_ref.0, expected_object_ref.0,
                "Object ID should match"
            );
            assert_eq!(object_ref.1, expected_object_ref.1, "Version should match");
            assert_eq!(
                object.owner(),
                &iota_types::object::Owner::AddressOwner(sender)
            );

            // Verify object type is present
            let obj_data = object
                .data
                .try_as_move()
                .expect("Gas object should be a Move object");
            assert!(
                obj_data.type_().is_coin(),
                "Gas object should be a coin type"
            );
        }
        _ => panic!("Expected ObjectRead::Exists, got {object_read:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_read_service_nonexistent_object() {
    let (_cluster, mut read_client) = setup_test_cluster_and_read_client().await;

    // Use a dummy object ID that doesn't exist
    let nonexistent_id = ObjectID::from_bytes([0u8; 32]).unwrap();

    let object_read = tokio::time::timeout(
        Duration::from_secs(30),
        read_client.get_object(nonexistent_id),
    )
    .await
    .expect("timeout waiting for object")
    .expect("ReadService get_object should work");

    // Verify ObjectRead::NotExists variant
    match object_read {
        ObjectRead::NotExists(object_id) => {
            assert_eq!(object_id, nonexistent_id, "Object ID should match");
        }
        _ => panic!("Expected ObjectRead::NotExists, got {object_read:?}"),
    }
}
