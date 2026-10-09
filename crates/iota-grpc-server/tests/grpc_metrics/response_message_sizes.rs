// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::v1::{
    ledger_service::{
        GetEpochRequest, GetHealthRequest, GetServiceInfoRequest,
        ledger_service_client::LedgerServiceClient,
    },
    state_service::{ListOwnedObjectsRequest, state_service_client::StateServiceClient},
    transaction_execution_service::{
        ExecuteTransactionItem, ExecuteTransactionsRequest, SimulateTransactionItem,
        SimulateTransactionsRequest, ViewFunctionCallItem, ViewFunctionCallsRequest,
        transaction_execution_service_client::TransactionExecutionServiceClient,
    },
};
use prost::Message;

use crate::{
    common::{self, MockGrpcStateReader, UnreachableExecutor},
    harness::{
        ListFixture, ListResponses, MetricsReader, call_get_objects, call_list_methods, connect,
        list_fixture, list_fixture_with_coins, start, start_with_executor,
    },
};

const GET_HEALTH: &str = "/iota.grpc.v1.ledger_service.LedgerService/GetHealth";
const GET_OBJECTS: &str = "/iota.grpc.v1.ledger_service.LedgerService/GetObjects";
const LIST_OWNED_OBJECTS: &str = "/iota.grpc.v1.state_service.StateService/ListOwnedObjects";

/// The count and the sum of the observed sizes of all methods.
fn observed(metrics: &MetricsReader) -> (u64, usize) {
    observed_of(metrics, &[])
}

/// The count and the sum of the observed sizes of the series with `labels`.
fn observed_of(metrics: &MetricsReader, labels: &[(&str, &str)]) -> (u64, usize) {
    let hist = metrics.histogram_totals("response_message_bytes", labels);
    (hist.count, hist.sum as usize)
}

#[tokio::test]
async fn ledger_service_unary_responses_are_observed() {
    let (handle, metrics) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    let health = ledger
        .get_health(GetHealthRequest::default().with_threshold_ms(u64::MAX))
        .await
        .unwrap()
        .into_inner();
    let info = ledger
        .get_service_info(GetServiceInfoRequest::default())
        .await
        .unwrap()
        .into_inner();
    let epoch = ledger
        .get_epoch(GetEpochRequest::default())
        .await
        .unwrap()
        .into_inner();

    let sizes = [
        health.encoded_len(),
        info.encoded_len(),
        epoch.encoded_len(),
    ];
    assert_eq!(observed(&metrics), (3, sizes.iter().sum()));
    assert_eq!(
        observed_of(&metrics, &[("method", GET_HEALTH)]),
        (1, health.encoded_len()),
        "each method has its own series"
    );
}

#[tokio::test]
async fn state_and_package_service_unary_responses_are_observed() {
    let ListFixture {
        mock,
        owner,
        parent,
        package,
    } = list_fixture();
    let (handle, metrics) = start(mock).await;
    let channel = connect(&handle).await;

    let ListResponses {
        owned,
        fields,
        versions,
    } = call_list_methods(channel, owner, parent, package).await;

    let sizes = [
        owned.encoded_len(),
        fields.encoded_len(),
        versions.encoded_len(),
    ];
    assert_eq!(observed(&metrics), (3, sizes.iter().sum()));
}

/// The items of these requests are invalid, so the responses carry an error for
/// each item and the executor is never reached.
#[tokio::test]
async fn transaction_execution_service_unary_responses_are_observed() {
    let (handle, metrics) = start_with_executor(
        MockGrpcStateReader::default(),
        Some(Arc::new(UnreachableExecutor)),
        |_| {},
    )
    .await;
    let mut client = TransactionExecutionServiceClient::new(connect(&handle).await);

    let executed = client
        .execute_transactions(
            ExecuteTransactionsRequest::default()
                .with_transactions(vec![ExecuteTransactionItem::default(); 2]),
        )
        .await
        .unwrap()
        .into_inner();
    let simulated = client
        .simulate_transactions(
            SimulateTransactionsRequest::default()
                .with_transactions(vec![SimulateTransactionItem::default(); 2]),
        )
        .await
        .unwrap()
        .into_inner();
    let viewed = client
        .view_function_calls(
            ViewFunctionCallsRequest::default()
                .with_view_function_calls(vec![ViewFunctionCallItem::default(); 2]),
        )
        .await
        .unwrap()
        .into_inner();

    let sizes = [
        executed.encoded_len(),
        simulated.encoded_len(),
        viewed.encoded_len(),
    ];
    assert_eq!(observed(&metrics), (3, sizes.iter().sum()));
}

#[tokio::test]
async fn a_large_response_is_observed_with_its_whole_size() {
    let ListFixture { mock, owner, .. } = list_fixture_with_coins(400);
    let (handle, metrics) = start(mock).await;

    let owned = StateServiceClient::new(connect(&handle).await)
        .list_owned_objects(
            ListOwnedObjectsRequest::default()
                .with_owner(common::owner_proto(owner))
                .with_page_size(400),
        )
        .await
        .unwrap()
        .into_inner();

    assert_eq!(owned.objects.len(), 400);
    assert!(owned.encoded_len() > 32 * 1024, "{}", owned.encoded_len());
    assert_eq!(
        observed_of(&metrics, &[("method", LIST_OWNED_OBJECTS)]),
        (1, owned.encoded_len())
    );
}

#[tokio::test]
async fn each_message_of_a_stream_is_observed() {
    let call = call_get_objects().await;
    assert!(
        call.responses.len() > 1,
        "{} messages",
        call.responses.len()
    );

    let sizes = call.responses.iter().map(Message::encoded_len);
    assert_eq!(
        observed_of(&call.reader, &[("method", GET_OBJECTS)]),
        (call.responses.len() as u64, sizes.sum())
    );
}

#[tokio::test]
async fn a_failed_call_is_not_observed() {
    let (handle, metrics) = start(MockGrpcStateReader::default()).await;
    let failed = StateServiceClient::new(connect(&handle).await)
        .list_owned_objects(ListOwnedObjectsRequest::default())
        .await;
    assert!(failed.is_err());
    assert_eq!(observed(&metrics), (0, 0));
}
