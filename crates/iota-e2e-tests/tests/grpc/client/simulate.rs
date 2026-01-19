// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::base_types::IotaAddress;

use super::common::{
    create_transaction_for_simulation, is_success, setup_grpc_test, to_sdk_transaction,
};

#[sim_test]
async fn simulate_transaction_modes() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    // Test both regular simulation and dev-inspect mode
    for (dev_inspect, mode_name) in [(false, "regular"), (true, "dev-inspect")] {
        let transaction = create_transaction_for_simulation(&test_cluster).await;

        let result = client
            .simulate_transaction(transaction, dev_inspect, None)
            .await
            .unwrap_or_else(|e| panic!("Failed to simulate transaction in {mode_name} mode: {e}"));

        assert!(
            is_success(result.effects.status()),
            "{mode_name} simulation should succeed"
        );

        // Verify gas costs are reported
        let gas_summary = result.effects.gas_summary();
        assert!(
            gas_summary.computation_cost > 0 || gas_summary.storage_cost > 0,
            "{mode_name} simulation should report gas costs"
        );
    }
}

#[sim_test]
async fn simulate_transaction_minimal_mask() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let transaction = create_transaction_for_simulation(&test_cluster).await;

    let result = client
        .simulate_transaction(transaction, false, Some("transaction.effects"))
        .await
        .expect("Failed to simulate transaction with minimal mask");

    assert!(
        is_success(result.effects.status()),
        "Effects should be present with minimal mask"
    );
}

#[sim_test]
async fn simulate_transaction_idempotency() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let transaction = create_transaction_for_simulation(&test_cluster).await;

    let result1 = client
        .simulate_transaction(transaction.clone(), false, None)
        .await
        .expect("First simulation should succeed");

    let result2 = client
        .simulate_transaction(transaction, false, None)
        .await
        .expect("Second simulation should also succeed");

    assert!(
        is_success(result1.effects.status()),
        "First simulation should succeed"
    );
    assert!(
        is_success(result2.effects.status()),
        "Second simulation should succeed"
    );
}

#[sim_test]
async fn simulate_transaction_insufficient_gas() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let (sender, gas) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();

    let rgp = test_cluster.get_reference_gas_price().await;

    let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, sender)
        .with_gas_budget(1)
        .build();

    let transaction = to_sdk_transaction(&tx_data);
    let result = client.simulate_transaction(transaction, false, None).await;

    // With insufficient gas budget, simulation may either:
    // - Return an error from the server
    // - Succeed but show failure in effects (InsufficientGas)
    match result {
        Ok(response) => {
            assert!(
                !is_success(response.effects.status()),
                "Simulation with insufficient gas should fail in effects"
            );
        }
        Err(_) => {
            // Server rejected the transaction due to invalid gas budget
        }
    }
}

#[sim_test]
async fn simulate_transaction_invalid() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let (sender, gas) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();

    let rgp = test_cluster.get_reference_gas_price().await;

    let fake_recipient = IotaAddress::random_for_testing_only();
    let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(Some(1_000_000_000_000), fake_recipient)
        .build();

    let transaction = to_sdk_transaction(&tx_data);
    let result = client.simulate_transaction(transaction, false, None).await;

    // Transferring more than available balance should either:
    // - Return an error from the server
    // - Succeed but show failure in effects (InsufficientCoinBalance)
    match result {
        Ok(response) => {
            assert!(
                !is_success(response.effects.status()),
                "Simulation with insufficient balance should fail in effects"
            );
        }
        Err(_) => {
            // Server rejected the transaction due to insufficient balance
        }
    }
}
