// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v0::state_service::ListDynamicFieldsRequest;
use iota_macros::sim_test;

use crate::{
    collect_streaming_responses,
    utils::{assert_tonic_error, object_id_from_hex, setup_grpc_test},
};

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

    let responses = collect_streaming_responses!(
        state_client,
        list_dynamic_fields,
        request,
        "system state dynamic fields"
    );

    let fields: Vec<_> = responses.iter().flat_map(|r| &r.dynamic_fields).collect();

    assert!(
        !fields.is_empty(),
        "System state object should have at least one dynamic field"
    );

    // With the default read mask ("parent,field_id"), both fields should be set.
    for field in &fields {
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

    let responses = collect_streaming_responses!(
        state_client,
        list_dynamic_fields,
        request,
        "object with no dynamic fields"
    );

    let total_fields: usize = responses.iter().map(|r| r.dynamic_fields.len()).sum();
    assert_eq!(
        total_fields, 0,
        "Clock object should have no dynamic fields"
    );
}
