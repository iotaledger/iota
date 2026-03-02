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
use iota_types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{ObjectArg, TransactionData, TransactionDataAPI},
};
use prost_types::FieldMask;

use crate::utils::{assert_field_presence, setup_grpc_test};

async fn assert_simulate_transaction_request(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    transaction: ProtoTransaction,
    read_mask: Option<FieldMask>,
    expected_fields: &[&str],
    scenario: &str,
) -> SimulateTransactionResponse {
    let response = exec_client
        .simulate_transaction({
            let mut req = SimulateTransactionRequest::default()
                .with_transaction(transaction)
                .with_tx_checks(vec![]);
            if let Some(mask) = read_mask {
                req = req.with_read_mask(mask);
            }
            req
        })
        .await
        .unwrap()
        .into_inner();

    assert_field_presence(&response, expected_fields, scenario);
    response
}

#[sim_test]
async fn simulate_transaction_with_gas_estimation() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

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
    let transaction = ProtoTransaction::default()
        .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap()));

    let request = SimulateTransactionRequest::default()
        .with_transaction(transaction)
        .with_tx_checks(vec![])
        .with_estimate_gas_budget(true);

    // Simulate the transaction
    let response = exec_client
        .simulate_transaction(request)
        .await
        .unwrap()
        .into_inner();

    // Verify gas budget estimation worked correctly
    let bcs_data = response
        .executed_transaction
        .unwrap()
        .transaction
        .unwrap()
        .bcs
        .unwrap();

    let returned_tx: TransactionData = bcs::from_bytes(&bcs_data.data).unwrap();
    // The estimated budget should be much less than 1 billion
    assert!(
        returned_tx.gas_data().budget < 1_000_000_000,
        "estimated budget should be less than original 1_000_000_000, got: {}",
        returned_tx.gas_data().budget
    );
    // The gas data should be positive
    assert!(
        returned_tx.gas_data().budget > 0,
        "estimated budget should be positive"
    );
}

#[sim_test]
async fn simulate_transaction_readmask_scenarios() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

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

    let create_transaction = || {
        ProtoTransaction::default()
            .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap()))
    };

    // Tests for readmask scenarios
    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "default readmask",
            None,
            // SIMULATE_TRANSACTION_READ_MASK = "executed_transaction.transaction,
            // executed_transaction.effects,executed_transaction.events,
            // executed_transaction.input_objects,executed_transaction.output_objects,
            // suggested_gas_price,execution_result"
            // Wildcard paths expand to all their sub-fields.
            vec![
                "executed_transaction.transaction.digest",
                "executed_transaction.transaction.bcs",
                "executed_transaction.effects.digest",
                "executed_transaction.effects.bcs",
                "executed_transaction.events.digest",
                "executed_transaction.events.events",
                "executed_transaction.input_objects",
                "executed_transaction.output_objects",
                "suggested_gas_price",
                "execution_result",
            ],
        ),
        (
            "empty readmask",
            Some(FieldMask::from_paths(&[] as &[&str])),
            vec![],
        ),
        // Full readmask: requesting parent "executed_transaction" returns ALL nested fields
        // All fields are present even if empty (simple transfers have no events but events field
        // is present)
        (
            "full readmask",
            Some(FieldMask::from_paths([
                "executed_transaction",
                "suggested_gas_price",
                "execution_result",
            ])),
            // "executed_transaction" is a wildcard → all sub-fields returned.
            // checkpoint and timestamp are absent: simulate/execute transactions
            // are not yet included in a checkpoint.
            vec![
                "executed_transaction.transaction.digest",
                "executed_transaction.transaction.bcs",
                "executed_transaction.signatures",
                "executed_transaction.effects.digest",
                "executed_transaction.effects.bcs",
                "executed_transaction.events.digest",
                "executed_transaction.events.events",
                "executed_transaction.input_objects",
                "executed_transaction.output_objects",
                "suggested_gas_price",
                "execution_result",
            ],
        ),
        (
            "partial readmask (executed_transaction only)",
            Some(FieldMask::from_paths(["executed_transaction"])),
            // checkpoint and timestamp absent: not yet in a checkpoint.
            vec![
                "executed_transaction.transaction.digest",
                "executed_transaction.transaction.bcs",
                "executed_transaction.signatures",
                "executed_transaction.effects.digest",
                "executed_transaction.effects.bcs",
                "executed_transaction.events.digest",
                "executed_transaction.events.events",
                "executed_transaction.input_objects",
                "executed_transaction.output_objects",
            ],
        ),
        (
            "partial readmask (execution_result only)",
            Some(FieldMask::from_paths(["execution_result"])),
            vec![
                "execution_result.command_results",
                "execution_result.execution_error",
            ],
        ),
        // Specific nested field masks - only the specified nested fields are returned
        (
            "nested readmask (executed_transaction.effects only)",
            Some(FieldMask::from_paths(["executed_transaction.effects"])),
            vec![
                "executed_transaction.effects.digest",
                "executed_transaction.effects.bcs",
            ],
        ),
        (
            "nested readmask (multiple specific fields)",
            Some(FieldMask::from_paths([
                "executed_transaction.effects",
                "execution_result",
            ])),
            vec![
                "executed_transaction.effects.digest",
                "executed_transaction.effects.bcs",
                "execution_result.command_results",
                "execution_result.execution_error",
            ],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        assert_simulate_transaction_request(
            &mut exec_client,
            create_transaction(),
            mask,
            &expected_paths,
            scenario,
        )
        .await;
    }
}

