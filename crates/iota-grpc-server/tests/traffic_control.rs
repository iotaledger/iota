// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the traffic-control layer of the gRPC server, in
//! particular that errors feed the error policy. tonic returns a unary handler
//! error as a trailers-only response, so its `grpc-status` lands in the HTTP
//! response headers where the layer already sees it; per-item errors of a batch
//! API instead ride inside an otherwise successful response and must be
//! reported explicitly.

mod common;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use common::{MockGrpcStateReader, start_test_server_with_traffic_controller};
use iota_core::traffic_controller::TrafficController;
use iota_grpc_types::v1::{
    state_service::{ListOwnedObjectsRequest, state_service_client::StateServiceClient},
    transaction_execution_service::{
        ExecuteTransactionItem, ExecuteTransactionsRequest, execute_transaction_result,
        transaction_execution_service_client::TransactionExecutionServiceClient,
    },
    types::Address as ProtoAddress,
};
use iota_types::{
    digests::TransactionDigest,
    error::IotaError,
    messages_checkpoint::CheckpointSequenceNumber,
    quorum_driver_types::{
        ExecuteTransactionRequestV1, ExecuteTransactionResponseV1, QuorumDriverError,
    },
    traffic_control::{PolicyConfig, PolicyType, Weight},
    transaction::TransactionData,
    transaction_executor::{SimulateTransactionResult, TransactionExecutor, VmChecks},
};
use tonic::{Code, transport::Channel};

async fn connect_state_client(address: std::net::SocketAddr) -> StateServiceClient<Channel> {
    let channel = Channel::from_shared(format!("http://{address}"))
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to test gRPC server");
    StateServiceClient::new(channel)
}

/// A `TransactionExecutor` for tests whose requests fail validation before
/// reaching the executor.
struct UnreachableExecutor;

#[async_trait::async_trait]
impl TransactionExecutor for UnreachableExecutor {
    async fn execute_transaction(
        &self,
        _request: ExecuteTransactionRequestV1,
        _skip_certification: bool,
        _client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        unreachable!("test requests must fail validation before execution")
    }

    fn simulate_transaction(
        &self,
        _transaction: TransactionData,
        _checks: VmChecks,
    ) -> Result<SimulateTransactionResult, IotaError> {
        unreachable!("test requests must fail validation before simulation")
    }

    async fn wait_for_checkpoint_inclusion(
        &self,
        _digests: &[TransactionDigest],
        _timeout: Duration,
    ) -> Result<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>, IotaError> {
        unreachable!("test requests must fail validation before execution")
    }
}

/// Handler errors must count towards the error policy and eventually block
/// the client.
#[tokio::test]
async fn handler_errors_feed_the_error_policy() {
    let n: u64 = 5;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        error_policy_type: PolicyType::TestNConnIP(n - 1),
        dry_run: false,
        ..Default::default()
    };
    let traffic_controller = Arc::new(TrafficController::init_for_test(policy_config, None).await);
    let (handle, _reader) = start_test_server_with_traffic_controller(
        Arc::new(MockGrpcStateReader::default()),
        traffic_controller,
        None,
    )
    .await;
    let mut client = connect_state_client(handle.address()).await;

    // A request without the required `owner` field is rejected by the handler
    // with `InvalidArgument`, which reaches the client via the response
    // trailers. The counter is updated only after the response is generated
    // while the limit is checked before the request is handled, so allow a
    // few extra requests before requiring the block.
    for _ in 0..2 * n {
        let status = client
            .list_owned_objects(ListOwnedObjectsRequest::default())
            .await
            .expect_err("request without owner should be rejected");
        if status.code() == Code::ResourceExhausted {
            assert!(
                status.message().contains("Too many requests"),
                "unexpected block message: {status:?}"
            );
            return;
        }
        assert_eq!(
            status.code(),
            Code::InvalidArgument,
            "unexpected error: {status:?}"
        );
        // Yield so the traffic controller's background tally task can process
        // the pending tally and update the blocklist before the next request.
        tokio::task::yield_now().await;
    }
    panic!(
        "expected the error policy to block the client within {} requests",
        2 * n
    );
}

