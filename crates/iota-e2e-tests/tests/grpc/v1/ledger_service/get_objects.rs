// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::StreamExt;
use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::GET_OBJECTS_READ_MASK,
    v1::{
        ledger_service::{
            GetObjectsRequest, GetObjectsResponse, ObjectRequest, ObjectRequests,
            ledger_service_client::LedgerServiceClient, object_result,
        },
        types::ObjectReference,
    },
};
use iota_macros::sim_test;
use prost_types::FieldMask;

use crate::utils::{
    assert_field_presence, comma_separated_field_mask_to_paths, object_id_from_hex, setup_grpc_test,
};

async fn assert_get_objects_request(
    ledger_client: &mut LedgerServiceClient<iota_grpc_client::InterceptedChannel>,
    requests: Vec<ObjectRequest>,
    read_mask: Option<FieldMask>,
    max_message_size_bytes: Option<u32>,
    expected_field_mask_paths: &[&str],
    scenario: &str,
) -> Vec<GetObjectsResponse> {
    let mut request = GetObjectsRequest::default()
        .with_requests(ObjectRequests::default().with_requests(requests));

    if let Some(mask) = read_mask {
        request = request.with_read_mask(mask);
    }

    if let Some(size) = max_message_size_bytes {
        request = request.with_max_message_size_bytes(size);
    }

    let mut stream = ledger_client
        .get_objects(request)
        .await
        .unwrap()
        .into_inner();

    let mut responses = Vec::new();
    let mut response_count = 0;

    // Loop through all responses until has_next is false
    while let Some(response) = stream.next().await {
        let response = response.unwrap();
        response_count += 1;

        // Assert all returned objects have the expected fields
        for (idx, obj_result) in response.objects.iter().enumerate() {
            if let Some(object_result::Result::Object(object)) = &obj_result.result {
                assert_field_presence(
                    object,
                    expected_field_mask_paths,
                    &[],
                    &format!("{scenario} (response {response_count}, object {idx})"),
                );
            }
        }

        let has_next = response.has_next;
        responses.push(response);

        // If has_next is false, this should be the last response
        if !has_next {
            break;
        }
    }

    // Validate has_next values: all intermediate messages should have has_next=true
    for (idx, response) in responses[..responses.len() - 1].iter().enumerate() {
        assert!(
            response.has_next,
            "Intermediate stream message #{} should have has_next=true, but got false",
            idx + 1
        );
    }

    // Verify the last response has has_next=false
    assert!(
        !responses.last().unwrap().has_next,
        "{scenario}: last response should have has_next=false"
    );

    // Verify stream is exhausted
    assert!(
        stream.next().await.is_none(),
        "{scenario}: stream should be exhausted after has_next=false"
    );

    responses
}

#[sim_test]
async fn get_objects_readmask_scenarios() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    let object_id = object_id_from_hex("0x5");

    // Tests for single-object readmask scenarios
    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "default readmask",
            None,
            comma_separated_field_mask_to_paths(GET_OBJECTS_READ_MASK),
        ),
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            vec![],
        ),
        (
            "full readmask",
            Some(FieldMask::from_paths(["reference", "bcs"])),
            vec!["reference", "bcs"],
        ),
        (
            "partial readmask (reference fields only)",
            Some(FieldMask::from_paths([
                "reference.object_id",
                "reference.version",
            ])),
            vec!["reference.object_id", "reference.version"],
        ),
        (
            "partial readmask (bcs only)",
            Some(FieldMask::from_paths(["bcs"])),
            vec!["bcs"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        let responses = assert_get_objects_request(
            &mut ledger_client,
            vec![
                ObjectRequest::default()
                    .with_object_ref(ObjectReference::default().with_object_id(object_id.clone())),
            ],
            mask,
            None,
            &expected_paths,
            scenario,
        )
        .await;

        let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
        assert_eq!(total_objects, 1, "{scenario}: expected 1 object");
    }
}

