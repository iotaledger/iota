// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_client::{Client, Error};
use iota_sdk_types::{Digest, ExecutionStatus, SignedTransaction, Transaction, UserSignature};
use iota_test_transaction_builder::{TestTransactionBuilder, make_transfer_iota_transaction};
use iota_types::base_types::IotaAddress;
use test_cluster::{TestCluster, TestClusterBuilder};

/// Set up a test cluster with gRPC enabled and connect a client.
///
/// This is the standard setup for all high-level gRPC client tests.
/// Waits for the specified checkpoint before returning.
pub async fn setup_grpc_test(wait_for_checkpoint: u64) -> (TestCluster, Client) {
    let test_cluster = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .build()
        .await;

    test_cluster
        .wait_for_checkpoint(wait_for_checkpoint, None)
        .await;

    let client = Client::connect(test_cluster.grpc_url())
        .await
        .expect("Failed to connect to gRPC server");

    (test_cluster, client)
}

/// Check if execution status is success.
pub fn is_success(status: &ExecutionStatus) -> bool {
    matches!(status, ExecutionStatus::Success)
}

/// Convert `iota_types::transaction::TransactionData` to
/// `iota_sdk_types::Transaction`.
///
/// BCS round-trip is required because iota_types and iota_sdk_types are
/// separate type systems that happen to have compatible BCS representations.
pub fn to_sdk_transaction(tx_data: &iota_types::transaction::TransactionData) -> Transaction {
    let bcs_bytes = bcs::to_bytes(tx_data).expect("BCS serialization failed");
    bcs::from_bytes(&bcs_bytes).expect("BCS deserialization failed")
}

/// Convert `iota_types::transaction::Transaction` to
/// `iota_sdk_types::SignedTransaction`.
pub fn to_sdk_signed_transaction(tx: iota_types::transaction::Transaction) -> SignedTransaction {
    let transaction = to_sdk_transaction(tx.transaction_data());

    let signatures: Vec<UserSignature> = tx
        .tx_signatures()
        .iter()
        .map(|sig| {
            // BCS round-trip for type system compatibility
            let sig_bytes = bcs::to_bytes(sig).expect("BCS serialization failed");
            bcs::from_bytes(&sig_bytes).expect("Signature deserialization failed")
        })
        .collect();

    SignedTransaction {
        transaction,
        signatures,
    }
}

/// Create a signed transaction for testing (IOTA transfer to random recipient).
pub async fn create_signed_transaction(test_cluster: &TestCluster) -> SignedTransaction {
    let recipient = IotaAddress::random_for_testing_only();
    let tx = make_transfer_iota_transaction(&test_cluster.wallet, Some(recipient), Some(100)).await;
    to_sdk_signed_transaction(tx)
}

/// Create an unsigned transaction for simulation testing.
pub async fn create_transaction_for_simulation(test_cluster: &TestCluster) -> Transaction {
    let (sender, gas) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();

    let rgp = test_cluster.get_reference_gas_price().await;

    let tx_data = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, sender)
        .build();

    to_sdk_transaction(&tx_data)
}

/// Execute a transaction and return its digest.
///
/// This is useful for tests that need a finalized transaction to query.
pub async fn execute_transaction_and_get_digest(test_cluster: &TestCluster) -> Digest {
    let (sender, gas) = test_cluster
        .wallet
        .get_one_gas_object()
        .await
        .unwrap()
        .unwrap();
    let rgp = test_cluster.get_reference_gas_price().await;
    let transaction_data = TestTransactionBuilder::new(sender, gas, rgp)
        .transfer_iota(None, sender)
        .build();
    let signed_transaction = test_cluster.wallet.sign_transaction(&transaction_data);
    let transaction_digest = *signed_transaction.digest();

    test_cluster
        .wallet
        .execute_transaction_may_fail(signed_transaction)
        .await
        .unwrap();

    Digest::new(transaction_digest.into_inner())
}

/// Assert that a result is a "not found" style error.
///
/// The server may return different error types for not-found conditions:
/// - `NotFound` or `InvalidArgument` gRPC status
/// - `ProtoConversion` error when server returns empty data
/// - `Server` error with a message
pub fn assert_not_found_error<T: std::fmt::Debug>(result: Result<T, Error>) {
    assert!(result.is_err(), "Expected not-found error, got success");

    match result {
        Err(Error::ProtoConversion(_)) => {
            // Server returned empty/null data that failed to deserialize
        }
        Err(Error::Grpc(status)) => {
            assert!(
                status.code() == tonic::Code::NotFound
                    || status.code() == tonic::Code::InvalidArgument,
                "Expected NotFound or InvalidArgument, got: {:?}",
                status.code()
            );
        }
        Err(Error::Server(msg)) => {
            assert!(!msg.is_empty(), "Error message should not be empty");
        }
        Err(e) => panic!("Unexpected error type: {e:?}"),
        Ok(_) => unreachable!(),
    }
}
