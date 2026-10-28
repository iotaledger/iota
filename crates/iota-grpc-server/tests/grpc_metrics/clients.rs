// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The client metrics of the listener of the gRPC API, with a real tonic
//! client.

use crate::{
    common::MockGrpcStateReader,
    connections::wait_until,
    harness::{connect, start},
};

#[tokio::test]
async fn a_client_of_the_grpc_api_is_counted() {
    let (handle, metrics) = start(MockGrpcStateReader::new_from_iter(0..=10)).await;
    let _channels = [connect(&handle).await, connect(&handle).await];
    wait_until("the connections to be counted", || {
        metrics.histogram_totals("client_connections", &[]).count == 2
    })
    .await;

    let client_connections =
        metrics.histogram_totals("client_connections", &[("scope", "loopback")]);
    assert_eq!(
        (client_connections.count, client_connections.sum),
        (2, 3.0),
        "1 + 2"
    );
    assert_eq!(
        metrics.value("client_connections_peak", &[("scope", "loopback")]),
        2.0
    );
    assert_eq!(metrics.value("clients", &[("scope", "loopback")]), 1.0);
}