#[sim_test]
async fn get_objects_batch() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    // Test batch request with multiple objects and partial readmask
    let responses = assert_get_objects_request(
        &mut ledger_client,
        vec![
            ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(object_id_from_hex("0x1")),
            ),
            ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(object_id_from_hex("0x2")),
            ),
            ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(object_id_from_hex("0x3")),
            ),
            ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(object_id_from_hex("0x5")),
            ),
        ],
        Some(FieldMask::from_paths(["reference.object_id", "bcs"])),
        None,
        &["reference.object_id", "bcs"],
        "batch with 4 objects",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(total_objects, 4);
}

#[sim_test]
async fn get_objects_with_version() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    let object_id = object_id_from_hex("0x5");

    // Request specific version
    let responses = assert_get_objects_request(
        &mut ledger_client,
        vec![
            ObjectRequest::default().with_object_ref(
                ObjectReference::default()
                    .with_object_id(object_id)
                    .with_version(1),
            ),
        ],
        Some(FieldMask::from_paths([
            "reference.object_id",
            "reference.version",
        ])),
        None,
        &["reference.object_id", "reference.version"],
        "specific version query",
    )
    .await;

    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(
        total_objects, 1,
        "specific version query: expected 1 object"
    );
}

#[sim_test]
async fn get_objects_streaming() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    // Test streaming by requesting many objects with full readmask
    // Use only known-to-exist objects (0x1-0x6 commonly exist in test cluster)
    // but repeat them multiple times to ensure we have enough data for potential
    // multi-message streaming
    let mut requests = Vec::new();
    let known_objects = ["0x1", "0x2", "0x3", "0x5", "0x6"];

    // Request each object 20 times to create a larger payload (100 total objects,
    // around 2-3 MB)
    for _ in 0..20 {
        for obj_id in &known_objects {
            requests.push(ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(object_id_from_hex(obj_id)),
            ));
        }
    }

    let responses = assert_get_objects_request(
        &mut ledger_client,
        requests,
        Some(FieldMask::from_paths(["reference", "bcs"])),
        // Use minimum allowed message size to maximize chance of streaming
        Some(1024 * 1024_u32), // 1MB (minimum allowed)
        &["reference", "bcs"],
        "streaming with 100 objects",
    )
    .await;

    // Verify multi-message streaming occurred (more than 1 stream message)
    assert!(
        responses.len() > 1,
        "Expected multi-message streaming (>1 message), but got only {} message(s)",
        responses.len()
    );

    // Verify we got all 100 results
    let total_objects: usize = responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(
        total_objects, 100,
        "Should have received 100 results (objects or errors)"
    );
}

#[sim_test]
async fn get_objects_empty_request() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    // Test empty request list
    let responses =
        assert_get_objects_request(&mut ledger_client, vec![], None, None, &[], "empty request")
            .await;

    // Should return single response with 0 objects
    assert_eq!(responses.len(), 1, "Should have 1 response");
    assert_eq!(responses[0].objects.len(), 0, "Should have 0 objects");
    assert!(
        !responses[0].has_next,
        "has_next should be false for empty request"
    );
}

#[sim_test]
async fn get_objects_nonexistent() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    // Request objects that don't exist
    let responses = assert_get_objects_request(
        &mut ledger_client,
        vec![
            ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(object_id_from_hex("0xdead")),
            ),
            ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(object_id_from_hex("0xbeef")),
            ),
        ],
        None,
        None,
        &[], // Skip field mask validation for error responses
        "non-existent objects",
    )
    .await;

    // Verify all results contain errors (not objects)
    let mut error_count = 0;
    for response in &responses {
        for obj_result in &response.objects {
            assert!(
                matches!(obj_result.result, Some(object_result::Result::Error(_))),
                "Expected error for non-existent object"
            );
            assert!(
                !matches!(obj_result.result, Some(object_result::Result::Object(_))),
                "Expected no object for non-existent object"
            );

            if let Some(object_result::Result::Error(error)) = &obj_result.result {
                // Verify error has a non-zero code (indicating an actual error)
                assert!(
                    error.code != 0,
                    "Error should have non-zero code, got: {}",
                    error.code
                );
            }
            error_count += 1;
        }
    }

    assert_eq!(error_count, 2, "Should receive 2 errors");
}
