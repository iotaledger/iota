// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::Error;
use iota_macros::sim_test;
use iota_sdk_types::UserSignature;
use iota_test_transaction_builder::make_transfer_iota_transaction;
use iota_types::base_types::IotaAddress;

use super::{
    super::utils::setup_grpc_test,
    common::{create_signed_transaction, is_success},
};

#[sim_test]
async fn execute_transaction_transfer() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx, None, None)
        .await
        .expect("Failed to execute transaction");

    let effects = result
        .body()
        .effects()
        .expect("Failed to get effects from execution result")
        .effects()
        .expect("Failed to get inner effects from execution result");

    assert!(
        is_success(effects.status()),
        "Transaction should have succeeded"
    );

    // Verify gas was charged
    let gas_summary = effects.gas_summary();
    assert!(
        gas_summary.computation_cost > 0 || gas_summary.storage_cost > 0,
        "Some gas should have been charged"
    );

    // Verify response fields are present with default mask
    assert!(
        result.body().input_objects.is_some(),
        "Input objects should be present with default mask"
    );
    assert!(
        result.body().output_objects.is_some(),
        "Output objects should be present with default mask"
    );
}

/// Verify that a transfer creates the expected output objects: the mutated gas
/// coin for the sender and a new coin for the recipient.
#[sim_test]
async fn execute_transaction_transfer_outputs() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let recipient = IotaAddress::random_for_testing_only();
    let amount = 9;

    let tx =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;
    let signed_tx: iota_sdk_types::SignedTransaction =
        tx.try_into().expect("SDK type conversion failed");

    let result = client
        .execute_transaction(signed_tx, None, None)
        .await
        .expect("Failed to execute transaction");

    let effects = result
        .body()
        .effects()
        .expect("Failed to get effects")
        .effects()
        .expect("Failed to get inner effects");

    assert!(is_success(effects.status()), "Transaction should succeed");

    // A SplitCoins + TransferObjects transfer produces at least 2 output objects:
    // the mutated gas coin (sender) and the new coin (recipient).
    let output_objects = result
        .body()
        .output_objects
        .as_ref()
        .expect("output objects");
    assert!(
        output_objects.objects.len() >= 2,
        "Expected at least 2 output objects (gas + recipient coin), got {}",
        output_objects.objects.len()
    );
}

#[sim_test]
async fn execute_transaction_minimal_mask() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx, Some("effects"), None)
        .await
        .expect("Failed to execute transaction");

    assert!(
        is_success(
            result
                .body()
                .effects()
                .expect("Failed to get SDK effects from execution result with minimal mask")
                .effects()
                .expect("Failed to get inner effects from execution result with minimal mask")
                .status()
        ),
        "Effects should show successful execution"
    );
    assert!(
        result.body().input_objects.is_none(),
        "Input objects should not be present with minimal mask"
    );
    assert!(
        result.body().output_objects.is_none(),
        "Output objects should not be present with minimal mask"
    );
}

#[sim_test]
async fn execute_transaction_invalid_signature() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let mut signed_tx = create_signed_transaction(&test_cluster).await;

    // Corrupt the signature by modifying its bytes to create a definitively invalid
    // signature. We serialize the original signature, corrupt it, then
    // deserialize back.
    assert!(
        !signed_tx.signatures.is_empty(),
        "Transaction should have at least one signature"
    );
    let mut sig_bytes = bcs::to_bytes(&signed_tx.signatures[0]).expect("BCS serialization failed");
    // Only flip bytes near the end of the signature data where the actual
    // cryptographic signature bytes are. We must preserve the BCS length
    // prefixes at the beginning to allow deserialization.
    let corrupt_count = sig_bytes.len().min(32);
    for byte in sig_bytes.iter_mut().rev().take(corrupt_count) {
        *byte = !*byte;
    }
    let corrupted_sig: UserSignature =
        bcs::from_bytes(&sig_bytes).expect("Corrupted signature should still deserialize");
    signed_tx.signatures = vec![corrupted_sig];

    let result = client.execute_transaction(signed_tx, None, None).await;

    // With batch semantics, per-item validation errors come back as Error::Server
    let err = result.expect_err("Expected error for invalid signature");
    match &err {
        Error::Server(status) => {
            assert_eq!(
                status.code,
                tonic::Code::InvalidArgument as i32,
                "Expected InvalidArgument, got code {}: {}",
                status.code,
                status.message
            );
        }
        other => panic!("Expected Server error for invalid signature, got: {other:?}"),
    }
}

#[sim_test]
async fn execute_transaction_idempotency() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result1 = client
        .execute_transaction(signed_tx.clone(), None, None)
        .await
        .expect("First execution should succeed");

    assert!(
        is_success(
            result1
                .body()
                .effects()
                .expect("Failed to get SDK effects from first execution result")
                .effects()
                .expect("Failed to get inner effects from first execution result")
                .status()
        ),
        "First execution should succeed"
    );

    // Re-submitting the same transaction must return the cached successful result.
    // The server uses TransactionOrchestrator with a NotifyRead pub-sub mechanism
    // that naturally returns cached effects for duplicates.
    let result2 = client
        .execute_transaction(signed_tx, None, None)
        .await
        .expect("Re-execution should return cached result");

    assert!(
        is_success(
            result2
                .body()
                .effects()
                .expect("Failed to get SDK effects from re-execution result")
                .effects()
                .expect("Failed to get inner effects from re-execution result")
                .status()
        ),
        "Re-execution should show success (cached result)"
    );
}
