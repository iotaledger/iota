// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the traffic-control layer of the gRPC server: that
//! client errors feed the error policy and that batch APIs are charged per item
//! by both the spam and error policies.
//!
//! tonic returns a unary handler error as a trailers-only response, so its
//! `grpc-status` lands in the HTTP response headers where the layer already
//! sees it. A batch API's items instead ride inside the response body (embedded
//! in a unary response or streamed lazily), so the handlers report each item to
//! the traffic controller explicitly.

mod common;

use std::{collections::BTreeMap, future::Future, sync::Arc, time::Duration};

use common::{MockGrpcStateReader, start_test_server, start_test_server_with_traffic_controller};
use iota_core::traffic_controller::TrafficController;
use iota_grpc_server::GrpcServerHandle;
use iota_grpc_types::v1::{
    ledger_service::{
        GetObjectsRequest, ObjectRequest, ObjectRequests,
        ledger_service_client::LedgerServiceClient,
    },
    state_service::{ListOwnedObjectsRequest, state_service_client::StateServiceClient},
    transaction_execution_service::{
        ExecuteTransactionItem, ExecuteTransactionsRequest,
        transaction_execution_service_client::TransactionExecutionServiceClient,
    },
    types::{Address as ProtoAddress, ObjectId as ProtoObjectId, ObjectReference},
};
use iota_sdk_types::TransactionDigest;
use iota_types::{
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

/// Error policy threshold for the block tests: the client is blocked once this
/// many client errors have been tallied, so the block is expected within this
/// many requests.
const ERROR_BLOCK_ATTEMPTS: u64 = 5;

/// Spam policy threshold for the batch tests.
const SPAM_THRESHOLD: u64 = 15;
/// Items in the single batch used to prove per-item spam accounting; larger
/// than [`SPAM_THRESHOLD`] so one batch alone exceeds it.
const SPAM_BATCH_SIZE: usize = 20;
/// Probe attempts made after the oversized batch. Kept below [`SPAM_THRESHOLD`]
/// so per-request accounting (the batch plus these probes) could not reach the
/// threshold on its own — only per-item accounting can.
const SPAM_PROBE_ATTEMPTS: usize = 3;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

/// Start a gRPC server backed by an empty mock store, wired to a traffic
/// controller running `policy_config`.
async fn server_with_policy(
    policy_config: PolicyConfig,
    executor: Option<Arc<dyn TransactionExecutor>>,
) -> GrpcServerHandle {
    let traffic_controller = Arc::new(TrafficController::init_for_test(policy_config, None).await);
    let (handle, _reader) = start_test_server_with_traffic_controller(
        Arc::new(MockGrpcStateReader::default()),
        traffic_controller,
        executor,
    )
    .await;
    handle
}

/// Connect a channel to the test server.
async fn connect(handle: &GrpcServerHandle) -> Channel {
    Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to test gRPC server")
}

/// A `GetObjects` request for `n` distinct object lookups.
fn get_objects_batch(n: usize) -> GetObjectsRequest {
    let requests = (0..n)
        .map(|i| {
            ObjectRequest::default().with_object_ref(
                ObjectReference::default()
                    .with_object_id(ProtoObjectId::default().with_object_id(vec![i as u8; 32])),
            )
        })
        .collect();
    GetObjectsRequest::default().with_requests(ObjectRequests::default().with_requests(requests))
}

/// An `ExecuteTransactions` request carrying `n` empty (invalid) items.
fn execute_batch(n: usize) -> ExecuteTransactionsRequest {
    ExecuteTransactionsRequest::default()
        .with_transactions(vec![ExecuteTransactionItem::default(); n])
}

/// Assert that the traffic controller blocks the client within `attempts`.
///
/// The blocklist is applied asynchronously by the background tally task, so a
/// block becomes visible only some time after the offending requests complete.
/// This yields between attempts and sleeps before the final one to give the
/// tally task time to apply the block, then treats a `ResourceExhausted` /
/// "Too many requests" response as success. Any other outcome is a not-yet-
/// blocked request and the loop continues.
async fn assert_client_blocked<F, Fut, T>(attempts: usize, mut request: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, tonic::Status>>,
{
    for attempt in 0..attempts {
        // Give the background tally task time to apply the blocklist before the
        // final attempt, which is expected to be rejected.
        if attempt == attempts - 1 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        } else {
            tokio::task::yield_now().await;
        }
        if let Err(status) = request().await {
            if status.code() == Code::ResourceExhausted {
                assert!(
                    status.message().contains("Too many requests"),
                    "unexpected block message: {status:?}"
                );
                return;
            }
        }
    }
    panic!("expected the traffic controller to block the client within {attempts} requests");
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Handler errors must count towards the error policy and eventually block the
/// client. A request without the required `owner` field is rejected by the
/// handler with `InvalidArgument`, which reaches the client and feeds the error
/// policy.
#[tokio::test]
async fn handler_errors_feed_the_error_policy() {
    let handle = server_with_policy(
        PolicyConfig {
            connection_blocklist_ttl_sec: 120,
            error_policy_type: PolicyType::TestNConnIP(ERROR_BLOCK_ATTEMPTS - 1),
            dry_run: false,
            ..Default::default()
        },
        None,
    )
    .await;
    let client = StateServiceClient::new(connect(&handle).await);

    assert_client_blocked(ERROR_BLOCK_ATTEMPTS as usize, || {
        let mut client = client.clone();
        async move {
            client
                .list_owned_objects(ListOwnedObjectsRequest::default())
                .await
        }
    })
    .await;
}

/// Batch APIs embed per-item errors inside a successful gRPC response, where
/// the transport-level traffic control cannot see them. They must still feed
/// the error policy and eventually block the client.
#[tokio::test]
async fn embedded_batch_errors_feed_the_error_policy() {
    let handle = server_with_policy(
        PolicyConfig {
            connection_blocklist_ttl_sec: 120,
            error_policy_type: PolicyType::TestNConnIP(ERROR_BLOCK_ATTEMPTS - 1),
            dry_run: false,
            ..Default::default()
        },
        Some(Arc::new(UnreachableExecutor)),
    )
    .await;
    let client = TransactionExecutionServiceClient::new(connect(&handle).await);

    // An empty `ExecuteTransactionItem` fails validation, which the batch API
    // reports as a per-item error embedded in an otherwise OK response.
    assert_client_blocked(ERROR_BLOCK_ATTEMPTS as usize, || {
        let mut client = client.clone();
        async move { client.execute_transactions(execute_batch(1)).await }
    })
    .await;
}

/// Each item of a batch request must count individually towards the spam
/// policy, so a client cannot dilute its request rate by batching. A single
/// batch carries more items than the threshold, so per-item accounting blocks
/// the client while per-request accounting (one tally per batch) could not.
#[tokio::test]
async fn batched_requests_accrue_spam_per_item() {
    let handle = server_with_policy(
        PolicyConfig {
            connection_blocklist_ttl_sec: 120,
            spam_policy_type: PolicyType::TestNConnIP(SPAM_THRESHOLD),
            // Error policy off, so any block is attributable to the spam policy.
            error_policy_type: PolicyType::NoOp,
            // Count every tally deterministically.
            spam_sample_rate: Weight::one(),
            dry_run: false,
            ..Default::default()
        },
        Some(Arc::new(UnreachableExecutor)),
    )
    .await;
    let mut client = TransactionExecutionServiceClient::new(connect(&handle).await);

    // One batch of empty items: each fails validation and is reported as an
    // embedded per-item error, and each counts as one request for the spam
    // policy.
    client
        .execute_transactions(execute_batch(SPAM_BATCH_SIZE))
        .await
        .expect("batch request itself should succeed");

    assert_client_blocked(SPAM_PROBE_ATTEMPTS, || {
        let mut client = client.clone();
        async move { client.execute_transactions(execute_batch(1)).await }
    })
    .await;
}

/// A streaming batch read (`get_objects`) must also count each requested item
/// individually towards the spam policy, charged from the request's item count.
#[tokio::test]
async fn batched_reads_accrue_spam_per_item() {
    let handle = server_with_policy(
        PolicyConfig {
            connection_blocklist_ttl_sec: 120,
            spam_policy_type: PolicyType::TestNConnIP(SPAM_THRESHOLD),
            // Error policy off, so any block is attributable to the spam policy.
            error_policy_type: PolicyType::NoOp,
            // Count every tally deterministically.
            spam_sample_rate: Weight::one(),
            dry_run: false,
            ..Default::default()
        },
        None,
    )
    .await;
    let mut client = LedgerServiceClient::new(connect(&handle).await);

    // One batch of object lookups: each requested item counts as one request for
    // the spam policy. The lookups miss in the mock store, which does not affect
    // the spam count.
    client
        .get_objects(get_objects_batch(SPAM_BATCH_SIZE))
        .await
        .expect("batch request itself should succeed");

    assert_client_blocked(SPAM_PROBE_ATTEMPTS, || {
        let mut client = client.clone();
        async move { client.get_objects(get_objects_batch(1)).await }
    })
    .await;
}

/// A read batch larger than the configured maximum is rejected. Bounding the
/// count also bounds the per-item traffic-control tally so a large batch cannot
/// flood the tally channel.
#[tokio::test]
async fn oversized_read_batch_is_rejected() {
    let max_batch: u32 = 5;
    let (handle, _reader) = start_test_server(Arc::new(MockGrpcStateReader::default()), |config| {
        config.max_get_objects_batch_size = max_batch;
    })
    .await;
    let mut client = LedgerServiceClient::new(connect(&handle).await);

    // At the limit: accepted (per-item lookups miss in the mock store, which is
    // reported as embedded results, not a request-level error).
    client
        .get_objects(get_objects_batch(max_batch as usize))
        .await
        .expect("batch at the limit should be accepted");

    // Over the limit: rejected with `InvalidArgument`.
    let status = client
        .get_objects(get_objects_batch(max_batch as usize + 1))
        .await
        .expect_err("batch over the limit should be rejected");
    assert_eq!(
        status.code(),
        Code::InvalidArgument,
        "unexpected error: {status:?}"
    );
}

/// Successful requests must not count towards the error policy or get blocked.
#[tokio::test]
async fn successful_requests_do_not_feed_the_error_policy() {
    let handle = server_with_policy(
        PolicyConfig {
            connection_blocklist_ttl_sec: 120,
            // Block on the first error tally, so a successful request wrongly fed
            // to the error policy would block the client and fail this test.
            error_policy_type: PolicyType::TestNConnIP(1),
            dry_run: false,
            ..Default::default()
        },
        None,
    )
    .await;
    let mut client = StateServiceClient::new(connect(&handle).await);

    let request = ListOwnedObjectsRequest::default()
        .with_owner(ProtoAddress::default().with_address(vec![1u8; 32]));
    for _ in 0..10 {
        client
            .list_owned_objects(request.clone())
            .await
            .expect("valid request should succeed");
        // Yield so the tally task runs; a wrongly tallied error would block the
        // next request.
        tokio::task::yield_now().await;
    }
}
