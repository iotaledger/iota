// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pagination edge-case tests for `ListDynamicFields`.
//!
//! Uses `MockGrpcStateReader` populated with controlled data so we can
//! exercise cursor-based seeking without a full validator cluster.

mod common;

use std::sync::Arc;

use common::{MockGrpcStateReader, connect_state_client, object_id_proto, start_test_server};
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::state_service::{
        ListDynamicFieldsRequest, ListDynamicFieldsResponse,
        state_service_client::StateServiceClient,
    },
};
use iota_sdk_move_types::iota_framework::{dynamic_field::Field, object::UID};
use iota_sdk_types::{MoveStruct, ObjectId, Owner, StructTag, TransactionDigest, TypeTag};
use iota_types::{
    iota_sdk_types_conversions::struct_tag_sdk_to_core,
    move_layout::MoveLayout as _,
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    storage::DynamicFieldKey,
};
use move_core_types::{
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
};
use prost_types::FieldMask;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    // Walk at most `len + 2` pages, so a regression where the cursor stops
    // advancing fails the test instead of hanging it.
    let mut pages_left = expected_ids.len() + 2;
    loop {
        assert!(
            pages_left > 0,
            "cursor did not advance: page budget exhausted"
        );
        pages_left -= 1;
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

    assert_eq!(
        returned_field_ids(&page2),
        [index_ids[1].as_bytes().to_vec()],
        "the unchanged second row must not be skipped when the cursor row is gone",
    );
}

/// The operator config must bound the response size even when the client
/// asks for far more.
#[tokio::test]
async fn message_size_clamped_by_operator_config() {
    // Above the 4 MiB floor of `GrpcApiConfig::max_message_size_bytes`, so
    // that the test can tell the operator value from the floor.
    const OPERATOR_MAX_MESSAGE_SIZE_BYTES: u32 = 8 * 1024 * 1024;
    const CLIENT_REQUESTED_MAX_MESSAGE_SIZE_BYTES: u32 = 128 * 1024 * 1024;
    // 60 fields of 200 KB each, about 12 MB in total.
    const NUM_FIELDS: usize = 60;
    const VALUE_LEN: usize = 200_000;

    let parent = ObjectId::random();
    let (mut mock, field_ids) = make_field_mock(parent, NUM_FIELDS);
    let field_type =
        StructTag::new_dynamic_field(TypeTag::U64, TypeTag::Vector(Box::new(TypeTag::U8)));
    for (name, field_id) in field_ids.iter().enumerate() {
        let contents = bcs::to_bytes(&Field {
            id: (*field_id).into(),
            name: name as u64,
            value: vec![0u8; VALUE_LEN],
        })
        .unwrap();
        let move_struct = MoveStruct::new_from_execution_with_limit(
            field_type.clone(),
            OBJECT_START_VERSION,
            contents,
            u64::try_from(VALUE_LEN).unwrap() + 1024,
        )
        .unwrap();
        let object = Object::new_move(
            move_struct,
            Owner::Object(parent),
            TransactionDigest::GENESIS_MARKER,
        );
        mock.objects.insert(*field_id, object);
    }
    mock.struct_layouts.insert(
        field_type.clone(),
        MoveTypeLayout::Struct(Box::new(MoveStructLayout {
            type_: struct_tag_sdk_to_core(&field_type),
            fields: vec![
                MoveFieldLayout::new(
                    ident_str!("id").to_owned(),
                    MoveTypeLayout::Struct(Box::new(UID::layout())),
                ),
                MoveFieldLayout::new(ident_str!("name").to_owned(), MoveTypeLayout::U64),
                MoveFieldLayout::new(
                    ident_str!("value").to_owned(),
                    MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)),
                ),
            ],
        })),
    );

    let (handle, _reader) = start_test_server(Arc::new(mock), |config| {
        config.max_message_size_bytes = OPERATOR_MAX_MESSAGE_SIZE_BYTES;
    })
    .await;
    // The default client decode limit (4 MiB) is below the operator value.
    let channel = tonic::transport::Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .expect("connect");
    let mut client = StateServiceClient::new(channel).max_decoding_message_size(128 * 1024 * 1024);

    let request = ListDynamicFieldsRequest::default()
        .with_parent(object_id_proto(parent))
        .with_page_size(1000)
        .with_max_message_size_bytes(CLIENT_REQUESTED_MAX_MESSAGE_SIZE_BYTES)
        .with_read_mask(FieldMask::from_str("field_id,value"));
    let resp = client
        .list_dynamic_fields(request)
        .await
        .expect("the server must clamp the response instead of failing")
        .into_inner();

    assert!(
        resp.dynamic_fields
            .iter()
            .all(|field| field.value.is_some()),
        "the values must be loaded, or the response is too small to test the limit"
    );
    assert!(
        resp.next_page_token.is_some(),
        "the operator limit ({OPERATOR_MAX_MESSAGE_SIZE_BYTES} bytes) must split the response \
         even though the client asked for {CLIENT_REQUESTED_MAX_MESSAGE_SIZE_BYTES}"
    );
    common::assert_messages_within_limit(&[resp], OPERATOR_MAX_MESSAGE_SIZE_BYTES);
}
