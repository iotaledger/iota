// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{field::FieldMaskUtil, v1::state_service::ListDynamicFieldsRequest};
use iota_macros::sim_test;
use prost_types::FieldMask;

use crate::utils::{assert_tonic_error, object_id_from_hex, setup_grpc_test};

#[sim_test]
async fn list_dynamic_fields_missing_parent() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;
    let mut state_client = client.state_service_client();

    // Missing parent should return InvalidArgument
    let result = state_client
        .list_dynamic_fields(ListDynamicFieldsRequest::default())
        .await;

    assert_tonic_error(result, tonic::Code::InvalidArgument, "missing parent");
}

#[sim_test]
async fn list_dynamic_fields_system_state() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    // System state object (0x5) wraps `IotaSystemStateInnerV1` as a dynamic
    // field, so it always has at least one dynamic field after genesis.
    let request = ListDynamicFieldsRequest::default().with_parent(object_id_from_hex("0x5"));

    let response = state_client
        .list_dynamic_fields(request)
        .await
        .unwrap()
        .into_inner();

    assert!(
        !response.dynamic_fields.is_empty(),
        "System state object should have at least one dynamic field"
    );

    // With the default read mask ("parent,field_id"), both fields should be set.
    for field in &response.dynamic_fields {
        assert!(
            field.parent.is_some(),
            "parent should be populated with default read mask"
        );
        assert!(
            field.field_id.is_some(),
            "field_id should be populated with default read mask"
        );
    }
}

#[sim_test]
async fn list_dynamic_fields_no_fields() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    // Clock object (0x6) has no dynamic fields
    let request = ListDynamicFieldsRequest::default().with_parent(object_id_from_hex("0x6"));

    let response = state_client
        .list_dynamic_fields(request)
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        response.dynamic_fields.len(),
        0,
        "Clock object should have no dynamic fields"
    );
    assert!(
        response.next_page_token.is_none(),
        "Should have no next page token when there are no results"
    );
}

#[sim_test]
async fn list_dynamic_fields_with_readmask() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    // Request only "kind" — other index-only fields (parent, field_id, etc.)
    // should be absent.
    let request = ListDynamicFieldsRequest::default()
        .with_parent(object_id_from_hex("0x5"))
        .with_read_mask(FieldMask::from_paths(["kind"]));

    let response = state_client
        .list_dynamic_fields(request)
        .await
        .unwrap()
        .into_inner();

    assert!(
        !response.dynamic_fields.is_empty(),
        "Should return fields with partial mask"
    );

    for field in &response.dynamic_fields {
        assert!(
            field.kind.is_some(),
            "kind should be populated when requested"
        );
        assert!(
            field.parent.is_none(),
            "parent should be absent when not in read mask"
        );
        assert!(
            field.field_id.is_none(),
            "field_id should be absent when not in read mask"
        );
    }
}

#[sim_test]
async fn list_dynamic_fields_with_page_size() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    let request = ListDynamicFieldsRequest::default()
        .with_parent(object_id_from_hex("0x5"))
        .with_page_size(1);

    let response = state_client
        .list_dynamic_fields(request)
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        response.dynamic_fields.len(),
        1,
        "Should return exactly 1 dynamic field, got {}",
        response.dynamic_fields.len()
    );
}

#[sim_test]
async fn list_dynamic_fields_invalid_readmask() {
    let (_test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let mut state_client = client.state_service_client();

    // Invalid field path in read mask should return InvalidArgument
    let request = ListDynamicFieldsRequest::default()
        .with_parent(object_id_from_hex("0x5"))
        .with_read_mask(FieldMask::from_paths(["nonexistent_field"]));

    let result = state_client.list_dynamic_fields(request).await;

    assert_tonic_error(
        result,
        tonic::Code::InvalidArgument,
        "invalid read mask field",
    );
}
