// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use axum::{Router, routing::get};
use bytes::Bytes;
use futures::StreamExt;
use iota_http::Config;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Notify,
};

use crate::common::*;

fn completed(reader: &MetricsReader) -> u64 {
    reader
        .histogram_totals("request_completion_latency", &[])
        .count
}

/// The requests that have started and not ended: each start observes the
/// requests in flight on its connection, and each end observes a completion.
fn in_flight(reader: &MetricsReader) -> f64 {
    // Read the ends first: a start always comes before its end.
    let ended = completed(reader);
    let started = reader
        .histogram_totals("connection_inflight_requests", &[])
        .count;
    (started - ended) as f64
}

#[tokio::test]
async fn http1_requests_are_counted_per_connection() {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, ok_app());

    let mut streams = vec![];
    for _ in 0..3 {
        streams.push(TcpStream::connect(addr).await.unwrap());
    }
    for (carried, stream) in streams.iter_mut().enumerate() {
        for _ in 0..=carried {
            get_root(stream).await;
        }
    }
    drop(streams);
    wait_until("the connections to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;

    assert_eq!(completed(&reader), 6);
    assert_eq!(in_flight(&reader), 0.0);
    assert_eq!(reader.value("connection_inflight_requests_peak", &[]), 1.0);
    assert_eq!(
        reader
            .histogram_totals("connection_first_request_seconds", &[("protocol", "h1")])
            .count,
        3
    );
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("end", "request")])
            .count,
        3,
        "0 + 1 + 2 gaps between requests"
    );
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("end", "close")])
            .count,
        3
    );
    assert_eq!(
        reader
            .histogram_totals("connection_lifetime_seconds", &[("used", "true")])
            .count,
        3
    );
    assert_eq!(
        reader
            .histogram_totals("connection_lifetime_seconds", &[("used", "false")])
            .count,
        0
    );
}

#[tokio::test]
async fn a_connection_without_requests_is_unused() {
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

    assert_eq!(
        reader
            .histogram_totals("connection_lifetime_seconds", &[("used", "false")])
            .count,
        1
    );
    assert_eq!(
        reader
            .histogram_totals("connection_first_request_seconds", &[])
            .count,
        0
    );
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[])
            .count,
        0
    );
}

#[tokio::test]
async fn first_request_and_idle_gaps_are_timed() {
    const FIRST: Duration = Duration::from_millis(300);
    const BEFORE_SECOND: Duration = Duration::from_millis(300);
    const BEFORE_CLOSE: Duration = Duration::from_millis(900);
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, ok_app());

    let mut stream = TcpStream::connect(addr).await.unwrap();
    wait_until("the connection to be counted", || {
        reader.value("inbound_connections", &[]) == 1.0
    })
    .await;
    tokio::time::sleep(FIRST).await;
    get_root(&mut stream).await;
    tokio::time::sleep(BEFORE_SECOND).await;
    get_root(&mut stream).await;
    tokio::time::sleep(BEFORE_CLOSE).await;
    drop(stream);
    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;

    let first = reader.histogram_totals("connection_first_request_seconds", &[("protocol", "h1")]);
    assert_eq!(first.count, 1);
    assert!(
        (0.3..0.9).contains(&first.sum),
        "first request {}",
        first.sum
    );
    let between = reader.histogram_totals(
        "connection_idle_seconds",
        &[("end", "request"), ("protocol", "h1")],
    );
    assert_eq!(between.count, 1);
    assert!(
        (0.3..0.9).contains(&between.sum),
        "gap before the second request {}",
        between.sum
    );
    let before_close = reader.histogram_totals(
        "connection_idle_seconds",
        &[("end", "close"), ("protocol", "h1")],
    );
    assert_eq!(before_close.count, 1);
    assert!(
        (0.9..5.0).contains(&before_close.sum),
        "gap before the close {}",
        before_close.sum
    );
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("protocol", "h2")])
            .count,
        0
    );
    assert_eq!(
        reader
            .histogram_totals("connection_first_request_seconds", &[("protocol", "h2")])
            .count,
        0
    );
}