#[sim_test]
async fn simulate_transaction_invalid_bcs() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Create transaction with invalid BCS data
    let transaction = ProtoTransaction::default().with_bcs(
        BcsData::default().with_data(vec![0xff, 0xff, 0xff]), // Invalid BCS
    );

    // Request should fail with invalid BCS
    let result = exec_client
        .simulate_transaction(
            SimulateTransactionRequest::default()
                .with_transaction(transaction)
                .with_tx_checks(vec![]),
        )
        .await;

    assert!(
        result.is_err(),
        "Expected error for invalid BCS data, but got success"
    );
}

#[sim_test]
async fn simulate_transaction_empty_request() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Test empty/missing transaction
    let result = exec_client
        .simulate_transaction(SimulateTransactionRequest::default().with_tx_checks(vec![]))
        .await;

    assert!(
        result.is_err(),
        "Expected error for missing transaction, but got success"
    );
}

#[sim_test]
async fn simulate_transaction_command_results() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let gas_obj = gas.last().unwrap();
    let obj_to_split = gas.first().unwrap();

    // Build a programmable transaction that will produce command results
    // We need to use a Move call that returns values, not just transfer_arg
    let mut builder = ProgrammableTransactionBuilder::new();

    // Use SplitCoins which returns a value (the split coin)
    let gas_coin_arg = builder
        .obj(ObjectArg::ImmOrOwnedObject(*obj_to_split))
        .unwrap();
    let amount = builder.pure(1000u64).unwrap();

    // SplitCoins returns the newly created coin, which is an ExecutionResult
    let split_result = builder.command(iota_types::transaction::Command::SplitCoins(
        gas_coin_arg,
        vec![amount],
    ));

    // Transfer the split coin to sender (this uses the result from the previous
    // command)
    builder.transfer_arg(sender, split_result);

    let pt = builder.finish();

    let tx_data = TransactionData::new_programmable(
        sender,
        vec![*gas_obj],
        pt,
        10_000_000, // gas budget
        1000,       // gas price
    );

    let create_transaction = || {
        ProtoTransaction::default()
            .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap()))
    };

    // Test cases for command_results field presence
    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "default readmask",
            None,
            vec![
                "executed_transaction.transaction.digest",
                "executed_transaction.transaction.bcs",
                "executed_transaction.effects.digest",
                "executed_transaction.effects.bcs",
                "executed_transaction.events.digest",
                "executed_transaction.events.events",
                "executed_transaction.input_objects",
                "executed_transaction.output_objects",
                "suggested_gas_price",
                // mutated_by_ref has argument since they reference input arguments
                "execution_result.command_results.results.mutated_by_ref.outputs.argument.kind",
                "execution_result.command_results.results.mutated_by_ref.outputs.type_tag",
                "execution_result.command_results.results.mutated_by_ref.outputs.bcs",
                "execution_result.command_results.results.mutated_by_ref.outputs.json",
                // return_values don't have argument (they're results, not arguments)
                "execution_result.command_results.results.return_values.outputs.type_tag",
                "execution_result.command_results.results.return_values.outputs.bcs",
                "execution_result.command_results.results.return_values.outputs.json",
            ],
        ),
        (
            "full command_results readmask",
            Some(FieldMask::from_paths(["execution_result.command_results"])),
            vec![
                // Full mask returns all nested fields
                // mutated_by_ref has argument since they reference input arguments
                "execution_result.command_results.results.mutated_by_ref.outputs.argument.kind",
                "execution_result.command_results.results.mutated_by_ref.outputs.type_tag",
                "execution_result.command_results.results.mutated_by_ref.outputs.bcs",
                "execution_result.command_results.results.mutated_by_ref.outputs.json",
                // return_values don't have argument (they're results, not arguments)
                "execution_result.command_results.results.return_values.outputs.type_tag",
                "execution_result.command_results.results.return_values.outputs.bcs",
                "execution_result.command_results.results.return_values.outputs.json",
            ],
        ),
        (
            "command_results with nested return_values field",
            Some(FieldMask::from_paths([
                "execution_result.command_results.results.return_values",
            ])),
            vec![
                // return_values don't have argument (they're results, not arguments)
                "execution_result.command_results.results.return_values.outputs.type_tag",
                "execution_result.command_results.results.return_values.outputs.bcs",
                "execution_result.command_results.results.return_values.outputs.json",
            ],
        ),
        (
            "command_results with nested mutated_by_ref field",
            Some(FieldMask::from_paths([
                "execution_result.command_results.results.mutated_by_ref",
            ])),
            vec![
                // mutated_by_ref has argument since they reference input arguments
                "execution_result.command_results.results.mutated_by_ref.outputs.argument.kind",
                "execution_result.command_results.results.mutated_by_ref.outputs.type_tag",
                "execution_result.command_results.results.mutated_by_ref.outputs.bcs",
                "execution_result.command_results.results.mutated_by_ref.outputs.json",
            ],
        ),
        (
            "command_results return_values outputs with type_tag field",
            Some(FieldMask::from_paths([
                "execution_result.command_results.results.return_values.outputs.type_tag",
            ])),
            vec!["execution_result.command_results.results.return_values.outputs.type_tag"],
        ),
        (
            "command_results mutated_by_ref outputs with argument field",
            Some(FieldMask::from_paths([
                "execution_result.command_results.results.mutated_by_ref.outputs.argument",
            ])),
            vec!["execution_result.command_results.results.mutated_by_ref.outputs.argument.kind"],
        ),
        (
            "command_results mutated_by_ref outputs",
            Some(FieldMask::from_paths([
                "execution_result.command_results.results.mutated_by_ref.outputs",
            ])),
            vec![
                // mutated_by_ref has argument since they reference input arguments
                "execution_result.command_results.results.mutated_by_ref.outputs.argument.kind",
                "execution_result.command_results.results.mutated_by_ref.outputs.type_tag",
                "execution_result.command_results.results.mutated_by_ref.outputs.bcs",
                "execution_result.command_results.results.mutated_by_ref.outputs.json",
            ],
        ),
    ];

    for (scenario, mask, expected_paths) in test_cases {
        assert_simulate_transaction_request(
            &mut exec_client,
            create_transaction(),
            mask,
            &expected_paths,
            scenario,
        )
        .await;
    }
}
