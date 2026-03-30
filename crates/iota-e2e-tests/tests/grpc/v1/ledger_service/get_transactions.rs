// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::StreamExt;
use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::GET_TRANSACTIONS_READ_MASK,
    v1::ledger_service::{
        GetTransactionsRequest, GetTransactionsResponse, TransactionRequest, TransactionRequests,
        ledger_service_client::LedgerServiceClient, transaction_result,
    },
};
use iota_macros::sim_test;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::digests::TransactionDigest;
use prost_types::FieldMask;
use test_cluster::TestCluster;

use crate::utils::{assert_field_presence, comma_separated_field_mask_to_paths, setup_grpc_test};

/// Helper to create a test transaction and return its digest
async fn create_test_transaction(test_cluster: &TestCluster) -> TransactionDigest {
    let (sender, gas) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    let transaction_data = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, sender)
        .build();
    let signed_transaction = test_cluster.wallet.sign_transaction(&transaction_data);
    let transaction_digest = *signed_transaction.digest();
    test_cluster
        .wallet
        .execute_transaction_may_fail(signed_transaction)
        .await
        .unwrap();
    transaction_digest
}

/// Helper function to make GetTransactions requests and validate responses..
async fn assert_get_transactions_request(
    ledger_client: &mut LedgerServiceClient<iota_grpc_client::InterceptedChannel>,
    digests: Vec<TransactionDigest>,
    read_mask: Option<FieldMask>,
    max_message_size_bytes: Option<u32>,
    expected_field_mask_paths: &[&str],
    scenario: &str,
) -> Vec<GetTransactionsResponse> {
    let mut request = GetTransactionsRequest::default().with_requests(
        TransactionRequests::default().with_requests(
            digests
                .iter()
                .map(|d| {
                    TransactionRequest::default().with_digest({
                        iota_grpc_types::v1::types::Digest::default()
                            .with_digest(d.inner().to_vec())
                    })
                })
                .collect(),
        ),
    );
    if let Some(mask) = read_mask {
        request = request.with_read_mask(mask);
    }
    if let Some(size) = max_message_size_bytes {
        request = request.with_max_message_size_bytes(size);
    };

    let mut stream = ledger_client
        .get_transactions(request)
        .await
        .unwrap()
        .into_inner();

    let mut responses = Vec::new();
    let mut response_count = 0;

    // Loop through all responses until has_next is false
    while let Some(response) = stream.next().await {
        let response = response.unwrap();
        response_count += 1;

        // Assert all returned transactions have the expected fields
        for (idx, tx_result) in response.transaction_results.iter().enumerate() {
            if let Some(transaction_result::Result::ExecutedTransaction(transaction)) =
                &tx_result.result
            {
                assert_field_presence(
                    transaction,
                    expected_field_mask_paths,
                    &[],
                    &format!("{scenario} (response {response_count}, transaction {idx})"),
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
async fn get_transactions_readmask_scenarios() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Create a test transaction
    let transaction_digest = create_test_transaction(&test_cluster).await;

    // Tests for single-transaction readmask scenarios
    // Note: When a parent field is specified without nested paths (e.g.,
    // "effects"), FieldMaskTree treats it as a wildcard and includes all nested
    // fields. So "effects" means "effects.digest" AND "effects.bcs".
    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "default readmask",
            None,
            comma_separated_field_mask_to_paths(GET_TRANSACTIONS_READ_MASK),
        ),
        // Empty readmask - returns no fields
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            vec![],
        ),
        (
            "full readmask",
            Some(FieldMask::from_paths([
                "transaction",
                "signatures",
                "effects",
                "events",
                "checkpoint",
                "timestamp",
            ])),
            vec![
                "transaction",
                "signatures",
                "effects",
                "events",
                "checkpoint",
                "timestamp",
            ],
        ),
        // Partial readmask: digest only
        (
            "partial readmask (digest only)",
            Some(FieldMask::from_paths(["transaction.digest"])),
            vec!["transaction.digest"],
        ),
        // Partial readmask: effects.digest only (specific nested field)
        (
            "partial readmask (effects.digest only)",
            Some(FieldMask::from_paths(["effects.digest"])),
            vec!["effects.digest"],
        ),
        // Partial readmask: effects wildcard (all nested fields)
        (
            "partial readmask (effects wildcard)",
            Some(FieldMask::from_paths(["effects"])),
            vec!["effects"],
        ),
        // Partial readmask: transaction + signatures
        (
            "partial readmask (transaction + signatures)",
            Some(FieldMask::from_paths(["transaction.digest", "signatures"])),
            vec!["transaction.digest", "signatures"],
        ),
        // Partial readmask: checkpoint + timestamp (metadata only)
        (
            "partial readmask (checkpoint + timestamp)",
            Some(FieldMask::from_paths(["checkpoint", "timestamp"])),
            vec!["checkpoint", "timestamp"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        let responses = assert_get_transactions_request(
            &mut ledger_client,
            vec![transaction_digest],
            mask,
            None,
            &expected_paths,
            scenario,
        )
        .await;

        let total_transactions: usize = responses.iter().map(|r| r.transaction_results.len()).sum();
        assert_eq!(total_transactions, 1, "{scenario}: expected 1 transaction");
    }
}

#[sim_test]
async fn get_transactions_batch() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Create multiple test transactions
    let mut digests = Vec::new();
    for _ in 0..3 {
        let digest = create_test_transaction(&test_cluster).await;
        digests.push(digest);
    }

    // Test batch request with partial readmask.
    let responses = assert_get_transactions_request(
        &mut ledger_client,
        digests.clone(),
        Some(FieldMask::from_paths(["transaction.digest", "effects"])),
        None,
        &["transaction.digest", "effects"],
        "batch with 3 transactions",
    )
    .await;

    let total_transactions: usize = responses.iter().map(|r| r.transaction_results.len()).sum();
    assert_eq!(
        total_transactions, 3,
        "Should have received 3 transactions in batch"
    );
}

#[sim_test]
async fn get_transactions_streaming() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Create multiple test transactions to have enough data for streaming
    let mut digests = Vec::new();
    for _ in 0..10 {
        let digest = create_test_transaction(&test_cluster).await;
        digests.push(digest);
    }

    // Request each transaction multiple times to create larger payload
    let mut all_digests = Vec::new();
    for _ in 0..100 {
        all_digests.extend(digests.iter().cloned());
    }

    // Test streaming by requesting many transactions with full readmask.
    // Use minimum allowed message size to maximize multi-message streaming.
    let responses = assert_get_transactions_request(
        &mut ledger_client,
        all_digests,
        Some(FieldMask::from_paths([
            "transaction",
            "signatures",
            "effects",
            "checkpoint",
            "timestamp",
            "input_objects",
            "output_objects",
        ])),
        Some(1024 * 1024_u32), // 1MB (minimum allowed)
        &[
            "transaction",
            "signatures",
            "effects",
            "checkpoint",
            "timestamp",
            "input_objects",
            "output_objects",
        ],
        "streaming with 1000 transactions",
    )
    .await;

    // Verify we got all 1000 results
    let total_transactions: usize = responses.iter().map(|r| r.transaction_results.len()).sum();
    assert_eq!(
        total_transactions, 1000,
        "Should have received 1000 transactions"
    );

    // Verify the number of response messages is greater than 1 (i.e., streaming
    // occurred)
    assert!(
        responses.len() > 1,
        "Should have received multiple response messages for streaming"
    );
}

#[sim_test]
async fn get_transactions_empty_request() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    // Test empty request list
    let responses = assert_get_transactions_request(
        &mut ledger_client,
        vec![],
        None,
        None,
        &[],
        "empty request",
    )
    .await;

    // Should return single response with 0 transactions
    assert_eq!(responses.len(), 1, "Should have 1 response");
    assert_eq!(
        responses[0].transaction_results.len(),
        0,
        "Should have 0 transactions"
    );
    assert!(
        !responses[0].has_next,
        "has_next should be false for empty request"
    );
}