#[tokio::test]
async fn http2_idle_gaps_have_the_protocol_h2() {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, ok_app());

    let send = h2_connect(addr, h2::client::Builder::new()).await;
    h2_read(h2_get(&send, "/").await).await;
    wait_until("the first request to end", || completed(&reader) == 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    h2_read(h2_get(&send, "/").await).await;

    let gap = reader.histogram_totals(
        "connection_idle_seconds",
        &[("end", "request"), ("protocol", "h2")],
    );
    assert_eq!(gap.count, 1);
    assert!((0.3..5.0).contains(&gap.sum), "gap {}", gap.sum);
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("protocol", "h1")])
            .count,
        0
    );
}

#[tokio::test]
async fn http2_requests_in_flight() {
    const N: usize = 8;
    let (reader, metrics) = setup();
    let barrier = Arc::new(tokio::sync::Barrier::new(N));
    let app = Router::new().route(
        "/",
        get(move || {
            let barrier = barrier.clone();
            async move {
                barrier.wait().await;
                "ok"
            }
        }),
    );
    let addr = serve(Config::default(), &metrics, app);

    // The requests start right after the connection is ready, so they are in
    // its first second.
    let send = h2_connect(addr, h2::client::Builder::new()).await;
    let mut responses = vec![];
    for _ in 0..N {
        responses.push(tokio::spawn(h2_read(h2_get(&send, "/").await)));
    }
    for response in responses {
        assert_eq!(response.await.unwrap(), 2);
    }

    // No request ends before all arrived, so the values are 1, 2, ... N.
    let inflight = reader.histogram_totals("connection_inflight_requests", &[]);
    assert_eq!(inflight.count, N as u64);
    assert_eq!(inflight.sum, (N * (N + 1) / 2) as f64);
    assert_eq!(
        reader.value("connection_inflight_requests_peak", &[]),
        N as f64
    );
    assert_eq!(
        reader
            .histogram_totals("connection_first_request_seconds", &[("protocol", "h2")])
            .count,
        1
    );
    wait_until("the requests to end", || completed(&reader) == N as u64).await;
}

const CHUNK: usize = 256 * 1024;

/// The route `/big` sends `chunks` chunks of 256 KiB; `/small` sends 2 bytes.
fn big_app(chunks: usize) -> Router {
    Router::new()
        .route(
            "/big",
            get(move || async move {
                let chunks = (0..chunks)
                    .map(|_| Ok::<_, std::convert::Infallible>(Bytes::from(vec![7u8; CHUNK])));
                axum::body::Body::from_stream(futures::stream::iter(chunks))
            }),
        )
        .route(
            "/full",
            get(|| async { axum::body::Body::from(vec![7u8; CHUNK]) }),
        )
        .route("/small", get(|| async { "ok" }))
}

/// A client that lets the server send 1 KiB of a response.
async fn small_window_client(addr: std::net::SocketAddr) -> h2::client::SendRequest<Bytes> {
    h2_connect(
        addr,
        h2::client::Builder::new().initial_window_size(1024).clone(),
    )
    .await
}

/// A server that sends `chunks` chunks of 256 KiB, and a client that lets it
/// send 1 KiB. A second request shows that the server has handled the first.
async fn hold_response(
    reader: &MetricsReader,
    metrics: &iota_http::metrics::ListenerMetrics,
    chunks: usize,
) {
    let addr = serve(Config::default(), metrics, big_app(chunks));

    let send = small_window_client(addr).await;
    let big = h2_get(&send, "/big").await;
    let big_body = big.await.unwrap().into_body();
    h2_read(h2_get(&send, "/small").await).await;
    wait_until("the small request to end", || completed(reader) == 1).await;
    assert_eq!(
        in_flight(reader),
        1.0,
        "the unread response is still in flight"
    );

    assert_eq!(h2_read_body(big_body).await, CHUNK * chunks);
    wait_until("the big request to end", || completed(reader) == 2).await;
    assert_eq!(in_flight(reader), 0.0);
}

#[tokio::test]
async fn an_unread_response_keeps_the_request_in_flight() {
    let (reader, metrics) = setup();
    hold_response(&reader, &metrics, 3).await;
}

