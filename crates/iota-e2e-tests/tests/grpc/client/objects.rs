// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_sdk_types::ObjectId;

use super::common::{assert_not_found_error, setup_grpc_test};

/// System package IDs that are always available.
const SYSTEM_PACKAGE_IDS: [&str; 3] = ["0x1", "0x2", "0x3"];

#[sim_test]
async fn get_objects_single() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let object_id: ObjectId = "0x2".parse().expect("Invalid object ID");

    let objects = client
        .get_objects(&[(object_id, None)], None)
        .await
        .expect("Failed to get object");

    assert_eq!(objects.len(), 1, "Expected exactly one object");

    let object = &objects[0];
    assert!(object.version() > 0, "Object should have a valid version");
}

#[sim_test]
async fn get_objects_batch_system_packages() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let object_ids: Vec<ObjectId> = SYSTEM_PACKAGE_IDS
        .iter()
        .map(|s| s.parse().expect("Invalid object ID"))
        .collect();

    let refs: Vec<_> = object_ids.iter().map(|id| (*id, None)).collect();

    let objects = client
        .get_objects(&refs, None)
        .await
        .expect("Failed to get objects");

    assert_eq!(
        objects.len(),
        object_ids.len(),
        "Should return same number of objects as requested"
    );

    for object in &objects {
        assert!(
            object.version() > 0,
            "Each object should have a valid version"
        );
        assert!(
            object.data.is_package(),
            "System object should be a package"
        );
    }
}

#[sim_test]
async fn get_objects_empty_input() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let objects = client
        .get_objects(&[], None)
        .await
        .expect("Empty input should succeed");

    assert!(objects.is_empty(), "Empty input should return empty result");
}

#[sim_test]
async fn get_objects_nonexistent() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let fake_id: ObjectId = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        .parse()
        .expect("Invalid object ID");

    let result = client.get_objects(&[(fake_id, None)], None).await;
    assert_not_found_error(result);
}

#[sim_test]
async fn get_objects_with_version() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let object_id: ObjectId = "0x2".parse().expect("Invalid object ID");

    let objects = client
        .get_objects(&[(object_id, None)], None)
        .await
        .expect("Failed to get object");

    let current_version = objects[0].version();

    let objects_with_version = client
        .get_objects(&[(object_id, Some(current_version))], None)
        .await
        .expect("Failed to get object with specific version");

    assert_eq!(
        objects_with_version[0].version(),
        current_version,
        "Object version should match requested version"
    );
}

#[sim_test]
async fn get_objects_invalid_version() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let object_id: ObjectId = "0x2".parse().expect("Invalid object ID");

    let result = client
        .get_objects(&[(object_id, Some(999_999_999))], None)
        .await;

    assert!(
        result.is_err(),
        "Fetching object with invalid version should return an error"
    );
}

#[sim_test]
async fn get_objects_mixed_valid_invalid() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let valid_id: ObjectId = "0x2".parse().expect("Invalid object ID");
    let invalid_id: ObjectId = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        .parse()
        .expect("Invalid object ID");

    let result = client
        .get_objects(&[(valid_id, None), (invalid_id, None)], None)
        .await;

    assert!(
        result.is_err(),
        "Mixed valid/invalid should return an error when encountering invalid object"
    );
}
