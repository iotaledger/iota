// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The connection metrics of the listener of the gRPC API, with a real tonic
//! client.

use std::{future::Future, time::Duration};

use tonic::transport::Channel;

use crate::{
    common::{MockGrpcStateReader, TestCertificate},
    harness::{MetricsReader, connect, start, start_with},
};

async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    for _ in 0..1000 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

/// Opens three connections, then closes them, and checks what the metrics say
/// at each point.
async fn assert_connections_are_counted<F, Fut>(metrics: &MetricsReader, connect: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = Channel>,
{
    let mut channels = vec![];
    for _ in 0..3 {
        channels.push(connect().await);
    }

    wait_until("the connections to be open", || {
        metrics.value("inbound_connections", &[]) == 3.0
    })
    .await;
    assert_eq!(
        metrics.value("inbound_connections_accepted", &[("scope", "loopback")]),
        3.0
    );
    assert_eq!(
        metrics.value("inbound_connections_accepted", &[("scope", "public")]),
        0.0
    );
    assert_eq!(metrics.value("inbound_connections_peak", &[]), 3.0);

    drop(channels);
    wait_until("the connections to close", || {
        metrics.value("inbound_connections", &[]) == 0.0
    })
    .await;
    assert_eq!(
        metrics.value("inbound_connections_peak", &[]),
        3.0,
        "the peak stays after the close"
    );
    assert_eq!(
        metrics
            .histogram_totals("connection_lifetime_seconds", &[])
            .count,
        3
    );
}

#[tokio::test]
async fn connections_are_counted() {
    let (handle, metrics) = start(MockGrpcStateReader::default()).await;

    assert_connections_are_counted(&metrics, || connect(&handle)).await;

    assert!(
        !metrics.has_family("pending_handshakes_peak"),
        "a plain listener has no TLS metrics"
    );
}

#[tokio::test]
async fn connections_and_handshakes_are_counted_over_tls() {
    let certificate = TestCertificate::new();
    let tls_config = certificate.tls_config.clone();
    let (handle, metrics) = start_with(MockGrpcStateReader::default(), |config| {
        config.tls = Some(tls_config)
    })
    .await;

    assert_connections_are_counted(&metrics, || certificate.connect(handle.address())).await;

    assert_eq!(
        metrics
            .histogram_totals("handshake_latency", &[("result", "completed")])
            .count,
        3
    );
    // The three connections open one after another, but the server may record
    // the end of a handshake after the client opens the next one: at least 1,
    // at most 3 handshakes are pending at once.
    let pending_peak = metrics.value("pending_handshakes_peak", &[]);
    assert!(
        (1.0..=3.0).contains(&pending_peak),
        "the peak of the pending handshakes is {pending_peak}"
    );
}