/// The body reports its end together with its only chunk, like the body of a
/// response that is a single buffer. Only the transport holding the chunk keeps
/// the request in flight.
#[tokio::test]
async fn the_request_stays_in_flight_while_the_transport_holds_the_last_chunk() {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, big_app(1));

    let send = small_window_client(addr).await;
    let full_body = h2_get(&send, "/full").await.await.unwrap().into_body();
    h2_read(h2_get(&send, "/small").await).await;
    wait_until("the small request to end", || completed(&reader) >= 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(completed(&reader), 1, "only the small request ended");
    assert_eq!(
        in_flight(&reader),
        1.0,
        "the unread response is still in flight"
    );

    assert_eq!(h2_read_body(full_body).await, CHUNK);
    wait_until("the full request to end", || completed(&reader) == 2).await;
}

#[tokio::test]
async fn a_held_response_keeps_the_connection_busy_for_the_next_requests() {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, big_app(3));

    let send = small_window_client(addr).await;
    let big_body = h2_get(&send, "/big").await.await.unwrap().into_body();
    h2_read(h2_get(&send, "/small").await).await;
    wait_until("the first small request to end", || completed(&reader) == 1).await;
    h2_read(h2_get(&send, "/small").await).await;
    wait_until("the second small request to end", || {
        completed(&reader) == 2
    })
    .await;

    let counts = |reader: &MetricsReader| {
        let all = reader.histogram_totals("connection_inflight_requests", &[]);
        (all.count, all.sum)
    };
    assert_eq!(
        counts(&reader),
        (3, 5.0),
        "1 + 2 + 2: the big response is held"
    );
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[])
            .count,
        0,
        "the connection was never idle"
    );

    assert_eq!(h2_read_body(big_body).await, CHUNK * 3);
    wait_until("the big request to end", || completed(&reader) == 3).await;
    h2_read(h2_get(&send, "/small").await).await;
    assert_eq!(
        counts(&reader),
        (4, 6.0),
        "the ended requests are not in flight any more"
    );
    assert_eq!(reader.value("connection_inflight_requests_peak", &[]), 2.0);
}

#[tokio::test]
async fn a_request_over_tls_is_counted() {
    let (reader, metrics) = setup();
    let addr = serve_tls(Config::default(), &metrics, ok_app());
    let (_, client_config) = tls_configs();

    let tcp = TcpStream::connect(addr).await.unwrap();
    let mut tls = tokio_rustls::TlsConnector::from(Arc::new(client_config))
        .connect(SERVER_NAME.to_string().try_into().unwrap(), tcp)
        .await
        .unwrap();
    tls.write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    let mut response = [0u8; 256];
    assert!(tls.read(&mut response).await.unwrap() > 0);

    wait_until("the request to end", || completed(&reader) == 1).await;
    assert_eq!(
        reader
            .histogram_totals("connection_first_request_seconds", &[("protocol", "h1")])
            .count,
        1
    );
    assert_eq!(reader.value("inbound_connections", &[]), 1.0, "still open");
}

#[tokio::test]
async fn delivery_starts_when_the_response_is_ready() {
    const HANDLER_TIME: Duration = Duration::from_millis(300);
    let (reader, metrics) = setup();
    let app = Router::new().route(
        "/",
        get(|| async {
            tokio::time::sleep(HANDLER_TIME).await;
            "ok"
        }),
    );
    let addr = serve(Config::default(), &metrics, app);

    let send = h2_connect(addr, h2::client::Builder::new()).await;
    h2_read(h2_get(&send, "/").await).await;
    wait_until("the request to end", || completed(&reader) == 1).await;

    let completion = reader.histogram_totals("request_completion_latency", &[]);
    let delivery = reader.histogram_totals("response_delivery_latency", &[]);
    assert_eq!(delivery.count, 1);
    assert!(completion.sum >= 0.3, "completion {}", completion.sum);
    assert!(
        completion.sum - delivery.sum >= 0.25,
        "the handler time is not in the delivery: {} of {}",
        delivery.sum,
        completion.sum
    );
}

#[tokio::test]
async fn delivery_includes_the_time_the_response_waits_for_the_client_window() {
    const PAUSE: Duration = Duration::from_millis(400);
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, big_app(3));

    let send = small_window_client(addr).await;
    let body = h2_get(&send, "/big").await.await.unwrap().into_body();
    tokio::time::sleep(PAUSE).await;
    h2_read_body(body).await;
    wait_until("the request to end", || completed(&reader) == 1).await;

    let delivery = reader.histogram_totals("response_delivery_latency", &[]);
    assert_eq!(delivery.count, 1);
    assert!(
        (0.4..10.0).contains(&delivery.sum),
        "delivery {}",
        delivery.sum
    );
}

