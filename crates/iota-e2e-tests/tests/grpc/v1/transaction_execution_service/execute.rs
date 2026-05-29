// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::FieldMaskUtil,
    read_masks::EXECUTE_TRANSACTIONS_READ_MASK,
    v1::{
        bcs::BcsData,
        signatures::{UserSignature, UserSignatures},
        transaction::{ExecutedTransaction, Transaction as ProtoTransaction},
        transaction_execution_service::{
            ExecuteTransactionItem, ExecuteTransactionsRequest, ExecuteTransactionsResponse,
            execute_transaction_result,
            transaction_execution_service_client::TransactionExecutionServiceClient,
        },
    },
};
use iota_macros::sim_test;
use iota_test_transaction_builder::make_transfer_iota_transaction;
use prost_types::FieldMask;

use super::build_item;
use crate::utils::{
    assert_field_presence, comma_separated_field_mask_to_paths, setup_grpc_test,
    setup_grpc_test_with_builder,
};

/// Extract the `ExecutedTransaction` from the first result in the response.
fn first_executed_transaction(response: &ExecuteTransactionsResponse) -> &ExecutedTransaction {
    let result = response
        .transaction_results
        .first()
        .expect("response should have at least one result");
    match &result.result {
        Some(execute_transaction_result::Result::ExecutedTransaction(tx)) => tx,
        Some(execute_transaction_result::Result::Error(e)) => {
            panic!("expected executed transaction, got error: {e:?}")
        }
        _ => panic!("expected executed transaction, got None"),
    }
}

async fn assert_execute_transaction_request(
    exec_client: &mut TransactionExecutionServiceClient<iota_grpc_client::InterceptedChannel>,
    item: ExecuteTransactionItem,
    read_mask: Option<FieldMask>,
    expected_fields: &[&str],
    scenario: &str,
) -> ExecuteTransactionsResponse {
    let response = exec_client
        .execute_transactions({
            let mut req = ExecuteTransactionsRequest::default().with_transactions(vec![item]);
            if let Some(mask) = read_mask {
                req = req.with_read_mask(mask);
            }
            req
        })
        .await
        .unwrap()
        .into_inner();

    let executed_tx = first_executed_transaction(&response);
    // Read mask paths apply directly to ExecutedTransaction fields
    // (e.g. "effects", not "executed_transaction.effects").
    assert_field_presence(executed_tx, expected_fields, &[], scenario);
    response
}

#[sim_test]
async fn execute_transaction_readmask_scenarios() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
    let amount = 9;

    // Read mask paths are relative to ExecutedTransaction
    // (e.g. "effects", not "executed_transaction.effects").
    type TestCase<'a> = (&'a str, Option<FieldMask>, Vec<&'a str>);
    let test_cases: Vec<TestCase> = vec![
        (
            "default readmask",
            None,
            // Bare paths with nested checkers (effects, events) auto-recurse into
            // all their sub-fields; "transaction.digest" is specific (bcs absent).
            comma_separated_field_mask_to_paths(EXECUTE_TRANSACTIONS_READ_MASK),
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

        let item = build_item(&txn);

        assert_execute_transaction_request(&mut exec_client, item, mask, &expected_paths, scenario)
            .await;
    }
}

#[sim_test]
async fn execute_transaction_invalid_bcs() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Create item with invalid BCS data
    let transaction = ProtoTransaction::default().with_bcs(
        BcsData::default().with_data(vec![0xff, 0xff, 0xff]), // Invalid BCS
    );
    let signatures = UserSignatures::default().with_signatures(vec![
        UserSignature::default().with_bcs(BcsData::default().with_data(vec![0x00; 64])),
    ]);
    let item = ExecuteTransactionItem::default()
        .with_transaction(transaction)
        .with_signatures(signatures);

    // With batch semantics, per-item errors are returned in the result
    let response = exec_client
        .execute_transactions(ExecuteTransactionsRequest::default().with_transactions(vec![item]))
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
async fn execute_transaction_invalid_signatures() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
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

    let item = ExecuteTransactionItem::default()
        .with_transaction(transaction)
        .with_signatures(signatures);

    // With batch semantics, per-item errors are returned in the result
    let response = exec_client
        .execute_transactions(ExecuteTransactionsRequest::default().with_transactions(vec![item]))
        .await
        .unwrap()
        .into_inner();

    let result = response.transaction_results.first().unwrap();
    let error = result
        .error()
        .expect("Expected per-item error for invalid signatures");
    assert_eq!(
        error.code,
        tonic::Code::InvalidArgument as i32,
        "Expected InvalidArgument error code for invalid signatures, got code {}",
        error.code
    );
}

