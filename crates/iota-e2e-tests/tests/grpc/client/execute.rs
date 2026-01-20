// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::Error;
use iota_macros::sim_test;
use iota_sdk_types::UserSignature;

use super::common::{create_signed_transaction, is_success, setup_grpc_test};

#[sim_test]
async fn execute_transaction_transfer() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx, None)
        .await
        .expect("Failed to execute transaction");

    assert!(
        is_success(result.effects.status()),
        "Transaction should have succeeded"
    );

    // Verify gas was charged
    let gas_summary = result.effects.gas_summary();
    assert!(
        gas_summary.computation_cost > 0 || gas_summary.storage_cost > 0,
        "Some gas should have been charged"
    );
}

#[sim_test]
async fn execute_transaction_response_fields() {
    let (test_cluster, client) = setup_grpc_test(1).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx, None)
        .await
        .expect("Failed to execute transaction");

    assert!(
        is_success(result.effects.status()),
        "Effects should show successful execution"
    );
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
    let (test_cluster, client) = setup_grpc_test(1).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx, Some("transaction.effects"))
        .await
        .expect("Failed to execute transaction");

    assert!(
        is_success(result.effects.status()),
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
    let (test_cluster, client) = setup_grpc_test(1).await;

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
    let (test_cluster, client) = setup_grpc_test(1).await;

    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result1 = client
        .execute_transaction(signed_tx.clone(), None)
        .await
        .expect("First execution should succeed");

    assert!(
        is_success(result1.effects.status()),
        "First execution should succeed"
    );

    let result2 = client.execute_transaction(signed_tx, None).await;

    // Re-submitting the same transaction may either:
    // - Return the cached successful result
    // - Return an error indicating the transaction was already executed
    // Both behaviors are acceptable for idempotency
    match result2 {
        Ok(response) => {
            assert!(
                is_success(response.effects.status()),
                "Re-execution should show success (cached result)"
            );
        }
        Err(Error::Grpc(status)) => {
            // Server may reject duplicate transaction
            assert!(
                status.code() == tonic::Code::AlreadyExists
                    || status.code() == tonic::Code::InvalidArgument,
                "Expected AlreadyExists or InvalidArgument for duplicate, got: {:?}",
                status.code()
            );
        }
        Err(e) => panic!("Unexpected error for duplicate transaction: {e:?}"),
    }
}