#[tokio::test]
async fn a_request_dropped_before_its_response_is_ready_has_no_delivery() {
    let (reader, metrics) = setup();
    let started = Arc::new(Notify::new());
    let app = Router::new().route(
        "/",
        get({
            let started = started.clone();
            move || {
                let started = started.clone();
                async move {
                    started.notify_one();
                    std::future::pending::<&'static str>().await
                }
            }
        }),
    );
    let addr = serve(Config::default(), &metrics, app);

    let send = h2_connect(addr, h2::client::Builder::new()).await;
    let response = h2_get(&send, "/").await;
    started.notified().await;
    drop(response);
    drop(send);

    wait_until("the request to end", || completed(&reader) == 1).await;
    assert_eq!(
        reader
            .histogram_totals("response_delivery_latency", &[])
            .count,
        0
    );
}

async fn assert_request_ended(reader: &MetricsReader) {
    wait_until("the request to end", || completed(reader) == 1).await;
    assert_eq!(in_flight(reader), 0.0);
    assert_eq!(completed(reader), 1);
}

#[tokio::test]
async fn an_http1_client_that_closes_in_the_middle_of_a_response_ends_the_request() {
    let (reader, metrics) = setup();
    let app = Router::new().route(
        "/",
        get(|| async {
            let first = futures::stream::once(async {
                Ok::<_, std::convert::Infallible>(Bytes::from(vec![7u8; 1024]))
            });
            axum::body::Body::from_stream(first.chain(futures::stream::pending()))
        }),
    );
    let addr = serve(Config::default(), &metrics, app);

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    let mut head = [0u8; 64];
    assert!(stream.read(&mut head).await.unwrap() > 0);
    wait_until("the request to start", || in_flight(&reader) == 1.0).await;
    drop(stream);

    assert_request_ended(&reader).await;
}

#[tokio::test]
async fn a_service_that_fails_ends_the_request() {
    let (reader, metrics) = setup();
    let addr = serve_service(
        Config::default(),
        &metrics,
        tower::service_fn(|_request: ServiceRequest| async {
            Err::<http::Response<axum::body::Body>, std::io::Error>(std::io::Error::other(
                "the service failed",
            ))
        }),
    );

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    let mut rest = Vec::new();
    stream.read_to_end(&mut rest).await.unwrap();

    assert_request_ended(&reader).await;
}

#[tokio::test]
async fn an_http2_connection_that_closes_while_a_chunk_is_held_ends_the_request() {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, big_app(3));

    let (send, connection) = h2_connect_with_task(
        addr,
        h2::client::Builder::new().initial_window_size(1024).clone(),
    )
    .await;
    let body = h2_get(&send, "/big").await.await.unwrap().into_body();
    wait_until("the request to start", || in_flight(&reader) == 1.0).await;
    // The body is still held when the TCP connection closes.
    connection.abort();

    assert_request_ended(&reader).await;
    // The request outlived the connection guard: the close is recorded once,
    // with no idle gap, because the request was in flight at the close.
    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;
    assert_eq!(
        reader
            .histogram_totals("connection_lifetime_seconds", &[("used", "true")])
            .count,
        1
    );
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("end", "close")])
            .count,
        0
    );
    drop(body);
    drop(send);
}

#[tokio::test]
async fn a_shutdown_while_a_request_is_in_flight_ends_the_request_and_records_one_close() {
    let (reader, metrics) = setup();
    let handle = serve_handle(Config::default(), &metrics, None, big_app(3));

    let send = small_window_client(*handle.local_addr()).await;
    let body = h2_get(&send, "/big").await.await.unwrap().into_body();
    wait_until("the request to start", || in_flight(&reader) == 1.0).await;

    handle.trigger_shutdown();
    tokio::time::timeout(Duration::from_secs(10), handle.wait_for_shutdown())
        .await
        .expect("the server did not shut down");

    assert_request_ended(&reader).await;
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
    drop(body);
    drop(send);
}