#[sim_test]
async fn execute_transaction_missing_transaction_field() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Item with signatures but no transaction field should produce a per-item error
    let signatures = UserSignatures::default().with_signatures(vec![
        UserSignature::default().with_bcs(BcsData::default().with_data(vec![0x00; 64])),
    ]);
    let item = ExecuteTransactionItem::default().with_signatures(signatures);

    let response = exec_client
        .execute_transactions(ExecuteTransactionsRequest::default().with_transactions(vec![item]))
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
async fn execute_transaction_empty_request() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Empty transactions list should fail at the top level
    let result = exec_client
        .execute_transactions(ExecuteTransactionsRequest::default())
        .await;

    assert!(
        result.is_err(),
        "Expected error for empty transactions list, but got success"
    );
}

#[sim_test]
async fn execute_transaction_batch() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
    let amount = 9;

    // Create two valid transactions
    let txn1 =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;
    let txn2 =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    let items = vec![build_item(&txn1), build_item(&txn2)];

    let response = exec_client
        .execute_transactions(ExecuteTransactionsRequest::default().with_transactions(items))
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        response.transaction_results.len(),
        2,
        "Expected 2 results for batch of 2 transactions"
    );

    // Both should succeed and match the input ordering
    let expected_digests: Vec<_> = [&txn1, &txn2]
        .iter()
        .map(|t| t.digest().into_inner())
        .collect();
    for (i, result) in response.transaction_results.iter().enumerate() {
        let executed = result.executed_transaction().unwrap_or_else(|| {
            panic!(
                "Expected success for transaction {i}, got: {:?}",
                result.result
            )
        });
        let tx = executed
            .transaction()
            .expect("executed transaction should have a transaction");
        let digest = tx
            .digest
            .as_ref()
            .expect("transaction should have a digest");
        assert_eq!(
            digest.digest.as_ref(),
            expected_digests[i].as_slice(),
            "Transaction {i} digest mismatch — results may be out of order"
        );
    }
}

