// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
    v0::{
        bcs::BcsData,
        transaction::Transaction as ProtoTransaction,
        transaction_execution_service::{
            SimulateTransactionRequest, SimulateTransactionResponse,
            transaction_execution_service_client::TransactionExecutionServiceClient,
        },
    },
};
use iota_macros::sim_test;
use iota_types::transaction::{TransactionData, TransactionDataAPI};
use prost_types::FieldMask;
use test_cluster::TestClusterBuilder;

use crate::{impl_field_presence_checker, utils::assert_field_presence};

// Generate the FieldPresenceChecker implementation for
// SimulateTransactionResponse (ExecutedTransaction checker is defined in
// mod.rs)
impl_field_presence_checker!(SimulateTransactionResponse {
    transaction: ExecutedTransaction,
    command_results,
});

async fn assert_simulate_transaction_request(
    client: &mut TransactionExecutionServiceClient<tonic::transport::Channel>,
    transaction: ProtoTransaction,
    read_mask: Option<FieldMask>,
    expected_fields: &[&str],
    scenario: &str,
) -> SimulateTransactionResponse {
    let response = client
        .simulate_transaction(SimulateTransactionRequest {
            transaction: Some(transaction),
            tx_checks: vec![],
            estimate_gas_budget: None,
            read_mask,
        })
        .await
        .unwrap()
        .into_inner();

    assert_field_presence(&response, expected_fields, scenario);
    response
}

#[sim_test]
async fn simulate_transaction_service_available() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint
    test_cluster.wait_for_checkpoint(1, None).await;

    // Test that we can connect to the TransactionExecutionService
    let client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url()).await;

    // The service should be available
    assert!(
        client.is_ok(),
        "TransactionExecutionService should be available"
    );
}

#[sim_test]
async fn simulate_transaction_simple_transfer() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let obj_to_send = gas.first().unwrap();
    let gas_obj = gas.last().unwrap();

    // Build a simple transfer transaction
    let tx_data = TransactionData::new_transfer(
        recipient,
        *obj_to_send,
        sender,
        *gas_obj,
        1_000_000, // gas budget
        1000,      // gas price
    );

    // Create the simulation request with BCS
    let transaction = ProtoTransaction {
        bcs: Some(BcsData {
            data: bcs::to_bytes(&tx_data).unwrap().into(),
        }),
        ..Default::default()
    };

    let request = SimulateTransactionRequest {
        transaction: Some(transaction),
        tx_checks: vec![],
        estimate_gas_budget: None,
        read_mask: None,
    };

    // Simulate the transaction
    let response = client
        .simulate_transaction(request)
        .await
        .unwrap()
        .into_inner();

    // Verify we got a response with effects
    // With default mask, simulate returns transaction with effects but excludes
    // input_objects and output_objects (they must be explicitly requested)
    let executed_transaction = response.transaction.unwrap();
    assert_field_presence(
        &executed_transaction,
        &[
            "digest",
            "transaction.digest",
            "transaction.bcs",
            "effects.digest",
            "effects.bcs",
        ],
        "simulate transfer - verify fields present with default mask",
    );
}

#[sim_test]
async fn simulate_transaction_with_gas_estimation() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let obj_to_send = gas.first().unwrap();
    let gas_obj = gas.last().unwrap();

    // Build a simple transfer transaction with a very high gas budget
    let tx_data = TransactionData::new_transfer(
        recipient,
        *obj_to_send,
        sender,
        *gas_obj,
        1_000_000_000, // very high gas budget
        1000,          // gas price
    );

    // Create the simulation request with gas estimation enabled
    let transaction = ProtoTransaction {
        bcs: Some(BcsData {
            data: bcs::to_bytes(&tx_data).unwrap().into(),
        }),
        ..Default::default()
    };

    let request = SimulateTransactionRequest {
        transaction: Some(transaction),
        tx_checks: vec![],
        estimate_gas_budget: Some(true),
        read_mask: None,
    };

    // Simulate the transaction
    let response = client
        .simulate_transaction(request)
        .await
        .unwrap()
        .into_inner();

    // Verify we got a response with effects and transaction
    // With default mask, simulate returns transaction with effects but excludes
    // input_objects and output_objects (they must be explicitly requested)
    let executed_transaction = response.transaction.unwrap();
    assert_field_presence(
        &executed_transaction,
        &[
            "digest",
            "transaction.digest",
            "transaction.bcs",
            "effects.digest",
            "effects.bcs",
        ],
        "simulate with gas estimation - verify fields present with default mask",
    );

    // Verify the returned transaction has an estimated budget
    // (it should be lower than the original 1_000_000_000)
    if let Some(tx_proto) = executed_transaction.transaction {
        if let Some(bcs_data) = tx_proto.bcs {
            let returned_tx: TransactionData = bcs::from_bytes(&bcs_data.data).unwrap();
            // The estimated budget should be much less than 1 billion
            assert!(returned_tx.gas_data().budget < 1_000_000_000);
            // But should be positive
            assert!(returned_tx.gas_data().budget > 0);
        }
    }
}