#[sim_test]
async fn get_transactions_nonexistent() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut ledger_client = client.ledger_service_client();

    // Request non-existent transactions
    let fake_digest1 = TransactionDigest::new([0u8; 32]);
    let fake_digest2 = TransactionDigest::new([1u8; 32]);

    let request = GetTransactionsRequest::default().with_requests(
        TransactionRequests::default().with_requests(vec![
            TransactionRequest::default().with_digest({
                iota_grpc_types::v1::types::Digest::default()
                    .with_digest(fake_digest1.inner().to_vec())
            }),
            TransactionRequest::default().with_digest({
                iota_grpc_types::v1::types::Digest::default()
                    .with_digest(fake_digest2.inner().to_vec())
            }),
        ]),
    );

    let mut stream = ledger_client
        .get_transactions(request)
        .await
        .unwrap()
        .into_inner();

    let mut responses = Vec::new();
    while let Some(response) = stream.next().await {
        let response = response.unwrap();
        let has_next = response.has_next;
        responses.push(response);
        if !has_next {
            break;
        }
    }

    // Verify all results contain errors (not transactions)
    let mut error_count = 0;
    for response in &responses {
        for tx_result in &response.transaction_results {
            assert!(
                matches!(tx_result.result, Some(transaction_result::Result::Error(_))),
                "Expected error for non-existent transaction"
            );
            assert!(
                !matches!(
                    tx_result.result,
                    Some(transaction_result::Result::ExecutedTransaction(_))
                ),
                "Expected no transaction for non-existent digest"
            );

            if let Some(transaction_result::Result::Error(error)) = &tx_result.result {
                // Verify error code is NOT_FOUND (5)
                assert_eq!(
                    error.code, 5,
                    "Error code should be NOT_FOUND (5), got: {}",
                    error.code
                );
            }
            error_count += 1;
        }
    }

    assert_eq!(error_count, 2, "Should receive 2 errors");
}