/// Each item of a batch request must count individually towards the spam
/// policy, so a client cannot dilute its request rate by batching.
#[tokio::test]
async fn batched_requests_accrue_spam_per_item() {
    // The spam policy blocks a client once its request count reaches
    // `spam_threshold`. A single batch carries more items than the threshold,
    // so per-item accounting blocks the client while per-request accounting
    // (one tally per batch) could not.
    let batch_size: usize = 20;
    let spam_threshold: u64 = 15;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        spam_policy_type: PolicyType::TestNConnIP(spam_threshold),
        // Error policy off, so any block is attributable to the spam policy.
        error_policy_type: PolicyType::NoOp,
        // Count every tally deterministically.
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let traffic_controller = Arc::new(TrafficController::init_for_test(policy_config, None).await);
    let (handle, _reader) = start_test_server_with_traffic_controller(
        Arc::new(MockGrpcStateReader::default()),
        traffic_controller,
        Some(Arc::new(UnreachableExecutor)),
    )
    .await;
    let channel = Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to test gRPC server");
    let mut client = TransactionExecutionServiceClient::new(channel);

    // One batch of empty items: each fails validation and is reported as an
    // embedded per-item error, and each counts as one request for the spam
    // policy.
    let batch = ExecuteTransactionsRequest::default()
        .with_transactions(vec![ExecuteTransactionItem::default(); batch_size]);
    client
        .execute_transactions(batch)
        .await
        .expect("batch request itself should succeed");

    // The single batch already exceeds the spam threshold, so the client must
    // be blocked within a handful of follow-up requests. The probe budget stays
    // well below `spam_threshold` so per-request accounting (batch plus probes)
    // could not reach the threshold on its own.
    let probe = ExecuteTransactionsRequest::default()
        .with_transactions(vec![ExecuteTransactionItem::default()]);
    let probe_budget = 10;
    for _ in 0..probe_budget {
        // Yield so the background tally task drains the batch's tallies and
        // updates the blocklist before the next check.
        tokio::task::yield_now().await;
        if let Err(status) = client.execute_transactions(probe.clone()).await {
            assert_eq!(
                status.code(),
                Code::ResourceExhausted,
                "unexpected error: {status:?}"
            );
            assert!(
                status.message().contains("Too many requests"),
                "unexpected block message: {status:?}"
            );
            return;
        }
    }
    panic!("expected the batch's items to block the client via the spam policy");
}

/// Successful requests must not count towards the error policy or get
/// blocked.
#[tokio::test]
async fn successful_requests_do_not_feed_the_error_policy() {
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        // Block on the first error tally, so a successful request wrongly fed to
        // the error policy would block the client and fail this test.
        error_policy_type: PolicyType::TestNConnIP(1),
        dry_run: false,
        ..Default::default()
    };
    let traffic_controller = Arc::new(TrafficController::init_for_test(policy_config, None).await);
    let (handle, _reader) = start_test_server_with_traffic_controller(
        Arc::new(MockGrpcStateReader::default()),
        traffic_controller,
        None,
    )
    .await;
    let mut client = connect_state_client(handle.address()).await;

    let request = ListOwnedObjectsRequest::default()
        .with_owner(ProtoAddress::default().with_address(vec![1u8; 32]));
    for _ in 0..10 {
        client
            .list_owned_objects(request.clone())
            .await
            .expect("valid request should succeed");
        tokio::task::yield_now().await;
    }
}

/// Batch APIs embed per-item errors inside a successful gRPC response, where
/// the transport-level traffic control cannot see them. They must still feed
/// the error policy and eventually block the client.
#[tokio::test]
async fn embedded_batch_errors_feed_the_error_policy() {
    let n: u64 = 5;
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        error_policy_type: PolicyType::TestNConnIP(n - 1),
        dry_run: false,
        ..Default::default()
    };
    let traffic_controller = Arc::new(TrafficController::init_for_test(policy_config, None).await);
    let (handle, _reader) = start_test_server_with_traffic_controller(
        Arc::new(MockGrpcStateReader::default()),
        traffic_controller,
        Some(Arc::new(UnreachableExecutor)),
    )
    .await;
    let channel = Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to test gRPC server");
    let mut client = TransactionExecutionServiceClient::new(channel);

    // An empty `ExecuteTransactionItem` fails validation, which the batch API
    // reports as a per-item error embedded in an OK response.
    let request = ExecuteTransactionsRequest::default()
        .with_transactions(vec![ExecuteTransactionItem::default()]);
    for _ in 0..2 * n {
        match client.execute_transactions(request.clone()).await {
            Ok(response) => {
                let result = response
                    .get_ref()
                    .transaction_results
                    .first()
                    .expect("response should contain one result");
                assert!(
                    matches!(
                        &result.result,
                        Some(execute_transaction_result::Result::Error(error))
                            if Code::from(error.code) == Code::InvalidArgument
                    ),
                    "expected an embedded InvalidArgument error: {result:?}"
                );
            }
            Err(status) => {
                assert_eq!(
                    status.code(),
                    Code::ResourceExhausted,
                    "unexpected error: {status:?}"
                );
                assert!(
                    status.message().contains("Too many requests"),
                    "unexpected block message: {status:?}"
                );
                return;
            }
        }
        // Yield so the traffic controller's background tally task can process
        // the pending tally and update the blocklist before the next request.
        tokio::task::yield_now().await;
    }
    panic!(
        "expected the error policy to block the client within {} requests",
        2 * n
    );
}
