// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pagination edge-case tests for `ListDynamicFields`.
//!
//! Uses `MockGrpcStateReader` populated with controlled data so we can
//! exercise cursor-based seeking without a full validator cluster.

mod common;

use std::sync::Arc;

use common::{MockGrpcStateReader, start_test_server};
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::{
        state_service::{
            ListDynamicFieldsRequest, ListDynamicFieldsResponse,
            state_service_client::StateServiceClient,
        },
        types::ObjectId as ProtoObjectId,
    },
};
use iota_sdk_types::ObjectId;
use iota_types::storage::DynamicFieldKey;
use prost_types::FieldMask;
use tonic::transport::Channel;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn object_id_proto(id: ObjectId) -> ProtoObjectId {
    ProtoObjectId::default().with_object_id(id.into_bytes().to_vec())
}

/// Connect a state-service client to the test server.
async fn connect_state_client(
    handle: &iota_grpc_server::GrpcServerHandle,
) -> StateServiceClient<Channel> {
    let channel = Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .unwrap();
    StateServiceClient::new(channel)
}

/// Extract the returned field IDs (as raw bytes) from a response.
fn returned_field_ids(resp: &ListDynamicFieldsResponse) -> Vec<Vec<u8>> {
    resp.dynamic_fields
        .iter()
        .map(|f| f.field_id.as_ref().unwrap().object_id.to_vec())
        .collect()
}

/// Set up a mock with `count` dynamic fields under a single parent.
///
/// Returns the mock and the field IDs in index key order.
fn make_field_mock(parent: ObjectId, count: usize) -> (MockGrpcStateReader, Vec<ObjectId>) {
    let mut ids: Vec<ObjectId> = (0..count).map(|_| ObjectId::random()).collect();
    // Sort IDs so they match the dynamic-field index key ordering.
    ids.sort();

    let mock = MockGrpcStateReader {
        dynamic_fields: ids
            .iter()
            .map(|&id| DynamicFieldKey::new(parent, id))
            .collect(),
        ..Default::default()
    };
    (mock, ids)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Walk through all items with page_size=1 and verify every field is returned
/// exactly once, in the expected order, with no duplicates or gaps.
#[tokio::test]
async fn paginate_one_at_a_time() {
    let parent = ObjectId::random();
    let (mock, expected_ids) = make_field_mock(parent, 5);

    let (handle, _reader) = start_test_server(Arc::new(mock), |_| {}).await;
    let mut client = connect_state_client(&handle).await;

    let base = ListDynamicFieldsRequest::default()
        .with_parent(object_id_proto(parent))
        .with_page_size(1)
        .with_read_mask(FieldMask::from_str("field_id"));

    let mut returned: Vec<Vec<u8>> = Vec::new();
    let mut page_token = None;
    loop {
        let mut request = base.clone();
        if let Some(token) = page_token.take() {
            request = request.with_page_token(token);
        }
        let resp = client
            .list_dynamic_fields(request)
            .await
            .unwrap()
            .into_inner();

        assert!(
            resp.dynamic_fields.len() <= 1,
            "page_size=1 but got {} fields",
            resp.dynamic_fields.len()
        );
        returned.extend(returned_field_ids(&resp));

        match resp.next_page_token {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }

    let expected: Vec<Vec<u8>> = expected_ids
        .iter()
        .map(|id| id.as_bytes().to_vec())
        .collect();
    assert_eq!(
        returned, expected,
        "every field must be returned exactly once, in index order",
    );
}

/// A cursor row that vanishes between pages (e.g. the field was deleted from
/// its parent) must not cause the next live row to be skipped: a field that
/// did not change is always returned.
#[tokio::test]
async fn cursor_row_removed_between_pages_loses_no_field() {
    let parent = ObjectId::random();
    let (mock, index_ids) = make_field_mock(parent, 3);
    // Derive the state where the first row (the future cursor row) is gone
    // from the index.
    let mut fields_after = mock.dynamic_fields.clone();
    fields_after.remove(0);

    // Page 1 against the full state returns the first row and a token
    // pointing at it.
    let (handle, _reader) = start_test_server(Arc::new(mock), |_| {}).await;
    let mut client = connect_state_client(&handle).await;
    let base = ListDynamicFieldsRequest::default()
        .with_parent(object_id_proto(parent))
        .with_page_size(1)
        .with_read_mask(FieldMask::from_str("field_id"));
    let page1 = client
        .list_dynamic_fields(base.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(
        returned_field_ids(&page1),
        [index_ids[0].as_bytes().to_vec()],
    );
    let token = page1.next_page_token.clone().expect("more rows exist");

    // Page 2 against the state without the cursor row.
    let mock_after = MockGrpcStateReader {
        dynamic_fields: fields_after,
        ..Default::default()
    };
    let (handle_after, _reader_after) = start_test_server(Arc::new(mock_after), |_| {}).await;
    let mut client_after = connect_state_client(&handle_after).await;
    let page2 = client_after
        .list_dynamic_fields(base.with_page_token(token))
        .await
        .unwrap()
        .into_inner();

    assert!(
        returned_field_ids(&page2).contains(&index_ids[1].as_bytes().to_vec()),
        "the unchanged second row must not be skipped when the cursor row is gone",
    );
}
