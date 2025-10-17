// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_grpc_api::client::WriteClient;
use iota_grpc_types::v0::write as grpc_write;
use iota_types::transaction::{TransactionData, TransactionDataAPI};
use test_cluster::TestCluster;

mod utils;
use utils::setup_test_cluster_and_client;

async fn setup_test_cluster_and_write_client() -> (TestCluster, WriteClient) {
    let (cluster, node_client) = setup_test_cluster_and_client().await;

    let write_client = node_client
        .write_client()
        .expect("Write client should be available");

    (cluster, write_client)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_write_service_execute_transaction() {
    let (cluster, mut write_client) = setup_test_cluster_and_write_client().await;

    let sender = cluster.get_address_0();
    let receiver = cluster.get_address_1();
    let amount = 1000u64;

    // Build a real transfer transaction using TestCluster's infrastructure
    let tx_data = cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .transfer_iota(Some(amount), receiver)
        .build();

    // Sign the transaction to get proper signatures
    let signed_tx = cluster.sign_transaction(&tx_data);

    // Extract real transaction bytes and signatures
    let tx_bytes =
        bcs::to_bytes(&signed_tx.data().intent_message().value).expect("BCS serialization failed");
    let signatures: Vec<Vec<u8>> = signed_tx
        .tx_signatures()
        .iter()
        .map(|sig| sig.as_ref().to_vec())
        .collect();

    // Test execute_transaction via WriteService with real transaction data
    let tx_result = tokio::time::timeout(Duration::from_secs(30), async {
        let request = grpc_write::ExecuteTransactionRequest {
            tx_bytes,
            signatures,
            options: Some(grpc_write::TransactionResponseOptions {
                show_input: false,
                show_raw_input: false,
                show_effects: true,
                show_events: false,
                show_object_changes: false,
                show_balance_changes: false,
                show_raw_effects: false,
            }),
            request_type: None, // Uses default: WaitForEffectsCert
        };

        let response = write_client.execute_transaction(request).await?;

        // Validate the ExecuteTransactionResponse
        assert!(response.digest.is_some(), "Response should have a digest");
        assert!(
            !response.digest.as_ref().unwrap().digest.is_empty(),
            "Digest should not be empty"
        );

        // Since we requested show_effects: true, validate effects are present
        assert!(
            response.effects.is_some(),
            "Effects should be present when show_effects is true"
        );

        // Validate that fields we didn't request are None/empty
        assert!(
            response.transaction.is_none(),
            "Transaction should be None when show_input is false"
        );
        assert!(
            response.raw_transaction.is_none(),
            "Raw transaction should be None when show_raw_input is false"
        );
        assert!(
            response.events.is_none(),
            "Events should be None when show_events is false"
        );
        assert!(
            response.object_changes.is_empty(),
            "Object changes should be empty when show_object_changes is false"
        );
        assert!(
            response.balance_changes.is_empty(),
            "Balance changes should be empty when show_balance_changes is false"
        );
        assert!(
            response.raw_effects.is_none(),
            "Raw effects should be None when show_raw_effects is false"
        );

        Ok::<(), anyhow::Error>(())
    })
    .await
    .expect("timeout waiting for transaction");

    match tx_result {
        Ok(()) => {}
        Err(e) => {
            panic!("WriteService transaction execution failed: {e}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_write_service_invalid_transaction() {
    let (_cluster, mut write_client) = setup_test_cluster_and_write_client().await;

    // Create invalid transaction data (dummy bytes that won't deserialize properly)
    let tx_bytes = vec![0u8; 32]; // Invalid transaction bytes
    let signatures = vec![vec![0u8; 64]]; // Invalid signature

    // Test execute_transaction with invalid data via WriteService
    let tx_result = tokio::time::timeout(Duration::from_secs(30), async {
        let request = grpc_write::ExecuteTransactionRequest {
            tx_bytes,
            signatures,
            options: Some(grpc_write::TransactionResponseOptions {
                show_input: false,
                show_raw_input: false,
                show_effects: true,
                show_events: false,
                show_object_changes: false,
                show_balance_changes: false,
                show_raw_effects: false,
            }),
            request_type: Some(1), // WaitForLocalExecution
        };

        let _response = write_client.execute_transaction(request).await?;
        // Should not reach here with invalid transaction data
        Ok::<(), anyhow::Error>(())
    })
    .await
    .expect("timeout waiting for transaction");

    if let Ok(()) = tx_result {
        // This would be unexpected for invalid transaction data
        panic!("WriteService should not succeed with invalid transaction data");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_transaction_data_bcs_deserialization() {
    let (cluster, mut write_client) = setup_test_cluster_and_write_client().await;

    let sender = cluster.get_address_0();
    let receiver = cluster.get_address_1();
    let amount = 1000u64;

    // Build and sign transaction
    let tx_data = cluster
        .test_transaction_builder_with_sender(sender)
        .await
        .transfer_iota(Some(amount), receiver)
        .build();

    let signed_tx = cluster.sign_transaction(&tx_data);
    let tx_bytes =
        bcs::to_bytes(&signed_tx.data().intent_message().value).expect("BCS serialization failed");
    let signatures: Vec<Vec<u8>> = signed_tx
        .tx_signatures()
        .iter()
        .map(|sig| sig.as_ref().to_vec())
        .collect();

    // Execute transaction
    let request = grpc_write::ExecuteTransactionRequest {
        tx_bytes,
        signatures,
        options: Some(grpc_write::TransactionResponseOptions {
            show_input: true,
            show_raw_input: true,
            show_effects: false,
            show_events: false,
            show_object_changes: false,
            show_balance_changes: false,
            show_raw_effects: false,
        }),
        request_type: None,
    };
    let response = write_client
        .execute_transaction(request)
        .await
        .expect("gRPC call should succeed");

    // Verify TransactionData can be deserialized from BCS
    assert!(
        response.transaction.is_some(),
        "TransactionData should be present when show_input is true"
    );

    let bcs_data = response.transaction.as_ref().unwrap();
    let deserialized_tx_data: TransactionData = bcs::from_bytes(&bcs_data.data)
        .expect("Should be able to deserialize TransactionData from BCS");

    // Verify the deserialized data matches original
    assert_eq!(deserialized_tx_data.sender(), sender, "Sender should match");
    assert_eq!(
        deserialized_tx_data.gas_budget(),
        tx_data.gas_budget(),
        "Gas budget should match"
    );

    // Verify raw_transaction can also be deserialized
    assert!(
        response.raw_transaction.is_some(),
        "Raw transaction should be present when show_raw_input is true"
    );
}
