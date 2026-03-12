// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_sdk_types::ObjectId;

use super::{super::utils::setup_grpc_test, common::assert_server_not_found};

/// System package IDs that are always available.
const SYSTEM_PACKAGE_IDS: [&str; 3] = ["0x1", "0x2", "0x3"];

#[sim_test]
async fn get_objects_scenarios() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;

    // Test: get single object
    let object_id: ObjectId = "0x2".parse().expect("Invalid object ID");
    let objects = client
        .get_objects(&[(object_id, None)], None)
        .await
        .expect("Failed to get object");
    assert_eq!(objects.body().len(), 1, "Expected exactly one object");
    assert!(
        objects.body()[0]
            .object_reference()
            .expect("Failed to get object reference")
            .version()
            > 0,
        "Object should have a valid version"
    );

    // Test: get batch of system packages
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
        objects.body().len(),
        object_ids.len(),
        "Should return same number of objects as requested"
    );
    for object in objects.body() {
        assert!(
            object
                .object_reference()
                .expect("Failed to get object reference")
                .version()
                > 0,
            "Each object should have a valid version"
        );
        assert!(
            object
                .object()
                .expect("Failed to deserialize object")
                .data
                .is_package(),
            "System object should be a package"
        );
    }

    // Test: empty input returns an error
    let err = client
        .get_objects(&[], None)
        .await
        .expect_err("Empty input should return an error");
    assert!(
        matches!(err, iota_grpc_client::Error::EmptyRequest),
        "Expected EmptyRequest error, got: {err}"
    );

    // Test: get object with specific version
    let object_id: ObjectId = "0x2".parse().expect("Invalid object ID");
    let objects = client
        .get_objects(&[(object_id, None)], None)
        .await
        .expect("Failed to get object");
    let current_version = objects.body()[0]
        .object_reference()
        .expect("Failed to get object reference")
        .version();
    let objects_with_version = client
        .get_objects(&[(object_id, Some(current_version))], None)
        .await
        .expect("Failed to get object with specific version");
    assert_eq!(
        objects_with_version.body()[0]
            .object_reference()
            .expect("Failed to get object reference")
            .version(),
        current_version,
        "Object version should match requested version"
    );

    // Test: nonexistent object returns not-found error
    let fake_id: ObjectId = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        .parse()
        .expect("Invalid object ID");
    let result = client.get_objects(&[(fake_id, None)], None).await;
    assert_server_not_found(result);

    // Test: invalid version returns error
    let object_id: ObjectId = "0x2".parse().expect("Invalid object ID");
    let result = client
        .get_objects(&[(object_id, Some(999_999_999))], None)
        .await;
    assert!(
        result.is_err(),
        "Fetching object with invalid version should return an error"
    );

    // Test: mixed valid/invalid returns error
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
