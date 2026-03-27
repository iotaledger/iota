// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests for `create_batching_stream!` macro used by `get_objects` and
//! `get_transactions` endpoints.
mod common;

use std::{collections::HashMap, sync::Arc};

use common::MockGrpcStateReader;
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::{
        ledger_service::{
            GetObjectsRequest, GetTransactionsRequest, ObjectRequest, ObjectRequests,
            TransactionRequest, TransactionRequests, ledger_service_client::LedgerServiceClient,
        },
        types::ObjectReference,
    },
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::{ObjectID, random_object_ref},
    crypto::{AccountKeyPair, get_key_pair},
    digests::TransactionDigest,
    effects::{TestEffectsBuilder, TransactionEffects},
    gas_coin::GasCoin,
    object::{MoveObject, OBJECT_START_VERSION, Object, Owner},
    transaction::VerifiedTransaction,
};
use prost::Message;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a large object (~`padding_bytes` extra) so that a small number of
/// objects exceeds the 1 MB minimum message size.
fn create_large_object(padding_bytes_len: usize) -> (ObjectID, Object) {
    let id = ObjectID::random();
    let (owner, _) = get_key_pair::<AccountKeyPair>();
    let mut contents = GasCoin::new(id, 100).to_bcs_bytes();
    contents.extend(vec![0u8; padding_bytes_len]);
    let move_obj = MoveObject::new_from_execution_with_limit(
        GasCoin::type_().into(),
        OBJECT_START_VERSION,
        contents,
        u64::try_from(padding_bytes_len).unwrap() + 1024,
    )
    .unwrap();
    let obj = Object::new_move(
        move_obj,
        Owner::AddressOwner(owner),
        TransactionDigest::genesis_marker(),
    );
    (id, obj)
}

