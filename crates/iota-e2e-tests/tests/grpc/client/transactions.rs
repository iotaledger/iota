// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_sdk_types::Digest;

use super::{
    super::utils::setup_grpc_test,
    common::{
        assert_proto_conversion_error, assert_server_not_found, execute_transaction_and_get_digest,
    },
};

#[sim_test]
async fn get_transactions_scenarios() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    // Execute transactions upfront for later tests
    let digest1 = execute_transaction_and_get_digest(&test_cluster).await;
    let digest2 = execute_transaction_and_get_digest(&test_cluster).await;
    test_cluster.wait_for_checkpoint(3, None).await;

    // Test: get single transaction
    let transactions = client
        .get_transactions(&[digest1], None)
        .await
        .expect("Failed to get transaction");
    assert_eq!(
        transactions.body().len(),
        1,
        "Expected exactly one transaction"
    );
    assert_eq!(
        transactions.body()[0]
            .transaction()
            .expect("Failed to get transaction from executed transaction")
            .digest()
            .expect("Failed to get digest from transaction"),
        digest1,
        "Transaction digest should match requested digest"
    );
    assert!(
        !transactions.body()[0]
            .signatures()
            .expect("Failed to get signatures from transaction")
            .signatures
            .is_empty(),
        "Signatures should be present"
    );

    // Test: get batch of transactions
    let transactions = client
        .get_transactions(&[digest1, digest2], None)
        .await
        .expect("Failed to get transactions");
    assert_eq!(
        transactions.body().len(),
        2,
        "Expected exactly two transactions"
    );
    assert_eq!(
        transactions.body()[0]
            .transaction()
            .expect("Failed to get transaction from executed transaction")
            .digest()
            .expect("Failed to get digest from first transaction"),
        digest1,
        "First transaction should match first digest"
    );
    assert_eq!(
        transactions.body()[1]
            .transaction()
            .expect("Failed to get transaction from executed transaction")
            .digest()
            .expect("Failed to get digest from second transaction"),
        digest2,
        "Second transaction should match second digest"
    );

    // Test: empty input returns an error
    let err = client
        .get_transactions(&[], None)
        .await
        .expect_err("Empty input should return an error");
    assert!(
        matches!(err, iota_grpc_client::Error::EmptyRequest),
        "Expected EmptyRequest error, got: {err}"
    );

    // Test: nonexistent transaction returns not-found error
    let fake_digest = Digest::new([0u8; 32]);
    let result = client.get_transactions(&[fake_digest], None).await;
    assert_server_not_found(result);

    // Test: mixed valid/invalid returns error
    let fake_digest = Digest::new([0u8; 32]);
    let result = client.get_transactions(&[digest1, fake_digest], None).await;
    assert!(
        result.is_err(),
        "Mixed valid/invalid should return an error when encountering invalid digest"
    );

    // Test: response fields match the default mask (transaction, signatures,
    // checkpoint, timestamp).
    let transactions = client
        .get_transactions(&[digest1], None)
        .await
        .expect("Failed to get transaction");
    let tx = &transactions.body()[0];
    assert_eq!(
        tx.transaction()
            .expect("Failed to get transaction from executed transaction")
            .digest()
            .expect("Failed to get digest from transaction"),
        digest1,
        "Digest should match"
    );
    assert!(
        !tx.signatures()
            .expect("Failed to get signatures from transaction")
            .signatures
            .is_empty(),
        "Signatures should be present"
    );
    assert!(
        tx.checkpoint.is_some(),
        "Checkpoint should be present after finalization"
    );
    assert!(
        tx.timestamp_ms()
            .expect("Failed to get timestamp from transaction")
            > 0,
        "Timestamp should be present after finalization"
    );

    // Test: invalid read mask causes deserialization error
    let result = client
        .get_transactions(&[digest1], Some("transaction.digest"))
        .await;

    let transactions = result.expect("request should work");
    let conversion_result = transactions.body()[0]
        .transaction()
        .expect("Failed to get transaction from executed transaction")
        .transaction()
        .map_err(Into::into);

    assert_proto_conversion_error(conversion_result);
}
