// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::panic;

use iota_macros::sim_test;
use iota_sdk_types::Digest;

use super::{
    super::utils::setup_grpc_test,
    common::{
        assert_proto_conversion_error, assert_server_not_found, execute_transaction_and_get_digest,
        is_success,
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
    assert_eq!(transactions.len(), 1, "Expected exactly one transaction");
    assert_eq!(
        transactions[0]
            .digest()
            .unwrap_or_else(|_| panic!("Failed to get digest from transaction")),
        digest1,
        "Transaction digest should match requested digest"
    );
    assert!(
        !transactions[0]
            .signatures()
            .expect("Failed to get signatures from transaction")
            .is_empty(),
        "Signatures should be present"
    );

    // Test: get batch of transactions
    let transactions = client
        .get_transactions(&[digest1, digest2], None)
        .await
        .expect("Failed to get transactions");
    assert_eq!(transactions.len(), 2, "Expected exactly two transactions");
    assert_eq!(
        transactions[0]
            .digest()
            .expect("Failed to get digest from first transaction"),
        digest1,
        "First transaction should match first digest"
    );
    assert_eq!(
        transactions[1]
            .digest()
            .expect("Failed to get digest from second transaction"),
        digest2,
        "Second transaction should match second digest"
    );

    // Test: empty input returns empty result
    let transactions = client
        .get_transactions(&[], None)
        .await
        .expect("Empty input should succeed");
    assert!(
        transactions.is_empty(),
        "Empty input should return empty result"
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

    // Test: response fields are complete
    let transactions = client
        .get_transactions(&[digest1], None)
        .await
        .expect("Failed to get transaction");
    let tx = &transactions[0];
    assert_eq!(
        tx.digest().expect("Failed to get digest from transaction"),
        digest1,
        "Digest should match"
    );
    assert!(
        !tx.signatures()
            .expect("Failed to get signatures from transaction")
            .is_empty(),
        "Signatures should be present"
    );
    assert!(
        is_success(
            tx.effects()
                .expect("Failed to get effects from transaction")
                .status()
        ),
        "Transaction should have succeeded"
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

    let conversion_result = result.expect("request should work")[0]
        .transaction()
        .map_err(Into::into);

    assert_proto_conversion_error(conversion_result);
}
