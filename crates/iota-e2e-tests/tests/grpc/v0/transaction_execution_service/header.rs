// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    headers,
    v0::{
        bcs::BcsData,
        signatures::{UserSignature, UserSignatures},
        transaction::Transaction as ProtoTransaction,
        transaction_execution_service::{ExecuteTransactionRequest, SimulateTransactionRequest},
    },
};
use iota_macros::sim_test;
use iota_test_transaction_builder::make_transfer_iota_transaction;
use iota_types::transaction::TransactionData;

use crate::{
    utils::setup_grpc_test_with_builder,
    v0::header::{parse_u64_header, verify_iota_headers},
};

#[sim_test]
async fn test_response_headers() {
    let (test_cluster, client) =
        setup_grpc_test_with_builder(|b| b.with_epoch_duration_ms(5000), None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();
    let amount = 9;

    // Test execute_transaction
    {
        test_cluster.wait_for_epoch(Some(2)).await;

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

        let response = exec_client
            .execute_transaction(
                ExecuteTransactionRequest::default()
                    .with_transaction(transaction)
                    .with_signatures(signatures),
            )
            .await
            .unwrap();

        let metadata = response.metadata();
        verify_iota_headers(metadata, "execute_transaction");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 1, "epoch should be at least 1, got {epoch}");
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
        let transaction = ProtoTransaction::default()
            .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap()));

        let request = SimulateTransactionRequest::default()
            .with_transaction(transaction)
            .with_tx_checks(vec![])
            .with_estimate_gas_budget(true);

        // Simulate the transaction
        let response = exec_client.simulate_transaction(request).await.unwrap();

        let metadata = response.metadata();
        verify_iota_headers(metadata, "simulate_transaction");

        // Verify epoch value
        let epoch = parse_u64_header(metadata, headers::X_IOTA_EPOCH);
        assert!(epoch >= 2, "epoch should be at least 2, got {epoch}");
    }
}
