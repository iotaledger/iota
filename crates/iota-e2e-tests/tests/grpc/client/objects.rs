// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_sdk_types::{ObjectId, Version};

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
            .as_ref()
            .expect("Object should be found")
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
        let object = object.as_ref().expect("System package should be found");
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
        .as_ref()
        .expect("Object should be found")
        .object_reference()
        .expect("Failed to get object reference")
        .version();
    let objects_with_version = client
        .get_objects(&[(object_id, Some(current_version))], None)
        .await
        .expect("Failed to get object with specific version");
    assert_eq!(
        objects_with_version.body()[0]
            .as_ref()
            .expect("Object should be found")
            .object_reference()
            .expect("Failed to get object reference")
            .version(),
        current_version,
        "Object version should match requested version"
    );

    // Test: a nonexistent object is reported against the ref that asked for it,
    // not as a failure of the call
    let fake_id: ObjectId = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        .parse()
        .expect("Invalid object ID");
    let mut results = client
        .get_objects(&[(fake_id, None)], None)
        .await
        .expect("The call itself should succeed")
        .into_inner();
    assert_eq!(results.len(), 1, "Expected one result per requested ref");
    assert_server_not_found(results.pop().expect("Length asserted above"));

    // Test: invalid version returns a per-ref error
    let object_id: ObjectId = "0x2".parse().expect("Invalid object ID");
    let mut results = client
        .get_objects(&[(object_id, Some(Version::from_u64(999_999_999)))], None)
        .await
        .expect("The call itself should succeed")
        .into_inner();
    assert_eq!(results.len(), 1, "Expected one result per requested ref");
    assert!(
        results.pop().expect("Length asserted above").is_err(),
        "Fetching object with invalid version should return an error"
    );

    // Test: a missing object fails only its own slot, leaving the objects the
    // node could serve intact
    let valid_id: ObjectId = "0x2".parse().expect("Invalid object ID");
    let invalid_id: ObjectId = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        .parse()
        .expect("Invalid object ID");
    let mut results = client
        .get_objects(&[(valid_id, None), (invalid_id, None)], None)
        .await
        .expect("The call itself should succeed")
        .into_inner();
    assert_eq!(results.len(), 2, "Expected one result per requested ref");
    let missing = results.pop().expect("Length asserted above");
    let found = results.pop().expect("Length asserted above");
    assert!(
        found.is_ok(),
        "The object that exists should still be returned, got: {found:?}"
    );
    assert_server_not_found(missing);
}
