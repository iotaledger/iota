// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::SIMULATE_TRANSACTIONS_READ_MASK,
    v1::{
        bcs::BcsData,
        transaction::Transaction as ProtoTransaction,
        transaction_execution_service::{
            SimulateTransactionItem, SimulateTransactionsRequest, SimulateTransactionsResponse,
            SimulatedTransaction, simulate_transaction_item::TransactionCheckModes,
            simulated_transaction::ExecutionResult,
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

use crate::utils::{assert_field_presence, comma_separated_field_mask_to_paths, setup_grpc_test};

/// Helper to build a `SimulateTransactionItem` from a proto transaction.
fn build_simulate_item(transaction: ProtoTransaction) -> SimulateTransactionItem {
    SimulateTransactionItem::default()
        .with_transaction(transaction)
        .with_tx_checks(vec![])
}

/// Extract the `SimulatedTransaction` from the first result in the response.
fn first_simulated_transaction(response: &SimulateTransactionsResponse) -> &SimulatedTransaction {
    let result = response
        .transaction_results
        .first()
        .expect("response should have at least one result");
    result
        .simulated_transaction()
        .expect("expected simulated transaction, not error")
}

/// Simulate a transaction and assert field presence at three levels:
///
/// 1. **`expected_response_paths` / `ignored_response_paths`** — asserted on
///    the [`SimulatedTransaction`].
/// 2. **`expected_command_results_paths`** — when non-empty, `execution_result`
///    is extracted as [`ExecutionResult::CommandResults`] and
///    [`assert_field_presence`] is called on it.  Paths are relative to
///    `CommandResults` (i.e. strip the leading
///    `"execution_result.command_results."` prefix).  Panics if
///    `execution_result` is not the `CommandResults` variant.
/// 3. **`expected_execution_error_paths`** — when non-empty, `execution_result`
///    is extracted as [`ExecutionResult::ExecutionError`] and
///    [`assert_field_presence`] is called on it.  Paths are relative to
///    `ExecutionError` (i.e. strip the leading
///    `"execution_result.execution_error."` prefix).  Panics if
///    `execution_result` is not the `ExecutionError` variant.
async fn assert_simulate_transaction_request(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    transaction: ProtoTransaction,
    read_mask: Option<FieldMask>,
    expected_response_paths: &[&str],
    ignored_response_paths: &[&str],
    expected_command_results_paths: &[&str],
    expected_execution_error_paths: &[&str],
    scenario: &str,
) -> SimulatedTransaction {
    let item = build_simulate_item(transaction);
    let mut req = SimulateTransactionsRequest::default().with_transactions(vec![item]);
    if let Some(mask) = read_mask {
        req = req.with_read_mask(mask);
    }

    let response = exec_client
        .simulate_transactions(req)
        .await
        .unwrap()
        .into_inner();

    let simulated = first_simulated_transaction(&response);

    assert_field_presence(
        simulated,
        expected_response_paths,
        ignored_response_paths,
        scenario,
    );

    if !expected_command_results_paths.is_empty() {
        let command_results = match simulated.execution_result {
            Some(ExecutionResult::CommandResults(ref cr)) => cr,
            Some(ExecutionResult::ExecutionError(_)) => {
                panic!("{scenario}: expected CommandResults but got ExecutionError")
            }
            Some(_) => panic!("{scenario}: expected CommandResults but got unknown variant"),
            None => panic!("{scenario}: execution_result is None"),
        };
        assert_field_presence(
            command_results,
            expected_command_results_paths,
            &[],
            &format!("{scenario} (command_results)"),
        );
    }
    if !expected_execution_error_paths.is_empty() {
        let execution_error = match simulated.execution_result {
            Some(ExecutionResult::ExecutionError(ref ee)) => ee,
            Some(ExecutionResult::CommandResults(_)) => {
                panic!("{scenario}: expected ExecutionError but got CommandResults")
            }
            Some(_) => panic!("{scenario}: expected ExecutionError but got unknown variant"),
            None => panic!("{scenario}: execution_result is None"),
        };
        assert_field_presence(
            execution_error,
            expected_execution_error_paths,
            &[],
            &format!("{scenario} (execution_error)"),
        );
    }

    simulated.clone()
}

#[sim_test]
async fn simulate_transaction_zero_gas_budget_uses_max() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let obj_to_send = gas.first().unwrap();
    let gas_obj = gas.last().unwrap();

    // Build a transfer transaction with gas budget = 0
    let tx_data = TransactionData::new_transfer(
        recipient,
        *obj_to_send,
        sender,
        *gas_obj,
        0,    // zero gas budget — server should replace with max_tx_gas
        1000, // gas price
    );

    let transaction = ProtoTransaction::default()
        .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap()));

    let item = SimulateTransactionItem::default()
        .with_transaction(transaction)
        .with_tx_checks(vec![TransactionCheckModes::DisableVmChecks as i32]);
    let request = SimulateTransactionsRequest::default().with_transactions(vec![item]);

    // Simulate the transaction
    let response = exec_client
        .simulate_transactions(request)
        .await
        .unwrap()
        .into_inner();

    let simulated = first_simulated_transaction(&response);

    // Verify that the returned transaction has a non-zero gas budget (replaced with
    // max_tx_gas)
    let bcs_data = simulated
        .executed_transaction
        .as_ref()
        .unwrap()
        .transaction
        .as_ref()
        .unwrap()
        .bcs
        .as_ref()
        .unwrap();

    let returned_tx: TransactionData = bcs::from_bytes(&bcs_data.data).unwrap();
    assert!(
        returned_tx.gas_data().budget > 0,
        "gas budget should have been replaced with max_tx_gas, but was 0"
    );
}

