// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::v1::transaction_execution_service::simulated_transaction::ExecutionResult;
use iota_macros::sim_test;
use iota_sdk_types::Transaction;
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::IotaAddress,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, Command, TransactionData, TransactionDataAPI},
};
use tonic::Code;

use super::{
    super::utils::{create_transaction_for_simulation, is_success, setup_grpc_test},
    common::assert_grpc_error,
};

#[sim_test]
async fn simulate_transaction_scenarios() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    // Test: regular and dev-inspect simulation modes
    for (skip_checks, mode_name) in [(false, "regular"), (true, "dev-inspect")] {
        let transaction = create_transaction_for_simulation(&test_cluster).await;

        let result = client
            .simulate_transaction(transaction, skip_checks, None)
            .await
            .unwrap_or_else(|e| panic!("Failed to simulate transaction in {mode_name} mode: {e}"));

        let effects = result
            .body()
            .executed_transaction()
            .expect("Failed to get executed_transaction from simulation result")
            .effects()
            .expect("Failed to get effects from simulation result")
            .effects()
            .expect("Failed to get inner effects from simulation result");
        assert!(
            is_success(effects.status()),
            "{mode_name} simulation should succeed"
        );

        let gas_summary = effects.gas_summary();
        assert!(
            gas_summary.computation_cost > 0 || gas_summary.storage_cost > 0,
            "{mode_name} simulation should report gas costs"
        );
    }

    // Test: minimal read mask
    let transaction = create_transaction_for_simulation(&test_cluster).await;
    let result = client
        .simulate_transaction(transaction, false, Some("executed_transaction.effects"))
        .await
        .expect("Failed to simulate transaction with minimal mask");

    let effects = result
        .body()
        .executed_transaction()
        .expect("Failed to get executed_transaction from simulation result")
        .effects()
        .expect("Failed to get effects from simulation result")
        .effects()
        .expect("Failed to get inner effects from simulation result");

    assert!(
        is_success(effects.status()),
        "Effects should be present with minimal mask"
    );

    // Test: insufficient gas budget returns gRPC error
    // Gas budget validation (min/max bounds) happens upfront in
    // check_gas_balance(), so a budget of 1 (below minimum) is rejected before
    // execution begins.
    let (sender, gas) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    let transaction = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, sender)
        .with_gas_budget(1)
        .build();
    let result = client.simulate_transaction(transaction, false, None).await;
    assert_grpc_error(result, Code::Internal);

    // Test: transfer exceeding balance returns Ok with failed effects
    // Transfer amount validation happens during Move VM execution, not upfront,
    // so the RPC succeeds but effects show failure (e.g., InsufficientCoinBalance).
    let (sender, gas) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    let fake_recipient = IotaAddress::random();
    let transaction = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(Some(1_000_000_000_000_000_000), fake_recipient)
        .build();
    let response = client
        .simulate_transaction(transaction, false, None)
        .await
        .expect("Simulation should succeed at RPC level");

    let effects = response
        .body()
        .executed_transaction()
        .expect("Failed to get executed_transaction from simulation result")
        .effects()
        .expect("Failed to get effects from simulation result")
        .effects()
        .expect("Failed to get inner effects from simulation result");

    assert!(
        !is_success(effects.status()),
        "Effects should show failure due to insufficient balance"
    );
}

/// Exercise the high-level client's SplitCoins + `command_results` path:
/// programmable-transaction construction through the SDK `Transaction` type,
/// read-mask forwarding, and `ExecutionResult::CommandResults` access on the
/// returned envelope. The exhaustive read-mask projection matrix for
/// `command_results` / `execution_error` is covered server-side by v1's
/// `simulate_transaction_readmask_scenarios`.
#[sim_test]
async fn simulate_transaction_command_results_split_coins() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.object_id);
    let gas_obj = gas.last().unwrap();
    let obj_to_split = gas.first().unwrap();

    let mut builder = ProgrammableTransactionBuilder::new();
    let coin_arg = builder
        .obj(CallArg::ImmutableOrOwned(*obj_to_split))
        .unwrap();
    let amount = builder.pure(1000u64).unwrap();
    let split_result = builder.command(Command::new_split_coins(coin_arg, vec![amount]));
    builder.transfer_arg(sender, split_result);
    let pt = builder.finish();

    let transaction: Transaction = TransactionData::new_programmable(
        sender,
        vec![*gas_obj],
        pt,
        10_000_000, // gas budget
        test_cluster.get_reference_gas_price().await,
    );

    let response = client
        .simulate_transaction(transaction, false, Some("execution_result.command_results"))
        .await
        .expect("simulate_transaction should succeed");

    let command_results = match response.body().execution_result.as_ref() {
        Some(ExecutionResult::CommandResults(cr)) => cr,
        other => panic!("expected CommandResults variant, got: {other:?}"),
    };

    let first = command_results
        .results
        .first()
        .expect("SplitCoins should produce at least one CommandResult");
    assert!(
        first.mutated_by_ref.is_some(),
        "SplitCoins should populate mutated_by_ref (input coin reference)"
    );
    assert!(
        first.return_values.is_some(),
        "SplitCoins should populate return_values (split-off coin)"
    );
}