#[sim_test]
async fn simulate_transaction_readmask_scenarios() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let obj_to_send = gas.first().unwrap();
    let gas_obj = gas.last().unwrap();

    // Build a simple transfer transaction
    let tx_data = TransactionData::new_transfer(
        recipient,
        *obj_to_send,
        sender,
        *gas_obj,
        1_000_000, // gas budget
        1000,      // gas price
    );

    let create_transaction = || ProtoTransaction {
        bcs: Some(BcsData {
            data: bcs::to_bytes(&tx_data).unwrap().into(),
        }),
        ..Default::default()
    };

    // Tests for readmask scenarios
    // Default mask for simulate excludes input_objects and output_objects
    // For SimulateTransactionResponse, fields are "transaction" and
    // "command_results" Within transaction (ExecutedTransaction), available
    // fields are: digest, transaction, effects, input_objects, output_objects
    // (events, checkpoint, timestamp are not available for simulate)
    // input_objects and output_objects must be explicitly requested
    type TestCase<'a> = (&'a str, Option<FieldMask>, &'a [&'a str]);
    let test_cases: Vec<TestCase> = vec![
        // Default mask excludes input_objects and output_objects
        (
            "default readmask",
            None,
            &[
                "transaction.digest",
                "transaction.transaction.digest",
                "transaction.transaction.bcs",
                "transaction.effects.digest",
                "transaction.effects.bcs",
                "command_results",
            ],
        ),
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            &[],
        ),
        // Full readmask "transaction" excludes input_objects and output_objects
        // They must be explicitly requested
        (
            "full readmask",
            Some(FieldMask::from_paths(["transaction", "command_results"])),
            &[
                "transaction.digest",
                "transaction.transaction.digest",
                "transaction.transaction.bcs",
                "transaction.effects.digest",
                "transaction.effects.bcs",
                "command_results",
            ],
        ),
        (
            "partial readmask (transaction only)",
            Some(FieldMask::from_paths(["transaction"])),
            &[
                "transaction.digest",
                "transaction.transaction.digest",
                "transaction.transaction.bcs",
                "transaction.effects.digest",
                "transaction.effects.bcs",
            ],
        ),
        (
            "partial readmask (command_results only)",
            Some(FieldMask::from_paths(["command_results"])),
            &["command_results"],
        ),
        // Specific nested field masks - only the specified nested fields are returned
        (
            "nested readmask (transaction.digest only)",
            Some(FieldMask::from_paths(["transaction.digest"])),
            &["transaction.digest"],
        ),
        (
            "nested readmask (transaction.effects only)",
            Some(FieldMask::from_paths(["transaction.effects"])),
            &["transaction.effects.digest", "transaction.effects.bcs"],
        ),
        (
            "nested readmask (multiple specific fields)",
            Some(FieldMask::from_paths([
                "transaction.digest",
                "transaction.effects",
                "command_results",
            ])),
            &[
                "transaction.digest",
                "transaction.effects.digest",
                "transaction.effects.bcs",
                "command_results",
            ],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        assert_simulate_transaction_request(
            &mut client,
            create_transaction(),
            mask,
            expected_paths,
            scenario,
        )
        .await;
    }
}

#[sim_test]
async fn simulate_transaction_invalid_bcs() {
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

    // Request should fail with invalid BCS
    let result = client
        .simulate_transaction(SimulateTransactionRequest {
            transaction: Some(transaction),
            tx_checks: vec![],
            estimate_gas_budget: None,
            read_mask: None,
        })
        .await;

    assert!(
        result.is_err(),
        "Expected error for invalid BCS data, but got success"
    );
}

#[sim_test]
async fn simulate_transaction_empty_request() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    // Test empty/missing transaction
    let result = client
        .simulate_transaction(SimulateTransactionRequest {
            transaction: None,
            tx_checks: vec![],
            estimate_gas_budget: None,
            read_mask: None,
        })
        .await;

    assert!(
        result.is_err(),
        "Expected error for missing transaction, but got success"
    );
}

#[sim_test]
async fn simulate_transaction_nested_field_masks() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    // Wait for at least one checkpoint
    test_cluster.wait_for_checkpoint(1, None).await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let obj_to_send = gas.first().unwrap();
    let gas_obj = gas.last().unwrap();

    // Build a simple transfer transaction
    let tx_data = TransactionData::new_transfer(
        recipient,
        *obj_to_send,
        sender,
        *gas_obj,
        1_000_000, // gas budget
        1000,      // gas price
    );

    let create_transaction = || ProtoTransaction {
        bcs: Some(BcsData {
            data: bcs::to_bytes(&tx_data).unwrap().into(),
        }),
        ..Default::default()
    };

    // Tests for fine-grained nested field masks in simulate
    // These test the ability to selectively include specific nested fields
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
        // Test combination with command_results (simulate-specific field)
        (
            "nested: effects.digest with command_results",
            Some(FieldMask::from_paths([
                "transaction.effects.digest",
                "command_results",
            ])),
            &["transaction.effects.digest", "command_results"],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        assert_simulate_transaction_request(
            &mut client,
            create_transaction(),
            mask,
            expected_paths,
            scenario,
        )
        .await;
    }
}
