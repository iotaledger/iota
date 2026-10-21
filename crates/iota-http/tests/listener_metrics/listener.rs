// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use iota_http::Config;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::common::*;

#[tokio::test]
async fn accepted_connections_are_counted_by_scope() {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, ok_app());

    let mut streams = vec![];
    for _ in 0..3 {
        streams.push(TcpStream::connect(addr).await.unwrap());
    }
    wait_until("the connections to be counted", || {
        reader.value("inbound_connections", &[]) == 3.0
    })
    .await;

    assert_eq!(
        reader.value("inbound_connections_accepted", &[("scope", "loopback")]),
        3.0
    );
    assert_eq!(reader.value("inbound_connections_peak", &[]), 3.0);
    assert!(
        !reader.has_family("pending_handshakes_peak"),
        "a plain listener has no TLS metrics"
    );

    drop(streams);
    wait_until("the connections to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;
    assert_eq!(reader.value("inbound_connections_peak", &[]), 3.0);
    let lifetime = reader.histogram_totals("connection_lifetime_seconds", &[]);
    assert_eq!(lifetime.count, 3);
    assert!(lifetime.sum < 5.0, "{}", lifetime.sum);
}

#[tokio::test]
async fn lifetime_is_the_time_from_accept_to_close() {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, ok_app());

    let stream = TcpStream::connect(addr).await.unwrap();
    wait_until("the connection to be counted", || {
        reader.value("inbound_connections", &[]) == 1.0
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    drop(stream);

    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;
    let lifetime = reader.histogram_totals("connection_lifetime_seconds", &[]);
    assert_eq!(lifetime.count, 1);
    assert!(
        (0.3..5.0).contains(&lifetime.sum),
        "lifetime {}",
        lifetime.sum
    );
}

#[tokio::test]
async fn open_connections_return_to_zero_when_the_server_stops() {
    let (reader, metrics) = setup();
    let handle = serve_handle(Config::default(), &metrics, None, ok_app());

    let mut stream = TcpStream::connect(handle.local_addr()).await.unwrap();
    get_root(&mut stream).await;
    assert_eq!(reader.value("inbound_connections", &[]), 1.0);

    handle.trigger_shutdown();
    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;
    assert_eq!(
        reader
            .histogram_totals("connection_lifetime_seconds", &[])
            .count,
        1
    );
}

#[tokio::test]
async fn tls_handshake_completed() {
    let (reader, metrics) = setup();
    let addr = serve_tls(Config::default(), &metrics, ok_app());
    let (_, client_config) = tls_configs();

    let tcp = TcpStream::connect(addr).await.unwrap();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
    let mut tls = connector
        .connect(SERVER_NAME.to_string().try_into().unwrap(), tcp)
        .await
        .unwrap();
    tls.write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    let mut buf = [0u8; 1024];
    assert!(tls.read(&mut buf).await.unwrap() > 0);

    let completed = reader.histogram_totals("handshake_latency", &[("result", "completed")]);
    assert_eq!(completed.count, 1);
    assert!(
        (0.0..5.0).contains(&completed.sum),
        "handshake {}",
        completed.sum
    );
    assert_eq!(
        reader
            .histogram_totals("handshake_latency", &[("result", "failed")])
            .count,
        0
    );
    assert_eq!(reader.value("pending_handshakes_peak", &[]), 1.0);
    assert_eq!(reader.value("inbound_connections", &[]), 1.0, "still open");
}

#[tokio::test]
async fn tls_handshake_timed_out() {
    let (reader, metrics) = setup();
    let addr = serve_tls(
        Config::default().handshake_timeout(Some(Duration::from_millis(200))),
        &metrics,
        ok_app(),
    );

    let mut silent = TcpStream::connect(addr).await.unwrap();
    let mut buf = [0u8; 1];
    let _ = silent.read(&mut buf).await;

    wait_until("the handshake to time out", || {
        reader
            .histogram_totals("handshake_latency", &[("result", "timed_out")])
            .count
            == 1
    })
    .await;
    let timed_out = reader.histogram_totals("handshake_latency", &[("result", "timed_out")]);
    assert!(
        (0.2..5.0).contains(&timed_out.sum),
        "handshake {}",
        timed_out.sum
    );
    assert_eq!(reader.value("pending_handshakes_peak", &[]), 1.0);
    wait_until("the close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;
}

#[tokio::test]
async fn garbage_instead_of_a_client_hello_is_a_failed_handshake() {
    let (reader, metrics) = setup();
    let addr = serve_tls(Config::default(), &metrics, ok_app());

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"this is not a TLS record, it is long enough to be read\r\n\r\n")
        .await
        .unwrap();

    wait_until("the handshake to fail", || {
        reader
            .histogram_totals("handshake_latency", &[("result", "failed")])
            .count
            == 1
    })
    .await;
    assert_eq!(
        reader
            .histogram_totals("handshake_latency", &[("result", "completed")])
            .count,
        0
    );
    assert_eq!(
        reader
            .histogram_totals("handshake_latency", &[("result", "timed_out")])
            .count,
        0
    );
    let failed = reader.histogram_totals("handshake_latency", &[("result", "failed")]);
    assert!(failed.sum < 5.0, "handshake {}", failed.sum);
    wait_until("the failed connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;
}

#[tokio::test]
async fn a_handshake_pending_at_shutdown_is_dropped() {
    let (reader, metrics) = setup();
    let (server_config, _) = tls_configs();
    let handle = serve_handle(Config::default(), &metrics, Some(server_config), ok_app());

    let _silent = TcpStream::connect(handle.local_addr()).await.unwrap();
    wait_until("the connection to be counted", || {
        reader.value("inbound_connections", &[]) == 1.0
    })
    .await;

    handle.trigger_shutdown();
    wait_until("the handshake to be dropped", || {
        reader
            .histogram_totals("handshake_latency", &[("result", "dropped")])
            .count
            == 1
    })
    .await;
    assert_eq!(
        reader
            .histogram_totals("handshake_latency", &[("result", "completed")])
            .count,
        0
    );
    wait_until("the close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;
}

#[tokio::test]
async fn pending_handshakes_peak_counts_the_silent_connections() {
    let (reader, metrics) = setup();
    let addr = serve_tls(Config::default(), &metrics, ok_app());

    let mut silent = vec![];
    for _ in 0..3 {
        silent.push(TcpStream::connect(addr).await.unwrap());
    }
    wait_until("the connections to be counted", || {
        reader.value("inbound_connections", &[]) == 3.0
    })
    .await;

    assert_eq!(reader.value("pending_handshakes_peak", &[]), 3.0);
    drop(silent);
}

#[tokio::test]
async fn a_connection_refused_for_its_peer_is_not_left_open() {
    let (reader, metrics) = setup();
    let addr = serve_mutual_tls(
        Config::default().max_connections_per_peer(Some(1)),
        &metrics,
        ok_app(),
    );

    let _first = connect_mutual_tls(addr).await;
    let mut second = connect_mutual_tls(addr).await;
    let mut buf = [0u8; 1];
    let _ = second.read(&mut buf).await;

    wait_until("the second connection to close", || {
        reader.value("inbound_connections", &[]) == 1.0
    })
    .await;
    assert_eq!(reader.value("inbound_connections_accepted", &[]), 2.0);
    assert_eq!(
        reader
            .histogram_totals("connection_lifetime_seconds", &[])
            .count,
        1
    );
}
