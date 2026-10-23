// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The request metrics of the listener of the gRPC API, with a real tonic
//! client.

use iota_grpc_types::v1::ledger_service::{
    GetHealthRequest, StreamCheckpointsRequest, ledger_service_client::LedgerServiceClient,
};

use crate::{
    common::MockGrpcStateReader,
    harness::{connect, requests_in_flight, start, wait_until},
};

#[tokio::test]
async fn a_unary_request_is_counted_and_ends() {
    let (handle, metrics) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    ledger
        .get_health(GetHealthRequest::default().with_threshold_ms(u64::MAX))
        .await
        .unwrap();

    wait_until("the request to end", || {
        metrics
            .histogram_totals("request_completion_latency", &[])
            .count
            == 1
    })
    .await;
    assert_eq!(requests_in_flight(&metrics), 0);
    let started = metrics.histogram_totals("connection_inflight_requests", &[]);
    assert_eq!((started.count, started.sum), (1, 1.0), "one request, alone");
    assert_eq!(
        metrics
            .histogram_totals("connection_first_request_seconds", &[("protocol", "h2")])
            .count,
        1
    );
    assert_eq!(metrics.value("connection_inflight_requests_peak", &[]), 1.0);
}

#[tokio::test]
async fn a_subscription_is_in_flight_until_the_stream_ends() {
    let (handle, metrics) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await);

    let subscription = ledger
        .stream_checkpoints(StreamCheckpointsRequest::default())
        .await
        .unwrap()
        .into_inner();
    wait_until("the subscription to start", || {
        requests_in_flight(&metrics) == 1
    })
    .await;
    // The subscription has no end by itself: the request stays in flight.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(requests_in_flight(&metrics), 1);
    assert_eq!(
        metrics
            .histogram_totals("request_completion_latency", &[])
            .count,
        0
    );

    drop(subscription);
    wait_until("the subscription to end", || {
        requests_in_flight(&metrics) == 0
    })
    .await;
    assert_eq!(
        metrics
            .histogram_totals("request_completion_latency", &[])
            .count,
        1
    );
}
