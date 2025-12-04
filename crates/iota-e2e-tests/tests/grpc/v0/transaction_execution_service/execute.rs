// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
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
use test_cluster::TestClusterBuilder;

use crate::{impl_field_presence_checker, utils::assert_field_presence};

// Generate the FieldPresenceChecker implementation for
// ExecuteTransactionResponse
impl_field_presence_checker!(ExecuteTransactionResponse {
    transaction: ExecutedTransaction,
});

async fn assert_execute_transaction_request(
    client: &mut TransactionExecutionServiceClient<tonic::transport::Channel>,
    transaction: ProtoTransaction,
    signatures: UserSignatures,
    read_mask: Option<FieldMask>,
    expected_fields: &[&str],
    scenario: &str,
) -> ExecuteTransactionResponse {
    let response = client
        .execute_transaction(ExecuteTransactionRequest {
            transaction: Some(transaction),
            signatures: Some(signatures),
            read_mask,
        })
        .await
        .unwrap()
        .into_inner();

    assert_field_presence(&response, expected_fields, scenario);
    response
}

#[sim_test]
async fn execute_transaction_transfer() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    let transaction = ProtoTransaction {
        bcs: Some(BcsData {
            data: bcs::to_bytes(txn.transaction_data()).unwrap().into(),
        }),
        ..Default::default()
    };

    let signatures = UserSignatures {
        signatures: txn
            .tx_signatures()
            .iter()
            .map(|s| UserSignature {
                bcs: Some(BcsData {
                    data: s.as_ref().to_vec().into(),
                }),
            })
            .collect(),
    };

    let _ = assert_execute_transaction_request(
        &mut client,
        transaction,
        signatures,
        Some(FieldMask::from_paths([
            "transaction.digest",
            "transaction.transaction",
            "transaction.signatures",
            "transaction.effects",
            "transaction.input_objects",
            "transaction.output_objects",
        ])),
        &[
            "transaction.digest",
            "transaction.transaction.digest",
            "transaction.transaction.bcs",
            "transaction.signatures",
            "transaction.effects.digest",
            "transaction.effects.bcs",
            "transaction.input_objects",
            "transaction.output_objects",
        ],
        "execute transaction transfer",
    )
    .await;
}

#[sim_test]
async fn execute_transaction_readmask_scenarios() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    // Tests for readmask scenarios
    // Default mask for execute is "transaction.effects" - only transaction with
    // effects is returned Empty mask returns nothing
    // Explicit masks control which fields are included
    type TestCase<'a> = (&'a str, Option<FieldMask>, &'a [&'a str]);
    let test_cases: Vec<TestCase> = vec![
        // Default mask is "transaction.effects", so only transaction.effects with all subfields is
        // returned
        (
            "default readmask",
            None,
            &["transaction.effects.digest", "transaction.effects.bcs"],
        ),
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            &[],
        ),
        // Full readmask "transaction" returns all fields EXCEPT input_objects and output_objects
        (
            "full readmask",
            Some(FieldMask::from_paths(["transaction"])),
            &[
                "transaction.digest",
                "transaction.transaction.digest",
                "transaction.transaction.bcs",
                "transaction.signatures",
                "transaction.effects.digest",
                "transaction.effects.bcs",
            ],
        ),
        // Specific nested field masks - only the specified nested fields are returned
        (
            "nested readmask (transaction.digest only)",
            Some(FieldMask::from_paths(["transaction.digest"])),
            &["transaction.digest"],
        ),
        (
            "nested readmask (multiple specific fields)",
            Some(FieldMask::from_paths([
                "transaction.digest",
                "transaction.effects",
            ])),
            &[
                "transaction.digest",
                "transaction.effects.digest",
                "transaction.effects.bcs",
            ],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        // Create a fresh transaction for each test case to avoid duplicate transaction
        // errors
        let txn =
            make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount))
                .await;

        let transaction = ProtoTransaction {
            bcs: Some(BcsData {
                data: bcs::to_bytes(txn.transaction_data()).unwrap().into(),
            }),
            ..Default::default()
        };

        let signatures = UserSignatures {
            signatures: txn
                .tx_signatures()
                .iter()
                .map(|s| UserSignature {
                    bcs: Some(BcsData {
                        data: s.as_ref().to_vec().into(),
                    }),
                })
                .collect(),
        };

        assert_execute_transaction_request(
            &mut client,
            transaction,
            signatures,
            mask,
            expected_paths,
            scenario,
        )
        .await;
    }
}