#[tokio::test]
async fn an_http2_stream_reset_in_the_middle_of_a_response_ends_the_request_and_keeps_the_connection()
 {
    let (reader, metrics) = setup();
    let addr = serve(Config::default(), &metrics, big_app(3));

    let send = small_window_client(addr).await;
    let body = h2_get(&send, "/big").await.await.unwrap().into_body();
    wait_until("the request to start", || in_flight(&reader) == 1.0).await;
    // Dropping the body sends RST_STREAM while the response is still held.
    drop(body);

    assert_request_ended(&reader).await;
    assert_eq!(reader.value("inbound_connections", &[]), 1.0);
    h2_read(h2_get(&send, "/small").await).await;
    wait_until("the second request to end", || completed(&reader) == 2).await;
    assert_eq!(reader.value("inbound_connections", &[]), 1.0);
}

#[tokio::test]
async fn a_connection_upgraded_by_a_response_has_no_idle_gap() {
    use hyper_util::rt::TokioIo;

    let (reader, metrics) = setup();
    let app = Router::new().route(
        "/",
        get(|mut request: axum::extract::Request| async move {
            let on_upgrade = hyper::upgrade::on(&mut request);
            tokio::spawn(async move {
                if let Ok(upgraded) = on_upgrade.await {
                    let mut io = TokioIo::new(upgraded);
                    let _ = tokio::io::copy(&mut io, &mut tokio::io::sink()).await;
                }
            });
            axum::response::Response::builder()
                .status(http::StatusCode::SWITCHING_PROTOCOLS)
                .header("connection", "upgrade")
                .header("upgrade", "test")
                .body(axum::body::Body::empty())
                .unwrap()
        }),
    );
    let addr = serve(Config::default(), &metrics, app);

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nhost: x\r\nconnection: upgrade\r\nupgrade: test\r\n\r\n")
        .await
        .unwrap();
    let mut head = [0u8; 256];
    let read = stream.read(&mut head).await.unwrap();
    assert!(
        String::from_utf8_lossy(&head[..read]).starts_with("HTTP/1.1 101"),
        "{}",
        String::from_utf8_lossy(&head[..read])
    );
    wait_until("the request to end", || completed(&reader) == 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    drop(stream);
    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;

    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("end", "close")])
            .count,
        0,
        "the time of the upgraded connection is no idle gap"
    );
    assert_eq!(
        reader
            .histogram_totals("connection_lifetime_seconds", &[("used", "true")])
            .count,
        1
    );
}

/// A response with the status 200 to a CONNECT of an HTTP/1 client turns the
/// connection into a tunnel.
#[tokio::test]
async fn an_http1_connect_tunnel_has_no_idle_gap() {
    let (reader, metrics) = setup();
    let addr = serve_service(
        Config::default(),
        &metrics,
        tower::service_fn(|_request: ServiceRequest| async {
            Ok::<_, std::convert::Infallible>(http::Response::new(axum::body::Body::empty()))
        }),
    );

    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"CONNECT example.com:80 HTTP/1.1\r\nhost: example.com:80\r\n\r\n")
        .await
        .unwrap();
    let mut head = [0u8; 256];
    let read = stream.read(&mut head).await.unwrap();
    assert!(
        String::from_utf8_lossy(&head[..read]).starts_with("HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&head[..read])
    );
    wait_until("the request to end", || completed(&reader) == 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    drop(stream);
    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;

    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("end", "close")])
            .count,
        0
    );
}

/// In HTTP/2 a CONNECT is one stream, and the connection goes on with ordinary
/// requests: the time after the last one is an idle gap.
#[tokio::test]
async fn an_http2_connection_that_answered_a_connect_is_not_upgraded() {
    let (reader, metrics) = setup();
    let addr = serve_service(
        Config::default(),
        &metrics,
        tower::service_fn(|_request: ServiceRequest| async {
            Ok::<_, std::convert::Infallible>(http::Response::new(axum::body::Body::empty()))
        }),
    );

    let (send, connection) = h2_connect_with_task(addr, h2::client::Builder::new()).await;
    let mut ready = send.clone().ready().await.unwrap();
    let connect = http::Request::connect("example.com:80").body(()).unwrap();
    let (response, _stream) = ready.send_request(connect, false).unwrap();
    assert!(response.await.unwrap().status().is_success());
    wait_until("the request to end", || completed(&reader) == 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    connection.abort();
    drop(send);
    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;

    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[("end", "close")])
            .count,
        1,
        "the connection was idle until the close"
    );
}