#[sim_test]
async fn simulate_transaction_below_min_gas_budget_returns_error() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random_for_testing_only();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let obj_to_send = gas.first().unwrap();
    let gas_obj = gas.last().unwrap();

    // Build a transfer transaction with a gas budget below the minimum
    // (min = base_tx_cost_fixed * gas_price = 1000 * 1000 = 1_000_000 NANOS)
    let tx_data = TransactionData::new_transfer(
        recipient,
        *obj_to_send,
        sender,
        *gas_obj,
        1,    // way below minimum gas budget
        1000, // gas price
    );

    let transaction = ProtoTransaction::default()
        .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap()));

    let item = build_simulate_item(transaction);
    let request = SimulateTransactionsRequest::default().with_transactions(vec![item]);

    let response = exec_client
        .simulate_transactions(request)
        .await
        .unwrap()
        .into_inner();

    // With upfront gas validation removed, the simulation engine itself
    // rejects the insufficient budget, producing an Internal error.
    let result = response.transaction_results.first().unwrap();
    let error = result
        .error()
        .expect("Expected per-item error for below-minimum gas budget");
    assert_eq!(
        error.code,
        tonic::Code::Internal as i32,
        "Expected Internal error code, got code {}",
        error.code
    );
}

#[sim_test]
async fn simulate_transaction_readmask_scenarios() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let gas_obj = gas.last().unwrap();
    let obj_to_split = gas.first().unwrap();

    // Build a SplitCoins programmable transaction so that execution_result
    // contains real command results (mutated_by_ref + return_values) that the
    // command-results readmask scenarios below can verify deeply.
    let mut builder = ProgrammableTransactionBuilder::new();
    let gas_coin_arg = builder
        .obj(ObjectArg::ImmOrOwnedObject(*obj_to_split))
        .unwrap();
    let amount = builder.pure(1000u64).unwrap();
    let split_result = builder.command(iota_types::transaction::Command::SplitCoins(
        gas_coin_arg,
        vec![amount],
    ));
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
        Some(
            ProtoTransaction::default()
                .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap())),
        )
    };

    // Build a failing transaction: try to split u64::MAX coins from obj_to_split.
    // The coin's balance is far below u64::MAX so the SplitCoins command aborts,
    // producing ExecutionResult::ExecutionError (bcs_kind + source +
    // command_index=0).
    let mut failing_builder = ProgrammableTransactionBuilder::new();
    let failing_coin_arg = failing_builder
        .obj(ObjectArg::ImmOrOwnedObject(*obj_to_split))
        .unwrap();
    let huge_amount = failing_builder.pure(u64::MAX).unwrap();
    failing_builder.command(iota_types::transaction::Command::SplitCoins(
        failing_coin_arg,
        vec![huge_amount],
    ));
    let failing_pt = failing_builder.finish();
    let failing_tx_data = TransactionData::new_programmable(
        sender,
        vec![*gas_obj],
        failing_pt,
        10_000_000, // gas budget
        1000,       // gas price
    );
    let create_failing_transaction = || {
        Some(
            ProtoTransaction::default()
                .with_bcs(BcsData::default().with_data(bcs::to_bytes(&failing_tx_data).unwrap())),
        )
    };

    // SplitCoins semantics for the first command result:
    //   mutated_by_ref — the input coin (modified in-place via mutable reference)
    //                    has `argument.kind` set because it references an input arg
    //   return_values  — the newly split coin (a fresh result, not an input arg)
    //                    has NO `argument` field
    #[derive(Default)]
    struct SimulateTestCase<'a> {
        scenario: &'a str,
        mask: Option<FieldMask>,
        /// The transaction to submit.
        transaction: Option<ProtoTransaction>,
        /// Paths asserted on the [`SimulatedTransaction`].
        expected_response: Vec<&'a str>,
        /// Paths on [`SimulatedTransaction`] that are skipped (e.g.
        /// fields that are legitimately absent in the test environment).
        ignored_response: Vec<&'a str>,
        /// When non-empty, `execution_result` is extracted as
        /// [`ExecutionResult::CommandResults`] and paths are asserted on it
        /// (relative to `CommandResults`).
        expected_command_results: Vec<&'a str>,
        /// When non-empty, `execution_result` is extracted as
        /// [`ExecutionResult::ExecutionError`] and paths are asserted on it
        /// (relative to `ExecutionError`).
        expected_execution_error: Vec<&'a str>,
    }

    let test_cases: Vec<SimulateTestCase> = vec![
        SimulateTestCase {
            scenario: "default readmask",
            transaction: create_transaction(),
            mask: None,
            expected_response: comma_separated_field_mask_to_paths(SIMULATE_TRANSACTIONS_READ_MASK),
            expected_command_results: vec![
                "mutated_by_ref.argument.kind",
                "mutated_by_ref.type_tag",
                "mutated_by_ref.bcs",
                "mutated_by_ref.json",
                "return_values.type_tag",
                "return_values.bcs",
                "return_values.json",
            ],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "empty readmask",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths(&[] as &[&str])),
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "full readmask",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths([
                "executed_transaction",
                "suggested_gas_price",
                "execution_result",
            ])),
            expected_response: vec![
                "executed_transaction",
                "suggested_gas_price",
                "execution_result",
            ],
            // checkpoint/timestamp are None for not-yet-checkpointed transactions
            ignored_response: vec![
                "executed_transaction.checkpoint",
                "executed_transaction.timestamp",
            ],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "partial readmask (executed_transaction only)",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths(["executed_transaction"])),
            expected_response: vec!["executed_transaction"],
            // checkpoint/timestamp are None for not-yet-checkpointed transactions
            ignored_response: vec![
                "executed_transaction.checkpoint",
                "executed_transaction.timestamp",
            ],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "partial readmask (execution_result only)",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths(["execution_result"])),
            expected_response: vec!["execution_result"],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "nested readmask (executed_transaction.effects only)",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths(["executed_transaction.effects"])),
            expected_response: vec!["executed_transaction.effects"],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "nested readmask (executed_transaction.effects + execution_result)",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths([
                "executed_transaction.effects",
                "execution_result",
            ])),
            expected_response: vec!["executed_transaction.effects", "execution_result"],
            ..Default::default()
        },
        // ====================================================================
        // command_results-focused cases: response check just verifies
        // execution_result is present (and executed_transaction / suggested_gas_price
        // are absent), then the deep CommandResults structure is verified.
        // ====================================================================
        SimulateTestCase {
            scenario: "command_results: full",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths(["execution_result.command_results"])),
            expected_response: vec!["execution_result"],
            expected_command_results: vec![
                "mutated_by_ref.argument.kind",
                "mutated_by_ref.type_tag",
                "mutated_by_ref.bcs",
                "mutated_by_ref.json",
                "return_values.type_tag",
                "return_values.bcs",
                "return_values.json",
            ],
            ..Default::default()
        },
        SimulateTestCase {
            // Only return_values requested — mutated_by_ref must be absent.
            scenario: "command_results: return_values only",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths([
                "execution_result.command_results.return_values",
            ])),
            expected_response: vec!["execution_result"],
            expected_command_results: vec![
                "return_values.type_tag",
                "return_values.bcs",
                "return_values.json",
            ],
            ..Default::default()
        },
        SimulateTestCase {
            // Only mutated_by_ref requested — return_values must be absent.
            scenario: "command_results: mutated_by_ref only",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths([
                "execution_result.command_results.mutated_by_ref",
            ])),
            expected_response: vec!["execution_result"],
            expected_command_results: vec![
                "mutated_by_ref.argument.kind",
                "mutated_by_ref.type_tag",
                "mutated_by_ref.bcs",
                "mutated_by_ref.json",
            ],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "command_results: return_values.type_tag only",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths([
                "execution_result.command_results.return_values.type_tag",
            ])),
            expected_response: vec!["execution_result"],
            expected_command_results: vec!["return_values.type_tag"],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "command_results: mutated_by_ref.argument only",
            transaction: create_transaction(),
            mask: Some(FieldMask::from_paths([
                "execution_result.command_results.mutated_by_ref.argument",
            ])),
            expected_response: vec!["execution_result"],
            expected_command_results: vec!["mutated_by_ref.argument.kind"],
            ..Default::default()
        },
        // ====================================================================
        // execution_error-focused cases: the failing transaction (SplitCoins
        // with u64::MAX amount) aborts at command index 0, producing an
        // ExecutionResult::ExecutionError with bcs_kind + source +
        // command_index.
        // ====================================================================
        SimulateTestCase {
            scenario: "execution_error: default readmask",
            transaction: create_failing_transaction(),
            mask: None,
            expected_response: comma_separated_field_mask_to_paths(SIMULATE_TRANSACTIONS_READ_MASK),
            expected_execution_error: vec!["bcs_kind", "source", "command_index"],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "execution_error: full fields",
            transaction: create_failing_transaction(),
            mask: Some(FieldMask::from_paths(["execution_result.execution_error"])),
            expected_response: vec!["execution_result"],
            expected_execution_error: vec!["bcs_kind", "source", "command_index"],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "execution_error: bcs_kind only",
            transaction: create_failing_transaction(),
            mask: Some(FieldMask::from_paths([
                "execution_result.execution_error.bcs_kind",
            ])),
            expected_response: vec!["execution_result"],
            expected_execution_error: vec!["bcs_kind"],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "execution_error: source only",
            transaction: create_failing_transaction(),
            mask: Some(FieldMask::from_paths([
                "execution_result.execution_error.source",
            ])),
            expected_response: vec!["execution_result"],
            expected_execution_error: vec!["source"],
            ..Default::default()
        },
        SimulateTestCase {
            scenario: "execution_error: command_index only",
            transaction: create_failing_transaction(),
            mask: Some(FieldMask::from_paths([
                "execution_result.execution_error.command_index",
            ])),
            expected_response: vec!["execution_result"],
            expected_execution_error: vec!["command_index"],
            ..Default::default()
        },
    ];

    for tc in test_cases {
        assert_simulate_transaction_request(
            &mut exec_client,
            tc.transaction.expect("test case transaction must be Some"),
            tc.mask,
            &tc.expected_response,
            &tc.ignored_response,
            &tc.expected_command_results,
            &tc.expected_execution_error,
            tc.scenario,
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

    // With batch semantics, per-item errors are returned in the result
    let item = build_simulate_item(transaction);
    let response = exec_client
        .simulate_transactions(SimulateTransactionsRequest::default().with_transactions(vec![item]))
        .await
        .unwrap()
        .into_inner();

    let result = response.transaction_results.first().unwrap();
    let error = result
        .error()
        .expect("Expected per-item error for invalid BCS data");
    assert_eq!(
        error.code,
        tonic::Code::InvalidArgument as i32,
        "Expected InvalidArgument error code for invalid BCS, got code {}",
        error.code
    );
}

#[sim_test]
async fn simulate_transaction_missing_transaction_field() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Item with no transaction field should produce a per-item error
    let item = SimulateTransactionItem::default().with_tx_checks(vec![]);

    let response = exec_client
        .simulate_transactions(SimulateTransactionsRequest::default().with_transactions(vec![item]))
        .await
        .unwrap()
        .into_inner();

    let result = response.transaction_results.first().unwrap();
    let error = result
        .error()
        .expect("Expected per-item error for missing transaction field");
    assert_eq!(
        error.code,
        tonic::Code::InvalidArgument as i32,
        "Expected InvalidArgument error code for missing transaction, got code {}",
        error.code
    );
}