#[sim_test]
async fn get_transactions_mixed_valid_invalid() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Create a real transaction
    let real_digest = create_test_transaction(&test_cluster).await;

    // Request mix of valid and invalid digests
    let fake_digest = TransactionDigest::new([0u8; 32]);

    let request = GetTransactionsRequest::default()
        .with_requests(TransactionRequests::default().with_requests(vec![
            // Valid digest first
            TransactionRequest::default().with_digest({
                iota_grpc_types::v1::types::Digest::default()
                    .with_digest(real_digest.inner().to_vec())
            }),
            // Invalid digest
            TransactionRequest::default().with_digest({
                iota_grpc_types::v1::types::Digest::default()
                    .with_digest(fake_digest.inner().to_vec())
            }),
        ]))
        .with_read_mask(FieldMask::from_paths(["transaction.digest"]));

    let mut stream = ledger_client
        .get_transactions(request)
        .await
        .unwrap()
        .into_inner();

    let mut all_results = Vec::new();
    while let Some(response) = stream.next().await {
        let response = response.unwrap();
        let has_next = response.has_next;
        for tx_result in response.transaction_results {
            all_results.push(tx_result);
        }
        if !has_next {
            break;
        }
    }

    // Should have exactly 2 results
    assert_eq!(all_results.len(), 2, "Should have 2 results total");

    // First result should be a transaction (valid digest)
    assert!(
        matches!(
            all_results[0].result,
            Some(transaction_result::Result::ExecutedTransaction(_))
        ),
        "First result should be a valid transaction"
    );
    assert!(
        !matches!(
            all_results[0].result,
            Some(transaction_result::Result::Error(_))
        ),
        "First result should not have an error"
    );

    // Second result should be an error (invalid digest)
    assert!(
        matches!(
            all_results[1].result,
            Some(transaction_result::Result::Error(_))
        ),
        "Second result should be an error"
    );
    assert!(
        !matches!(
            all_results[1].result,
            Some(transaction_result::Result::ExecutedTransaction(_))
        ),
        "Second result should not have a transaction"
    );

    // Verify error code is NOT_FOUND
    if let Some(transaction_result::Result::Error(error)) = &all_results[1].result {
        assert_eq!(
            error.code, 5,
            "Error code should be NOT_FOUND (5), got: {}",
            error.code
        );
    }
}