/// Create a transaction and its effects so `get_transactions` can return it.
fn create_test_transaction() -> (
    TransactionDigest,
    Arc<VerifiedTransaction>,
    TransactionEffects,
) {
    let (sender, key): (_, AccountKeyPair) = get_key_pair();
    let gas = random_object_ref();
    let tx = TestTransactionBuilder::new(sender, gas, 1000)
        .transfer(random_object_ref(), sender)
        .build_and_sign(&key);
    let effects = TestEffectsBuilder::new(tx.data()).build();
    let digest = *tx.digest();
    (
        digest,
        Arc::new(VerifiedTransaction::new_unchecked(tx)),
        effects,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_get_objects_batching_within_limit() {
    // Create 30 objects each ~50 KB so the total (~1.5 MB) exceeds the 1 MB
    // minimum message size and forces batching.
    const NUM_OBJECTS: usize = 30;
    const PADDING: usize = 50_000;

    let mut objects = HashMap::new();
    let mut object_ids = Vec::with_capacity(NUM_OBJECTS);
    for _ in 0..NUM_OBJECTS {
        let (id, obj) = create_large_object(PADDING);
        object_ids.push(id);
        objects.insert(id, obj);
    }

    let state_reader = Arc::new(MockGrpcStateReader {
        objects,
        ..Default::default()
    });

    let (server_handle, _) = common::start_test_server(state_reader, |_| {}).await;
    let addr = server_handle.address();

    // Use the raw tonic client instead of the high-level Client so we can
    // inspect individual streamed GetObjectsResponse messages and verify their
    // sizes. The high-level client reassembles them into a single response.
    let channel = tonic::transport::Channel::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .expect("connect");
    let mut client = LedgerServiceClient::new(channel).max_decoding_message_size(128 * 1024 * 1024);

    // Build proto request parts (reused across passes)
    let requests = ObjectRequests::default().with_requests(
        object_ids
            .iter()
            .map(|id| {
                ObjectRequest::default().with_object_ref(
                    ObjectReference::default().with_object_id(
                        iota_grpc_types::v1::types::ObjectId::default()
                            .with_object_id(id.as_ref().to_vec()),
                    ),
                )
            })
            .collect(),
    );
    let read_mask = prost_types::FieldMask::from_str("reference,bcs");

    // --- Pass 1: unlimited (128 MB) → measure single-batch encoded size ---
    let req = GetObjectsRequest::default()
        .with_requests(requests.clone())
        .with_read_mask(read_mask.clone())
        .with_max_message_size_bytes(128 * 1024 * 1024);
    let mut stream = client
        .get_objects(req)
        .await
        .expect("get_objects should succeed")
        .into_inner();

    let mut all_responses = Vec::new();
    while let Some(resp) = stream.message().await.expect("stream should not error") {
        all_responses.push(resp);
    }

    assert_eq!(
        all_responses.len(),
        1,
        "With 128 MB limit everything should fit in a single response"
    );
    assert!(
        !all_responses[0].has_next,
        "Single response should have has_next = false"
    );
    let total_items: usize = all_responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(total_items, NUM_OBJECTS, "All objects should be returned");

    // The macro checks `candidate_size + HAS_NEXT_TRUE_OVERHEAD > max` for each
    // item, where HAS_NEXT_TRUE_OVERHEAD = 2 bytes. encoded_len() with
    // has_next=false does not include those 2 bytes, so the exact limit that
    // still fits everything in one batch is encoded_len() + 2.
    let exact_limit = u32::try_from(all_responses[0].encoded_len()).unwrap() + 2;
    assert!(
        exact_limit >= 1_024 * 1_024,
        "Test prerequisite: single batch ({exact_limit}) must be >= 1 MB \
         so the server does not reject the limit"
    );

    // --- Pass 2: exact limit → should still fit in one batch ---
    let req = GetObjectsRequest::default()
        .with_requests(requests.clone())
        .with_read_mask(read_mask.clone())
        .with_max_message_size_bytes(exact_limit);
    let mut stream = client
        .get_objects(req)
        .await
        .expect("get_objects should succeed")
        .into_inner();

    let mut exact_responses = Vec::new();
    while let Some(resp) = stream.message().await.expect("stream should not error") {
        exact_responses.push(resp);
    }
    assert_eq!(
        exact_responses.len(),
        1,
        "At exact limit ({exact_limit}) all objects should still fit in one batch"
    );

    // --- Pass 3: exact - 1 → must split ---
    let tight_limit = exact_limit - 1;
    let req = GetObjectsRequest::default()
        .with_requests(requests.clone())
        .with_read_mask(read_mask.clone())
        .with_max_message_size_bytes(tight_limit);
    let mut stream = client
        .get_objects(req)
        .await
        .expect("get_objects should succeed")
        .into_inner();

    let mut split_responses = Vec::new();
    while let Some(resp) = stream.message().await.expect("stream should not error") {
        split_responses.push(resp);
    }

    assert!(
        split_responses.len() > 1,
        "At limit {tight_limit} (exact-1) objects must be split, got {} batch(es)",
        split_responses.len()
    );

    // Verify has_next flags: all but last should be true
    for (i, resp) in split_responses.iter().enumerate() {
        let is_last = i == split_responses.len() - 1;
        assert_eq!(
            resp.has_next, !is_last,
            "Response {i}: has_next should be {} (is_last={is_last})",
            !is_last
        );
    }

    // All items should be present
    let split_total: usize = split_responses.iter().map(|r| r.objects.len()).sum();
    assert_eq!(split_total, NUM_OBJECTS, "All objects must be returned");

    // Every response should fit within the message size limit
    for (i, resp) in split_responses.iter().enumerate() {
        let size = resp.encoded_len();
        assert!(
            size <= usize::try_from(tight_limit).unwrap(),
            "Response {i} has encoded_len {size} which exceeds limit {tight_limit}"
        );
    }

    server_handle.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn test_get_transactions_batching_within_limit() {
    // Each transaction proto (with bcs + signatures + effects) is ~1 KB.
    // 1500 transactions comfortably exceeds the 1 MB minimum message size.
    const NUM_TXS: usize = 1500;

    let mut transactions = HashMap::new();
    let mut effects_map = HashMap::new();
    let mut digests = Vec::with_capacity(NUM_TXS);

    for _ in 0..NUM_TXS {
        let (digest, tx, effects) = create_test_transaction();
        digests.push(digest);
        transactions.insert(digest, tx);
        effects_map.insert(digest, effects);
    }

    let state_reader = Arc::new(MockGrpcStateReader {
        transactions,
        effects: effects_map,
        ..Default::default()
    });

    let (server_handle, _) = common::start_test_server(state_reader, |_| {}).await;
    let addr = server_handle.address();

    // Use the raw tonic client instead of the high-level Client so we can
    // inspect individual streamed GetTransactionsResponse messages and verify
    // their sizes. The high-level client reassembles them into a single response.
    let channel = tonic::transport::Channel::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .expect("connect");
    let mut client = LedgerServiceClient::new(channel).max_decoding_message_size(128 * 1024 * 1024);

    // Build proto request parts (reused across passes)
    let requests = TransactionRequests::default().with_requests(
        digests
            .iter()
            .map(|d| {
                TransactionRequest::default().with_digest(
                    iota_grpc_types::v1::types::Digest::default()
                        .with_digest(d.into_inner().to_vec()),
                )
            })
            .collect(),
    );
    let read_mask = prost_types::FieldMask::from_str("transaction,signatures,effects");

    // --- Pass 1: unlimited (128 MB) → measure single-batch encoded size ---
    let req = GetTransactionsRequest::default()
        .with_requests(requests.clone())
        .with_read_mask(read_mask.clone())
        .with_max_message_size_bytes(128 * 1024 * 1024);
    let mut stream = client
        .get_transactions(req)
        .await
        .expect("get_transactions should succeed")
        .into_inner();

    let mut all_responses = Vec::new();
    while let Some(resp) = stream.message().await.expect("stream should not error") {
        all_responses.push(resp);
    }

    assert_eq!(
        all_responses.len(),
        1,
        "With 128 MB limit everything should fit in a single response"
    );
    assert!(
        !all_responses[0].has_next,
        "Single response should have has_next = false"
    );
    let total_items: usize = all_responses
        .iter()
        .map(|r| r.transaction_results.len())
        .sum();
    assert_eq!(total_items, NUM_TXS, "All transactions should be returned");

    // The macro checks `candidate_size + HAS_NEXT_TRUE_OVERHEAD > max` for each
    // item, where HAS_NEXT_TRUE_OVERHEAD = 2 bytes. encoded_len() with
    // has_next=false does not include those 2 bytes, so the exact limit that
    // still fits everything in one batch is encoded_len() + 2.
    let exact_limit = u32::try_from(all_responses[0].encoded_len()).unwrap() + 2;
    assert!(
        exact_limit >= 1_024 * 1_024,
        "Test prerequisite: single batch ({exact_limit}) must be >= 1 MB \
         so the server does not reject the limit"
    );

    // --- Pass 2: exact limit → should still fit in one batch ---
    let req = GetTransactionsRequest::default()
        .with_requests(requests.clone())
        .with_read_mask(read_mask.clone())
        .with_max_message_size_bytes(exact_limit);
    let mut stream = client
        .get_transactions(req)
        .await
        .expect("get_transactions should succeed")
        .into_inner();

    let mut exact_responses = Vec::new();
    while let Some(resp) = stream.message().await.expect("stream should not error") {
        exact_responses.push(resp);
    }
    assert_eq!(
        exact_responses.len(),
        1,
        "At exact limit ({exact_limit}) all transactions should still fit in one batch"
    );

    // --- Pass 3: exact - 1 → must split ---
    let tight_limit = exact_limit - 1;
    let req = GetTransactionsRequest::default()
        .with_requests(requests.clone())
        .with_read_mask(read_mask.clone())
        .with_max_message_size_bytes(tight_limit);
    let mut stream = client
        .get_transactions(req)
        .await
        .expect("get_transactions should succeed")
        .into_inner();

    let mut split_responses = Vec::new();
    while let Some(resp) = stream.message().await.expect("stream should not error") {
        split_responses.push(resp);
    }

    assert!(
        split_responses.len() > 1,
        "At limit {tight_limit} (exact-1) transactions must be split, got {} batch(es)",
        split_responses.len()
    );

    // Verify has_next flags
    for (i, resp) in split_responses.iter().enumerate() {
        let is_last = i == split_responses.len() - 1;
        assert_eq!(
            resp.has_next, !is_last,
            "Response {i}: has_next should be {} (is_last={is_last})",
            !is_last
        );
    }

    // All items should be present
    let split_total: usize = split_responses
        .iter()
        .map(|r| r.transaction_results.len())
        .sum();
    assert_eq!(split_total, NUM_TXS, "All transactions must be returned");

    // Every response should fit within the message size limit
    for (i, resp) in split_responses.iter().enumerate() {
        let size = resp.encoded_len();
        assert!(
            size <= usize::try_from(tight_limit).unwrap(),
            "Response {i} has encoded_len {size} which exceeds limit {tight_limit}"
        );
    }

    server_handle.shutdown().await.expect("shutdown");
}
