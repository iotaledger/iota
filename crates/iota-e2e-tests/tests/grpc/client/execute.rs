// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::{GrpcError, read_mask_fields::TransactionField};
use iota_macros::sim_test;
use iota_sdk_types::{Address, UserSignature};
use iota_test_transaction_builder::make_transfer_iota_transaction;
use iota_types::effects::TransactionEffectsAPI;

use super::super::utils::{create_signed_transaction, is_success, setup_grpc_test};

#[sim_test]
async fn execute_transaction_transfer() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let signed_tx = create_signed_transaction(&test_cluster).await;

    let result = client
        .execute_transaction(signed_tx)
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
    let gas_summary = effects.gas_cost_summary();
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
    let recipient = Address::random();
    let amount = 9;

    let tx =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(amount)).await;

    let result = client
        .execute_transaction(tx.into())
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
        .execute_transaction(signed_tx)
        .read_mask(TransactionField::EFFECTS)
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

    let result = client.execute_transaction(signed_tx).await;

    // With batch semantics, per-item validation errors come back as Error::Server
    let err = result.expect_err("Expected error for invalid signature");
    match &err {
        GrpcError::Server(status) => {
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
        .execute_transaction(signed_tx.clone())
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
        .execute_transaction(signed_tx)
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

async fn execute_transaction_error(
    client: &iota_grpc_client::GrpcClient,
    tx: iota_sdk_types::SignedTransaction,
) -> iota_grpc_types::google::rpc::Status {
    match client
        .execute_transaction(tx)
        .await
        .expect_err("the transaction must be rejected")
    {
        GrpcError::Server(status) => status,
        other => panic!("expected a server error, got: {other:?}"),
    }
}

#[sim_test]
async fn execute_transaction_missing_gas_object_error() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;
    let (sender, _) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();
    let gas_price = test_cluster.get_reference_gas_price().await;
    let missing_gas = iota_sdk_types::ObjectReference::new(
        iota_sdk_types::ObjectId::new([7; 32]),
        iota_sdk_types::Version::from_u64(1),
        iota_sdk_types::ObjectDigest::new([0; 32]),
    );
    let tx = test_cluster.wallet.sign_transaction(
        &iota_test_transaction_builder::TestTransactionBuilder::new(sender, missing_gas, gas_price)
            .transfer_iota(Some(1), sender)
            .build(),
    );

    // Validators report a missing input as retriable, since it may only be
    // missing locally, so the driver retries until it times out.
    let status = execute_transaction_error(&client, tx.into()).await;
    assert_eq!(
        status.code,
        i32::from(tonic::Code::Unavailable),
        "{status:?}"
    );
    assert_eq!(status.message, "timed-out before finality could be reached");
    assert!(status.details.is_empty(), "{status:?}");
}

#[sim_test]
async fn execute_transaction_double_used_object_error() {
    let (test_cluster, client) = setup_grpc_test(Some(1), None).await;

    let first =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(Address::random()), Some(1))
            .await;
    let second =
        make_transfer_iota_transaction(&test_cluster.wallet, Some(Address::random()), Some(2))
            .await;
    assert_ne!(first.digest(), second.digest());

    client
        .execute_transaction(first.into())
        .await
        .expect("the first transaction should execute");

    let status = execute_transaction_error(&client, second.into()).await;
    assert_eq!(
        status.code,
        i32::from(tonic::Code::InvalidArgument),
        "{status:?}"
    );
    assert!(
        status.message.starts_with(
            "Invalid transaction: unknown error: Transaction was rejected as invalid by more \
             than 1/3 of validator stake during submission (non-retriable): "
        ),
        "{status:?}"
    );
    assert!(
        status.message.contains("is not available for consumption"),
        "{status:?}"
    );
    assert_eq!(status.details.len(), 1, "{status:?}");
    let error_info = status.details[0]
        .to_msg::<iota_grpc_types::google::rpc::ErrorInfo>()
        .expect("the detail must be an ErrorInfo");
    assert_eq!(error_info.reason, "REJECTED_BY_VALIDATORS");
}
