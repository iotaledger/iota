// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::read_mask_fields::{TransactionField, TransactionReadMask};
use iota_macros::sim_test;
use iota_sdk_types::TransactionDigest;

use super::{
    super::utils::{execute_transaction_and_get_digest, setup_grpc_test},
    common::{assert_proto_conversion_error, assert_server_not_found},
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
        .get_transactions([digest1], TransactionReadMask::default())
        .await
        .expect("Failed to get transaction");
    assert_eq!(
        transactions.body().len(),
        1,
        "Expected exactly one transaction"
    );
    let tx = transactions.body()[0]
        .as_ref()
        .expect("Transaction should be found");
    assert_eq!(
        tx.transaction()
            .expect("Failed to get transaction from executed transaction")
            .digest()
            .expect("Failed to get digest from transaction"),
        digest1,
        "Transaction digest should match requested digest"
    );
    assert!(
        !tx.signatures()
            .expect("Failed to get signatures from transaction")
            .signatures
            .is_empty(),
        "Signatures should be present"
    );

    // Test: get batch of transactions
    let transactions = client
        .get_transactions([digest1, digest2], TransactionReadMask::default())
        .await
        .expect("Failed to get transactions");
    assert_eq!(
        transactions.body().len(),
        2,
        "Expected exactly two transactions"
    );
    assert_eq!(
        transactions.body()[0]
            .as_ref()
            .expect("First transaction should be found")
            .transaction()
            .expect("Failed to get transaction from executed transaction")
            .digest()
            .expect("Failed to get digest from first transaction"),
        digest1,
        "First transaction should match first digest"
    );
    assert_eq!(
        transactions.body()[1]
            .as_ref()
            .expect("Second transaction should be found")
            .transaction()
            .expect("Failed to get transaction from executed transaction")
            .digest()
            .expect("Failed to get digest from second transaction"),
        digest2,
        "Second transaction should match second digest"
    );

    // Test: empty input returns an error
    let err = client
        .get_transactions([], TransactionReadMask::default())
        .await
        .expect_err("Empty input should return an error");
    assert!(
        matches!(err, iota_grpc_client::Error::EmptyRequest),
        "Expected EmptyRequest error, got: {err}"
    );

    // Test: a nonexistent transaction is reported against the digest that asked
    // for it, not as a failure of the call
    let fake_digest = TransactionDigest::new([0u8; 32]);
    let mut results = client
        .get_transactions([fake_digest], TransactionReadMask::default())
        .await
        .expect("The call itself should succeed")
        .into_inner();
    assert_eq!(results.len(), 1, "Expected one result per requested digest");
    assert_server_not_found(results.pop().expect("Length asserted above"));

    // Test: a missing transaction fails only its own slot, leaving the
    // transactions the node could serve intact
    let fake_digest = TransactionDigest::new([0u8; 32]);
    let mut results = client
        .get_transactions([digest1, fake_digest], TransactionReadMask::default())
        .await
        .expect("The call itself should succeed")
        .into_inner();
    assert_eq!(results.len(), 2, "Expected one result per requested digest");
    let missing = results.pop().expect("Length asserted above");
    let found = results.pop().expect("Length asserted above");
    assert!(
        found.is_ok(),
        "The transaction that exists should still be returned, got: {found:?}"
    );
    assert_server_not_found(missing);

    // Test: response fields match the default mask (transaction, signatures,
    // checkpoint, timestamp).
    let transactions = client
        .get_transactions([digest1], TransactionReadMask::default())
        .await
        .expect("Failed to get transaction");
    let tx = transactions.body()[0]
        .as_ref()
        .expect("Transaction should be found");
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
        .get_transactions([digest1], TransactionField::TRANSACTION_DIGEST)
        .await;

    let transactions = result.expect("request should work");
    let conversion_result = transactions.body()[0]
        .as_ref()
        .expect("Transaction should be found")
        .transaction()
        .expect("Failed to get transaction from executed transaction")
        .transaction()
        .map_err(Into::into);

    assert_proto_conversion_error(conversion_result);
}
