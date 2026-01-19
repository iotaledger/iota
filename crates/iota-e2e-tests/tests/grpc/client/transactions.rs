// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::Error;
use iota_macros::sim_test;
use iota_sdk_types::Digest;

use super::common::{
    assert_not_found_error, execute_transaction_and_get_digest, is_success, setup_grpc_test,
};

#[sim_test]
async fn get_transactions_single() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let digest = execute_transaction_and_get_digest(&test_cluster).await;
    test_cluster.wait_for_checkpoint(2, None).await;

    let transactions = client
        .get_transactions(&[digest], None)
        .await
        .expect("Failed to get transaction");

    assert_eq!(transactions.len(), 1, "Expected exactly one transaction");

    let tx = &transactions[0];
    assert_eq!(
        tx.digest, digest,
        "Transaction digest should match requested digest"
    );
    assert!(!tx.signatures.is_empty(), "Signatures should be present");
}

#[sim_test]
async fn get_transactions_batch() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let digest1 = execute_transaction_and_get_digest(&test_cluster).await;
    let digest2 = execute_transaction_and_get_digest(&test_cluster).await;
    test_cluster.wait_for_checkpoint(3, None).await;

    let transactions = client
        .get_transactions(&[digest1, digest2], None)
        .await
        .expect("Failed to get transactions");

    assert_eq!(transactions.len(), 2, "Expected exactly two transactions");
    assert_eq!(
        transactions[0].digest, digest1,
        "First transaction should match first digest"
    );
    assert_eq!(
        transactions[1].digest, digest2,
        "Second transaction should match second digest"
    );
}

#[sim_test]
async fn get_transactions_empty_input() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let transactions = client
        .get_transactions(&[], None)
        .await
        .expect("Empty input should succeed");

    assert!(
        transactions.is_empty(),
        "Empty input should return empty result"
    );
}

#[sim_test]
async fn get_transactions_nonexistent() {
    let (_test_cluster, client) = setup_grpc_test(1).await;

    let fake_digest = Digest::new([0u8; 32]);
    let result = client.get_transactions(&[fake_digest], None).await;
    assert_not_found_error(result);
}

#[sim_test]
async fn get_transactions_mixed_valid_invalid() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let valid_digest = execute_transaction_and_get_digest(&test_cluster).await;
    test_cluster.wait_for_checkpoint(2, None).await;

    let fake_digest = Digest::new([0u8; 32]);

    let result = client
        .get_transactions(&[valid_digest, fake_digest], None)
        .await;

    assert!(
        result.is_err(),
        "Mixed valid/invalid should return an error when encountering invalid digest"
    );
}

#[sim_test]
async fn get_transactions_response_fields() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let digest = execute_transaction_and_get_digest(&test_cluster).await;
    test_cluster.wait_for_checkpoint(3, None).await;

    let transactions = client
        .get_transactions(&[digest], None)
        .await
        .expect("Failed to get transaction");

    let tx = &transactions[0];

    assert_eq!(tx.digest, digest, "Digest should match");
    assert!(!tx.signatures.is_empty(), "Signatures should be present");
    assert!(
        is_success(tx.effects.status()),
        "Transaction should have succeeded"
    );
    assert!(
        tx.checkpoint.is_some(),
        "Checkpoint should be present after finalization"
    );
    assert!(
        tx.timestamp_ms.is_some(),
        "Timestamp should be present after finalization"
    );
}

#[sim_test]
async fn get_transactions_invalid_read_mask() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let digest = execute_transaction_and_get_digest(&test_cluster).await;
    test_cluster.wait_for_checkpoint(2, None).await;

    // Try to fetch with an incomplete read mask (missing required fields)
    let result = client
        .get_transactions(&[digest], Some("transaction.digest"))
        .await;

    assert!(
        result.is_err(),
        "Incomplete read mask should cause deserialization error"
    );

    match result {
        Err(Error::ProtoConversion(_)) => {}
        Err(e) => panic!("Expected ProtoConversion error, got: {e:?}"),
        Ok(_) => panic!("Expected error for incomplete read mask"),
    }
}
