// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::Error;
use iota_macros::sim_test;
use iota_sdk_types::UserSignature;

use super::{
    super::utils::setup_grpc_test,
    common::{create_signed_transaction, is_success},
};

#[sim_test]
async fn execute_transaction_transfer() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx, None)
        .await
        .expect("Failed to execute transaction");

    let effects = result
        .effects()
        .expect("Failed to get SDK effects from execution result");

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
        result.input_objects.is_some(),
        "Input objects should be present with default mask"
    );
    assert!(
        result.output_objects.is_some(),
        "Output objects should be present with default mask"
    );
}

#[sim_test]
async fn execute_transaction_minimal_mask() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx, Some("transaction.effects"))
        .await
        .expect("Failed to execute transaction");

    assert!(
        is_success(
            result
                .effects()
                .expect("Failed to get SDK effects from execution result with minimal mask")
                .status()
        ),
        "Effects should show successful execution"
    );
    assert!(
        result.input_objects.is_none(),
        "Input objects should not be present with minimal mask"
    );
    assert!(
        result.output_objects.is_none(),
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

    let result = client.execute_transaction(signed_tx, None).await;

    // Transaction with invalid signature should be rejected
    assert!(
        matches!(result, Err(Error::Grpc(_)) | Err(Error::Signature(_))),
        "Expected Grpc or Signature error, got: {result:?}"
    );
}

#[sim_test]
async fn execute_transaction_idempotency() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result1 = client
        .execute_transaction(signed_tx.clone(), None)
        .await
        .expect("First execution should succeed");

    assert!(
        is_success(
            result1
                .effects()
                .expect("Failed to get SDK effects from first execution result")
                .status()
        ),
        "First execution should succeed"
    );

    // Re-submitting the same transaction must return the cached successful result.
    // The server uses TransactionOrchestrator with a NotifyRead pub-sub mechanism
    // that naturally returns cached effects for duplicates.
    let result2 = client
        .execute_transaction(signed_tx, None)
        .await
        .expect("Re-execution should return cached result");

    assert!(
        is_success(
            result2
                .effects()
                .expect("Failed to get SDK effects from re-execution result")
                .status()
        ),
        "Re-execution should show success (cached result)"
    );
}
