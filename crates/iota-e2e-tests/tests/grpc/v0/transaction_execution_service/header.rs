// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    headers,
    v0::{
        bcs::BcsData,
        signatures::{UserSignature, UserSignatures},
        transaction::Transaction as ProtoTransaction,
        transaction_execution_service::{
            ExecuteTransactionRequest, SimulateTransactionRequest,
            transaction_execution_service_client::TransactionExecutionServiceClient,
        },
    },
};
use iota_macros::sim_test;
use iota_test_transaction_builder::make_transfer_iota_transaction;
use iota_types::transaction::TransactionData;
use test_cluster::TestClusterBuilder;
use tonic::metadata::MetadataMap;

/// Helper function to verify that all expected IOTA headers are present in the
/// response metadata
fn verify_iota_headers(metadata: &MetadataMap, operation_name: &str) {
    let required_headers = [
        headers::X_IOTA_CHAIN,
        headers::X_IOTA_CHAIN_ID,
        headers::X_IOTA_CHECKPOINT_HEIGHT,
        headers::X_IOTA_EPOCH,
        headers::X_IOTA_TIMESTAMP_MS,
        headers::X_IOTA_TIMESTAMP,
        headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT,
        headers::X_IOTA_LOWEST_AVAILABLE_CHECKPOINT_OBJECTS,
    ];

    for header_name in &required_headers {
        assert!(
            metadata.get(*header_name).is_some(),
            "{} response should contain {} header",
            operation_name,
            header_name
        );
    }
}

/// Helper function to parse a u64 header value
fn parse_u64_header(metadata: &MetadataMap, header_name: &str) -> u64 {
    metadata
        .get(header_name)
        .unwrap_or_else(|| panic!("{} header should be present", header_name))
        .to_str()
        .unwrap_or_else(|_| panic!("{} header should be valid UTF-8", header_name))
        .parse()
        .unwrap_or_else(|_| panic!("{} header should be a valid u64", header_name))
}

#[sim_test]
async fn test_response_headers() {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .with_epoch_duration_ms(5000)
        .build()
        .await;

    let mut client = TransactionExecutionServiceClient::connect(test_cluster.grpc_url())
        .await
        .unwrap();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    // Test execute_transaction
    {
        test_cluster.wait_for_epoch(Some(2)).await;

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

        let response = client
            .execute_transaction(ExecuteTransactionRequest {
                transaction: Some(transaction),
                signatures: Some(signatures),
                read_mask: None,
            })
            .await
            .unwrap();

        let metadata = response.metadata();
        verify_iota_headers(metadata, "execute_transaction");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {}", epoch);
    }

    // Test simulate_transaction
    {
        test_cluster.wait_for_epoch(Some(3)).await;

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
        let response = client.simulate_transaction(request).await.unwrap();

        let metadata = response.metadata();
        verify_iota_headers(metadata, "simulate_transaction");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 2, "epoch should be at least 2, got {}", epoch);
    }
}
