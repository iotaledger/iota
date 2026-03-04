// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::EXECUTE_TRANSACTION_READ_MASK,
    v0::{
        bcs::BcsData,
        signatures::{UserSignature, UserSignatures},
        transaction::Transaction as ProtoTransaction,
        transaction_execution_service::{
            ExecuteTransactionRequest, ExecuteTransactionResponse,
            transaction_execution_service_client::TransactionExecutionServiceClient,
        },
    },
};
use iota_macros::sim_test;
use iota_test_transaction_builder::make_transfer_iota_transaction;
use prost_types::FieldMask;

use crate::utils::{assert_field_presence, comma_separated_field_mask_to_paths, setup_grpc_test};

async fn assert_execute_transaction_request(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    transaction: ProtoTransaction,
    signatures: UserSignatures,
    read_mask: Option<FieldMask>,
    expected_fields: &[&str],
    scenario: &str,
) -> ExecuteTransactionResponse {
    let response = exec_client
        .execute_transaction({
            let mut req = ExecuteTransactionRequest::default()
                .with_transaction(transaction)
                .with_signatures(signatures);
            if let Some(mask) = read_mask {
                req = req.with_read_mask(mask);
            }
            req
        })
        .await
        .unwrap()
        .into_inner();

    assert_field_presence(&response, expected_fields, &[], scenario);
    response
}

#[sim_test]
async fn execute_transaction_readmask_scenarios() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    // ExecuteTransactionResponse is field_mask_transparent, so paths are relative
    // to the inner ExecutedTransaction (e.g. "effects", not
    // "executed_transaction.effects").
    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "default readmask",
            None,
            // Bare paths with nested checkers (effects, events) auto-recurse into
            // all their sub-fields; "transaction.digest" is specific (bcs absent).
            comma_separated_field_mask_to_paths(EXECUTE_TRANSACTION_READ_MASK),
        ),
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            vec![],
        ),
        // Request all ExecutedTransaction fields explicitly.
        // All fields are present even if empty (e.g., events for simple transfers).
        (
            "full readmask",
            Some(FieldMask::from_paths([
                "transaction",
                "signatures",
                "effects",
                "events",
                "input_objects",
                "output_objects",
            ])),
            vec![
                "transaction",
                "signatures",
                "effects",
                "events",
                "input_objects",
                "output_objects",
            ],
        ),
        // Specific nested field masks — only the specified nested fields are returned.
        (
            "nested readmask (multiple specific fields)",
            Some(FieldMask::from_paths(["transaction.digest", "effects"])),
            vec!["transaction.digest", "effects"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        // Create a fresh transaction for each test case to avoid duplicate transaction
        // errors
        let txn =
            make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount))
                .await;

        let transaction = ProtoTransaction::default()
            .with_bcs(BcsData::default().with_data(bcs::to_bytes(txn.transaction_data()).unwrap()));

        let signatures = UserSignatures::default().with_signatures(
            txn.tx_signatures()
                .iter()
                .map(|s| {
                    UserSignature::default()
                        .with_bcs(BcsData::default().with_data(bcs::to_bytes(s).unwrap()))
                })
                .collect(),
        );

        assert_execute_transaction_request(
            &mut exec_client,
            transaction,
            signatures,
            mask,
            &expected_paths,
            scenario,
        )
        .await;
    }
}

#[sim_test]
async fn execute_transaction_invalid_bcs() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    // Create a valid transaction to get real signatures
    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    // Create transaction with invalid BCS data
    let transaction = ProtoTransaction::default().with_bcs(
        BcsData::default().with_data(vec![0xff, 0xff, 0xff]), // Invalid BCS
    );

    // Use valid signatures from the real transaction
    let signatures = UserSignatures::default().with_signatures(
        txn.tx_signatures()
            .iter()
            .map(|s| {
                UserSignature::default()
                    .with_bcs(BcsData::default().with_data(bcs::to_bytes(s).unwrap()))
            })
            .collect(),
    );

    // Request should fail with invalid BCS
    let result = exec_client
        .execute_transaction(
            ExecuteTransactionRequest::default()
                .with_transaction(transaction)
                .with_signatures(signatures),
        )
        .await;

    assert!(
        result.is_err(),
        "Expected error for invalid BCS data, but got success"
    );
}

#[sim_test]
async fn execute_transaction_invalid_signatures() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    let transaction = ProtoTransaction::default()
        .with_bcs(BcsData::default().with_data(bcs::to_bytes(txn.transaction_data()).unwrap()));

    // Create invalid signatures (wrong signature data)
    let signatures =
        UserSignatures::default().with_signatures(vec![UserSignature::default().with_bcs(
            BcsData::default().with_data(vec![0x00; 64]), // Invalid signature
        )]);

    // Request should fail with invalid signatures
    let result = exec_client
        .execute_transaction(
            ExecuteTransactionRequest::default()
                .with_transaction(transaction)
                .with_signatures(signatures),
        )
        .await;

    assert!(
        result.is_err(),
        "Expected error for invalid signatures, but got success"
    );
}

#[sim_test]
async fn execute_transaction_empty_request() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    // Create a valid transaction to get real signatures
    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    // Use valid signatures from the real transaction
    let signatures = UserSignatures::default().with_signatures(
        txn.tx_signatures()
            .iter()
            .map(|s| {
                UserSignature::default()
                    .with_bcs(BcsData::default().with_data(bcs::to_bytes(s).unwrap()))
            })
            .collect(),
    );

    // Test missing transaction with valid signatures
    let result = exec_client
        .execute_transaction(ExecuteTransactionRequest::default().with_signatures(signatures))
        .await;

    assert!(
        result.is_err(),
        "Expected error for missing transaction, but got success"
    );
}