#[sim_test]
async fn simulate_transaction_empty_request() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Empty transactions list should fail at the top level
    let result = exec_client
        .simulate_transactions(SimulateTransactionsRequest::default())
        .await;

    assert!(
        result.is_err(),
        "Expected error for empty transactions list, but got success"
    );
}

#[sim_test]
async fn simulate_transaction_batch() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let gas_obj = gas.last().unwrap();
    let obj = gas.first().unwrap();

    // Build two distinct simulation transactions
    let tx_data1 = TransactionData::new_transfer(
        iota_types::base_types::IotaAddress::random_for_testing_only(),
        *obj,
        sender,
        *gas_obj,
        10_000_000,
        1000,
    );
    let tx_data2 = TransactionData::new_transfer(
        iota_types::base_types::IotaAddress::random_for_testing_only(),
        *obj,
        sender,
        *gas_obj,
        10_000_000,
        1000,
    );

    let items = vec![
        build_simulate_item(
            ProtoTransaction::default()
                .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data1).unwrap())),
        ),
        build_simulate_item(
            ProtoTransaction::default()
                .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data2).unwrap())),
        ),
    ];

    let response = exec_client
        .simulate_transactions(SimulateTransactionsRequest::default().with_transactions(items))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        response.transaction_results.len(),
        2,
        "Expected 2 results for batch of 2 transactions"
    );

    // Both should succeed
    for (i, result) in response.transaction_results.iter().enumerate() {
        assert!(
            result.simulated_transaction().is_some(),
            "Expected success for transaction {i}, got: {:?}",
            result.result
        );
    }
}