#[sim_test]
async fn execute_transaction_batch_partial_failure() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
    let amount = 9;

    // First item: valid transaction
    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;
    let valid_item = build_item(&txn);

    // Second item: invalid BCS
    let invalid_item = ExecuteTransactionItem::default()
        .with_transaction(
            ProtoTransaction::default()
                .with_bcs(BcsData::default().with_data(vec![0xff, 0xff, 0xff])),
        )
        .with_signatures(UserSignatures::default().with_signatures(vec![
            UserSignature::default().with_bcs(BcsData::default().with_data(vec![0x00; 64])),
        ]));

    let response = exec_client
        .execute_transactions(
            ExecuteTransactionsRequest::default().with_transactions(vec![valid_item, invalid_item]),
        )
        .await
        .unwrap()
        .into_inner();

    assert_eq!(response.transaction_results.len(), 2);

    // First should succeed
    assert!(
        response.transaction_results[0]
            .executed_transaction()
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
async fn execute_transaction_batch_size_exceeded() {
    let (_test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    // Send more items than the configured max batch size.
    // The batch size check runs before any per-item validation, so the items
    // don't need to be valid transactions.
    let max_batch =
        iota_config::node::GrpcApiConfig::default().max_execute_transaction_batch_size as usize;
    let items = vec![ExecuteTransactionItem::default(); max_batch + 1];

    let result = exec_client
        .execute_transactions(ExecuteTransactionsRequest::default().with_transactions(items))
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

#[sim_test]
async fn execute_transaction_with_checkpoint_inclusion() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
    let amount = 9;

    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;
    let item = build_item(&txn);

    // Execute with checkpoint inclusion timeout and request checkpoint + timestamp
    // in the read mask
    let response = exec_client
        .execute_transactions(
            ExecuteTransactionsRequest::default()
                .with_transactions(vec![item])
                .with_read_mask(FieldMask::from_paths([
                    "transaction.digest",
                    "effects",
                    "checkpoint",
                    "timestamp",
                ]))
                .with_checkpoint_inclusion_timeout_ms(30_000),
        )
        .await
        .unwrap()
        .into_inner();

    let executed_tx = first_executed_transaction(&response);

    // Verify checkpoint and timestamp are populated
    assert!(
        executed_tx.checkpoint.is_some(),
        "checkpoint should be populated when checkpoint_inclusion_timeout_ms is set"
    );
    assert!(
        executed_tx.timestamp.is_some(),
        "timestamp should be populated when checkpoint_inclusion_timeout_ms is set"
    );

    // Verify the checkpoint number is reasonable (> 0)
    let checkpoint = executed_tx.checkpoint.unwrap();
    assert!(checkpoint > 0, "checkpoint should be > 0, got {checkpoint}");

    // Verify the timestamp is reasonable (> 0)
    let timestamp = executed_tx.timestamp.as_ref().unwrap();
    assert!(
        timestamp.seconds > 0,
        "timestamp seconds should be > 0, got {}",
        timestamp.seconds
    );
}

#[sim_test]
async fn execute_transaction_without_checkpoint_timeout_has_no_checkpoint() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
    let amount = 9;

    let txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;
    let item = build_item(&txn);

    // Execute without checkpoint inclusion timeout but request checkpoint in mask
    let response = exec_client
        .execute_transactions(
            ExecuteTransactionsRequest::default()
                .with_transactions(vec![item])
                .with_read_mask(FieldMask::from_paths([
                    "transaction.digest",
                    "effects",
                    "checkpoint",
                    "timestamp",
                ])),
        )
        .await
        .unwrap()
        .into_inner();

    let executed_tx = first_executed_transaction(&response);

    // Without checkpoint_inclusion_timeout_ms, checkpoint and timestamp should be
    // absent
    assert!(
        executed_tx.checkpoint.is_none(),
        "checkpoint should be None without checkpoint_inclusion_timeout_ms"
    );
    assert!(
        executed_tx.timestamp.is_none(),
        "timestamp should be None without checkpoint_inclusion_timeout_ms"
    );
}

#[sim_test]
async fn execute_transaction_batch_with_checkpoint_inclusion() {
    let (test_cluster, client) = setup_grpc_test(None, None).await;

    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
    let amount = 9;

    // Create two valid transactions
    let txn1 =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;
    let txn2 =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    let items = vec![build_item(&txn1), build_item(&txn2)];

    let response = exec_client
        .execute_transactions(
            ExecuteTransactionsRequest::default()
                .with_transactions(items)
                .with_read_mask(FieldMask::from_paths([
                    "transaction.digest",
                    "effects",
                    "checkpoint",
                    "timestamp",
                ]))
                .with_checkpoint_inclusion_timeout_ms(30_000),
        )
        .await
        .unwrap()
        .into_inner();

    assert_eq!(
        response.transaction_results.len(),
        2,
        "Expected 2 results for batch of 2 transactions"
    );

    // Both transactions should have checkpoint and timestamp populated
    for (i, result) in response.transaction_results.iter().enumerate() {
        let executed = result.executed_transaction().unwrap_or_else(|| {
            panic!(
                "Expected success for transaction {i}, got: {:?}",
                result.result
            )
        });

        assert!(
            executed.checkpoint.is_some(),
            "transaction {i}: checkpoint should be populated"
        );
        assert!(
            executed.timestamp.is_some(),
            "transaction {i}: timestamp should be populated"
        );
    }
}

/// Drop-guard that restores the white-flag env vars on scope exit so the
/// process-wide flag does not leak into sibling tests sharing this process.
#[must_use = "bind the guard for the lifetime of the test"]
struct WhiteFlagEnvGuard;

impl Drop for WhiteFlagEnvGuard {
    fn drop(&mut self) {
        // SAFETY: paired with `enable_white_flag_env`; both run on the test
        // thread before/after the cluster is alive.
        unsafe {
            std::env::remove_var("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE");
            std::env::remove_var(
                "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW",
            );
        }
    }
}

fn enable_white_flag_env() -> WhiteFlagEnvGuard {
    // SAFETY: must be set before the test cluster starts spawning validator
    // tasks; thread-local protocol-config overrides do not propagate across
    // spawned tasks outside msim.
    unsafe {
        std::env::set_var("IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE", "1");
        std::env::set_var(
            "IOTA_PROTOCOL_CONFIG_FEATURE_FLAGS_OVERRIDE_ENABLE_WHITE_FLAG_FLOW",
            "true",
        );
    }
    WhiteFlagEnvGuard
}

/// Skip-effect-certification path through the gRPC handler. With the white-flag
/// flow enabled and `checkpoint_inclusion_timeout_ms` set, the handler must
/// (1) drive the TD without a 2f+1 broadcast, (2) wait for local checkpoint
/// inclusion, and (3) rebuild the response from the local cache so the client
/// never sees uncertified single-validator data.
///
/// Pinpoint signal that the rebuild ran: `checkpoint` is populated (only set
/// by the post-checkpoint reconcile branch) and the effects payload is
/// internally consistent. A safety-guard fire or a leak of UncertifiedSingle-
/// Validator finality would surface as a `tonic::Code::Internal` error from
/// the handler rather than a successful response.
#[sim_test]
async fn execute_transaction_v1_skip_cert_rebuilds_from_cache() {
    let _env_guard = enable_white_flag_env();
    let _proto_guard =
        iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

    let (test_cluster, client) = setup_grpc_test(None, None).await;
    let mut exec_client = client.execution_service_client();

    let recipient = iota_types::base_types::IotaAddress::random();
    let txn = make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(9)).await;
    let item = build_item(&txn);

    let response = exec_client
        .execute_transactions(
            ExecuteTransactionsRequest::default()
                .with_transactions(vec![item])
                .with_read_mask(FieldMask::from_paths([
                    "transaction.digest",
                    "effects",
                    "events",
                    "input_objects",
                    "output_objects",
                    "checkpoint",
                    "timestamp",
                ]))
                .with_checkpoint_inclusion_timeout_ms(30_000),
        )
        .await
        .unwrap()
        .into_inner();

    let executed = first_executed_transaction(&response);
    assert!(
        executed.checkpoint.is_some_and(|cp| cp > 0),
        "skip-cert reconcile must populate a non-zero checkpoint sequence"
    );
    assert!(
        executed.timestamp.as_ref().is_some_and(|ts| ts.seconds > 0),
        "skip-cert reconcile must populate a non-zero checkpoint timestamp"
    );
    assert!(
        executed.effects.is_some(),
        "effects must be present after the cache rebuild"
    );
    assert!(
        executed.transaction.is_some(),
        "transaction (digest) must be present"
    );
}

