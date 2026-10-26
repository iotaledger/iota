// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use iota_grpc_server::GrpcServerMetrics;
use iota_grpc_types::v1::{
    state_service::{ListOwnedObjectsRequest, state_service_client::StateServiceClient},
    transaction_execution_service::transaction_execution_service_client::TransactionExecutionServiceClient,
};
use iota_traffic_controller::TrafficController;
use iota_types::traffic_control::{PolicyConfig, PolicyType, Weight};
use prometheus_filtered::Registry;
use tonic::Code;

use crate::{
    common::{
        EXECUTE_TRANSACTIONS_PATH, MockGrpcStateReader, PendingExecutor,
        call_execute_transactions_with_grpc_timeout, execute_transactions_request,
        start_test_server_with,
    },
    harness::{MetricsReader, PREFIX, connect, start_with_executor},
};

/// A call that the server ends at its `grpc-timeout` is counted with the status
/// of its response, not as canceled.
#[tokio::test]
async fn a_call_past_its_grpc_timeout_is_counted_as_deadline_exceeded() {
    let (handle, reader) = start_with_executor(
        MockGrpcStateReader::default(),
        Some(Arc::new(PendingExecutor::default())),
        |_| {},
    )
    .await;

    assert_eq!(
        call_execute_transactions_with_grpc_timeout(handle.address(), Duration::from_millis(200))
            .await,
        Code::DeadlineExceeded
    );

    let requests = |status| {
        reader.value(
            "requests",
            &[("method", EXECUTE_TRANSACTIONS_PATH), ("status", status)],
        )
    };
    assert_eq!(
        (requests("DeadlineExceeded"), requests("canceled")),
        (1.0, 0.0),
        "(DeadlineExceeded, canceled)"
    );
}

/// A call that the client ends before the server answers is counted as
/// canceled.
#[tokio::test]
async fn a_call_that_the_client_ends_is_counted_as_canceled() {
    let executor = Arc::new(PendingExecutor::default());
    let (handle, reader) = start_with_executor(
        MockGrpcStateReader::default(),
        Some(executor.clone()),
        |_| {},
    )
    .await;
    let mut client = TransactionExecutionServiceClient::new(connect(&handle).await);
    let call = tokio::spawn(async move {
        client
            .execute_transactions(execute_transactions_request())
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), executor.started.notified())
        .await
        .expect("the call must reach the executor");
    call.abort();

    let requests = |status| {
        reader.value(
            "requests",
            &[("method", EXECUTE_TRANSACTIONS_PATH), ("status", status)],
        )
    };
    // The server sees the end of the call only when the reset of the stream
    // arrives.
    tokio::time::timeout(Duration::from_secs(5), async {
        while requests("canceled") == 0.0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the call must be counted");
    assert_eq!(
        (requests("canceled"), requests("DeadlineExceeded")),
        (1.0, 0.0),
        "(canceled, DeadlineExceeded)"
    );
}

/// A request that traffic control refuses is counted with the status of its
/// response.
#[tokio::test]
async fn a_request_that_traffic_control_refuses_is_counted() {
    const LIST_OWNED_OBJECTS_PATH: &str =
        "/iota.grpc.v1.state_service.StateService/ListOwnedObjects";

    let registry = Registry::new();
    let policy_config = PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        spam_policy_type: PolicyType::TestNConnIP(1),
        // Error policy off, so the refusal comes from the spam policy.
        error_policy_type: PolicyType::NoOp,
        spam_sample_rate: Weight::one(),
        dry_run: false,
        ..Default::default()
    };
    let client_id_source = policy_config.client_id_source.clone();
    let (handle, _reader) = start_test_server_with(
        Arc::new(MockGrpcStateReader::default()),
        None,
        Some(Arc::new(TrafficController::init_for_test(policy_config))),
        Some(client_id_source),
        Some(GrpcServerMetrics::new(&registry)),
        |_| {},
    )
    .await;
    let mut client = StateServiceClient::new(connect(&handle).await);

    // The first request is served and fails without an owner; it reaches the
    // spam threshold, so the second request is refused.
    for _ in 0..2 {
        client
            .list_owned_objects(ListOwnedObjectsRequest::default())
            .await
            .expect_err("a request without an owner fails");
    }

    let reader = MetricsReader::new(&registry).with_prefix(PREFIX);
    let requests = |status| {
        reader.value(
            "requests",
            &[("method", LIST_OWNED_OBJECTS_PATH), ("status", status)],
        )
    };
    assert_eq!(
        (requests("InvalidArgument"), requests("ResourceExhausted")),
        (1.0, 1.0),
        "(InvalidArgument, ResourceExhausted)"
    );
}