#[sim_test]
async fn simulate_transaction_batch_partial_failure() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut exec_client = client.execution_service_client();

    let (sender, mut gas) = test_cluster.wallet.get_one_account().await.unwrap();
    gas.sort_by_key(|object_ref| object_ref.0);
    let gas_obj = gas.last().unwrap();
    let obj = gas.first().unwrap();

    // First item: valid transaction
    let tx_data = TransactionData::new_transfer(
        iota_types::base_types::IotaAddress::random_for_testing_only(),
        *obj,
        sender,
        *gas_obj,
        10_000_000,
        1000,
    );
    let valid_item = build_simulate_item(
        ProtoTransaction::default()
            .with_bcs(BcsData::default().with_data(bcs::to_bytes(&tx_data).unwrap())),
    );

    // Second item: invalid BCS
    let invalid_item = build_simulate_item(
        ProtoTransaction::default().with_bcs(BcsData::default().with_data(vec![0xff, 0xff, 0xff])),
    );

    let response = exec_client
        .simulate_transactions(
            SimulateTransactionsRequest::default()
                .with_transactions(vec![valid_item, invalid_item]),
        )
        .await
        .unwrap()
        .into_inner();

    assert_eq!(response.transaction_results.len(), 2);

    // First should succeed
    assert!(
        response.transaction_results[0]
            .simulated_transaction()
            .is_some(),
        "Expected success for first transaction, got: {:?}",
        response.transaction_results[0].result
    );

    // Second should fail with InvalidArgument
    let error = response.transaction_results[1]
        .error()
        .expect("Expected error for second transaction with invalid BCS");
    assert_eq!(
        error.code,
        tonic::Code::InvalidArgument as i32,
        "Expected InvalidArgument for invalid BCS, got code {}",
        error.code
    );
}

#[sim_test]
async fn simulate_transaction_batch_size_exceeded() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Send more items than the configured max batch size.
    // The batch size check runs before any per-item validation, so the items
    // don't need to be valid transactions.
    let max_batch =
        iota_config::node::GrpcApiConfig::default().max_simulate_transaction_batch_size as usize;
    let items = vec![SimulateTransactionItem::default(); max_batch + 1];

    let result = exec_client
        .simulate_transactions(SimulateTransactionsRequest::default().with_transactions(items))
        .await;

    assert!(
        result.is_err(),
        "Expected top-level error for oversized batch"
    );
    let status = result.unwrap_err();
    assert_eq!(
        status.code(),
        tonic::Code::InvalidArgument,
        "Expected InvalidArgument, got {:?}",
        status.code()
    );
}
