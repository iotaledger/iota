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
use iota_sdk_types::{Digest, TransactionDigest};
use prost_types::FieldMask;

use crate::utils::{
    assert_field_presence, assert_transfer_derived_changes, comma_separated_field_mask_to_paths,
    execute_transaction_and_get_digest, normalize_grpc_balance_changes, setup_grpc_test,
};

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
    let transaction_digest = execute_transaction_and_get_digest(&test_cluster).await;

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
async fn get_transactions_derived_changes() {
    use iota_test_transaction_builder::make_transfer_iota_transaction;
    use iota_types::transaction::TransactionDataAPI as _;

    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Transfer to a distinct recipient so the balance moves between two owners
    let recipient = iota_sdk_types::Address::random();
    let transaction =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(1000)).await;
    let sender = transaction.transaction_data().sender();
    let transaction_digest = *transaction.digest();
    test_cluster
        .wallet
        .execute_transaction_may_fail(transaction)
        .await
        .unwrap();

    // Requesting only the derived fields (plus effects for the gas charge)
    // must not leak the input/output objects they are computed from (asserted
    // absent by assert_get_transactions_request)
    let responses = assert_get_transactions_request(
        &mut ledger_client,
        vec![transaction_digest],
        Some(FieldMask::from_paths([
            "balance_changes",
            "object_changes",
            "effects",
        ])),
        None,
        &["balance_changes", "object_changes", "effects"],
        "derived changes only",
    )
    .await;

    let Some(transaction_result::Result::ExecutedTransaction(executed_transaction)) =
        &responses[0].transaction_results[0].result
    else {
        panic!("expected an executed transaction");
    };

    assert_transfer_derived_changes(
        executed_transaction,
        sender,
        recipient,
        1000,
        "get_transactions derived changes",
    );
}

#[sim_test]
async fn get_transactions_derived_changes_failed_transaction() {
    use iota_sdk_types::Command;
    use iota_types::{
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::{CallArg, TransactionData, TransactionDataAPI as _},
    };

    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Build a transaction that fails at execution: split more coins than the
    // input coin holds. It is still committed and charged gas.
    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.object_id);
    let gas_object = gas.last().unwrap();
    let coin_to_split = gas.first().unwrap();

    let mut builder = ProgrammableTransactionBuilder::new();
    let coin_arg = builder
        .obj(CallArg::ImmutableOrOwned(*coin_to_split))
        .unwrap();
    let huge_amount = builder.pure(u64::MAX).unwrap();
    builder.command(Command::new_split_coins(coin_arg, vec![huge_amount]));
    let transaction_data = TransactionData::new_programmable(
        sender,
        vec![*gas_object],
        builder.finish(),
        10_000_000,
        test_cluster.get_reference_gas_price().await,
    );
    let transaction = test_cluster.wallet.sign_transaction(&transaction_data);
    let transaction_digest = *transaction.digest();
    test_cluster
        .wallet
        .execute_transaction_may_fail(transaction)
        .await
        .unwrap();

    let responses = assert_get_transactions_request(
        &mut ledger_client,
        vec![transaction_digest],
        Some(FieldMask::from_paths([
            "balance_changes",
            "object_changes",
            "effects",
        ])),
        None,
        &["balance_changes", "object_changes", "effects"],
        "derived changes for failed transaction",
    )
    .await;

    let Some(transaction_result::Result::ExecutedTransaction(executed_transaction)) =
        &responses[0].transaction_results[0].result
    else {
        panic!("expected an executed transaction");
    };

    // A failed transaction reports exactly the gas charge and nothing else
    let gas = crate::utils::grpc_net_gas_usage(executed_transaction) as i128;
    assert!(gas > 0, "failed transaction should be charged gas: {gas}");
    assert_eq!(
        normalize_grpc_balance_changes(executed_transaction),
        vec![(
            iota_sdk_types::Owner::Address(sender),
            iota_types::gas_coin::GAS::type_tag(),
            -gas,
        )],
        "failed transaction should produce a single gas-only balance change"
    );

    // The intended split is rolled back; only sender-owned coin mutations
    // (gas smashing) remain
    let object_changes = crate::utils::normalize_grpc_object_changes(executed_transaction);
    assert!(
        !object_changes.is_empty(),
        "gas coin mutation should be reported"
    );
    assert!(
        object_changes.iter().all(|change| matches!(
            change,
            crate::utils::NormalizedObjectChange::Mutated { sender: s, owner, .. }
                if *s == sender && *owner == iota_sdk_types::Owner::Address(sender)
        )),
        "failed transaction should only report sender-owned mutations: {object_changes:?}"
    );
}

#[sim_test]
async fn get_transactions_batch() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut ledger_client = client.ledger_service_client();

    // Create multiple test transactions
    let mut digests = Vec::new();
    for _ in 0..3 {
        let digest = execute_transaction_and_get_digest(&test_cluster).await;
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
        let digest = execute_transaction_and_get_digest(&test_cluster).await;
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
    let fake_digest1 = Digest::new([0u8; 32]);
    let fake_digest2 = Digest::new([1u8; 32]);

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
    let real_digest = execute_transaction_and_get_digest(&test_cluster).await;

    // Request mix of valid and invalid digests
    let fake_digest = Digest::new([0u8; 32]);

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