#[sim_test]
async fn execute_transaction_invalid_bcs() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Create transaction with invalid BCS data
    let transaction = ProtoTransaction {
        bcs: Some(BcsData {
            data: vec![0xff, 0xff, 0xff].into(), // Invalid BCS
        }),
        ..Default::default()
    };

    let signatures = UserSignatures {
        signatures: vec![UserSignature {
            bcs: Some(BcsData {
                data: vec![0x00].into(),
            }),
        }],
    };

    // Request should fail with invalid BCS
    let result = client
        .execute_transaction(ExecuteTransactionRequest {
            transaction: Some(transaction),
            signatures: Some(signatures),
            read_mask: None,
        })
        .await;

    assert!(
        result.is_err(),
        "Expected error for invalid BCS data, but got success"
    );
}

#[sim_test]
async fn execute_transaction_invalid_signatures() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    let transaction = ProtoTransaction {
        bcs: Some(BcsData {
            data: bcs::to_bytes(txn.transaction_data()).unwrap().into(),
        }),
        ..Default::default()
    };

    // Create invalid signatures (wrong signature data)
    let signatures = UserSignatures {
        signatures: vec![UserSignature {
            bcs: Some(BcsData {
                data: vec![0x00; 64].into(), // Invalid signature
            }),
        }],
    };

    // Request should fail with invalid signatures
    let result = client
        .execute_transaction(ExecuteTransactionRequest {
            transaction: Some(transaction),
            signatures: Some(signatures),
            read_mask: None,
        })
        .await;

    assert!(
        result.is_err(),
        "Expected error for invalid signatures, but got success"
    );
}

#[sim_test]
async fn execute_transaction_empty_request() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Test empty/missing transaction
    let result = client
        .execute_transaction(ExecuteTransactionRequest {
            transaction: None,
            signatures: Some(UserSignatures { signatures: vec![] }),
            read_mask: None,
        })
        .await;

    assert!(
        result.is_err(),
        "Expected error for missing transaction, but got success"
    );
}

#[sim_test]
async fn execute_transaction_nested_field_masks() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    // Tests for fine-grained nested field masks
    // These test the ability to selectively include specific nested fields
    // within a message (e.g., only effects.digest, not effects.bcs)
    type TestCase<'a> = (&'a str, Option<FieldMask>, &'a [&'a str]);
    let test_cases: Vec<TestCase> = vec![
        // Test nested field masks within effects
        (
            "nested: effects.digest only",
            Some(FieldMask::from_paths(["transaction.effects.digest"])),
            &["transaction.effects.digest"],
        ),
        (
            "nested: effects.bcs only",
            Some(FieldMask::from_paths(["transaction.effects.bcs"])),
            &["transaction.effects.bcs"],
        ),
        (
            "nested: effects.digest and effects.bcs",
            Some(FieldMask::from_paths([
                "transaction.effects.digest",
                "transaction.effects.bcs",
            ])),
            &["transaction.effects.digest", "transaction.effects.bcs"],
        ),
        // Test nested field masks within transaction
        (
            "nested: transaction.digest only",
            Some(FieldMask::from_paths(["transaction.transaction.digest"])),
            &["transaction.transaction.digest"],
        ),
        (
            "nested: transaction.bcs only",
            Some(FieldMask::from_paths(["transaction.transaction.bcs"])),
            &["transaction.transaction.bcs"],
        ),
        // Test combination of nested fields from different messages
        (
            "nested: mixed fields from effects and transaction",
            Some(FieldMask::from_paths([
                "transaction.effects.digest",
                "transaction.transaction.bcs",
            ])),
            &["transaction.effects.digest", "transaction.transaction.bcs"],
        ),
        // Test deep nesting with top-level fields
        (
            "nested: mixed with top-level digest",
            Some(FieldMask::from_paths([
                "transaction.digest",
                "transaction.effects.digest",
            ])),
            &["transaction.digest", "transaction.effects.digest"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        // Create a fresh transaction for each test case
        let txn =
            make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount))
                .await;

        let transaction = ProtoTransaction {
            bcs: Some(BcsData {
                data: bcs::to_bytes(txn.transaction_data()).unwrap().into(),
            }),
            ..Default::default()
        };

        let signatures = UserSignatures {
            signatures: txn
                .tx_signatures()
                .iter()
                .map(|s| UserSignature {
                    bcs: Some(BcsData {
                        data: s.as_ref().to_vec().into(),
                    }),
                })
                .collect(),
        };

        assert_execute_transaction_request(
            &mut client,
            transaction,
            signatures,
            mask,
            expected_paths,
            scenario,
        )
        .await;
    }
}