/// Skip-cert via gRPC with no consensus quorum: with two of four validators
/// stopped, the tx is submitted but never lands in a checkpoint within the
/// timeout. The gRPC handler must surface a per-item error rather than a
/// success with uncertified data. `finalize_item` maps this to
/// `tonic::Code::DeadlineExceeded` (or `Internal` if the wait call itself
/// failed); the contract checked here is "no successful response with
/// uncertified data and no whole-RPC failure either."
#[sim_test]
async fn execute_transaction_v1_skip_cert_no_quorum_yields_per_item_error() {
    let _env_guard = enable_white_flag_env();
    let _proto_guard =
        iota_protocol_config::ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_white_flag_flow_for_testing(true);
            config
        });

    let (test_cluster, client) =
        setup_grpc_test_with_builder(|b| b.with_num_validators(4), None, None).await;
    let mut exec_client = client.execution_service_client();

    // Baseline: prove the skip-cert path works before we break quorum so a
    // later failure is attributable to the deliberate validator loss.
    let recipient = iota_types::base_types::IotaAddress::random();
    let baseline_txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(9)).await;
    exec_client
        .execute_transactions(
            ExecuteTransactionsRequest::default()
                .with_transactions(vec![build_item(&baseline_txn)])
                .with_read_mask(FieldMask::from_paths(["transaction.digest", "checkpoint"]))
                .with_checkpoint_inclusion_timeout_ms(30_000),
        )
        .await
        .expect("baseline skip-cert tx should succeed with full quorum");

    let validators = test_cluster.get_validator_pubkeys();
    assert_eq!(validators.len(), 4);
    test_cluster.stop_node(&validators[0]);
    test_cluster.stop_node(&validators[1]);

    let stuck_txn =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(9)).await;
    let response = exec_client
        .execute_transactions(
            ExecuteTransactionsRequest::default()
                .with_transactions(vec![build_item(&stuck_txn)])
                .with_read_mask(FieldMask::from_paths(["transaction.digest", "checkpoint"]))
                .with_checkpoint_inclusion_timeout_ms(5_000),
        )
        .await
        .expect("RPC itself should not fail; per-item error surfaces inside the response")
        .into_inner();

    let result = response
        .transaction_results
        .first()
        .expect("response should carry one per-item result");
    match &result.result {
        Some(execute_transaction_result::Result::Error(e)) => {
            let code = tonic::Code::from_i32(e.code);
            assert!(
                matches!(code, tonic::Code::DeadlineExceeded | tonic::Code::Internal),
                "skip-cert under quorum loss should yield DeadlineExceeded or Internal, got {code:?}: {e:?}"
            );
        }
        Some(execute_transaction_result::Result::ExecutedTransaction(tx)) => panic!(
            "skip-cert under quorum loss must not return a successful executed transaction; got {tx:?}"
        ),
        None => panic!("per-item result must be populated"),
        other => panic!("unexpected per-item result variant: {other:?}"),
    }
}
