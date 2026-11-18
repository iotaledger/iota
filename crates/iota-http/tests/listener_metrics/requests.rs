// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use axum::{Router, routing::get};
use bytes::Bytes;
use futures::StreamExt;
use http_body::Frame;
use http_body_util::StreamBody;
use iota_http::Config;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Notify,
};

use crate::common::{
    MetricsReader, SERVER_NAME, ServiceRequest, get_root, h2_connect, h2_connect_with_task, h2_get,
    h2_read, h2_read_body, ok_app, serve, serve_handle, serve_service, serve_tls, setup,
    tls_configs, wait_until,
};

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
    let address = serve(Config::default(), &metrics, ok_app());

    let mut streams = vec![];
    for _ in 0..3 {
        streams.push(TcpStream::connect(address).await.unwrap());
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
}

#[tokio::test]
async fn a_connection_without_requests_has_no_idle_gap() {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, ok_app());

    let stream = TcpStream::connect(address).await.unwrap();
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
            .histogram_totals("connection_lifetime_seconds", &[])
            .count,
        1
    );
    assert_eq!(
        reader
            .histogram_totals("connection_idle_seconds", &[])
            .count,
        0
    );
}

#[tokio::test]
async fn idle_gaps_are_timed() {
    const BEFORE_FIRST: Duration = Duration::from_millis(300);
    const BEFORE_SECOND: Duration = Duration::from_millis(300);
    const BEFORE_CLOSE: Duration = Duration::from_millis(900);
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, ok_app());

    let mut stream = TcpStream::connect(address).await.unwrap();
    wait_until("the connection to be counted", || {
        reader.value("inbound_connections", &[]) == 1.0
    })
    .await;
    tokio::time::sleep(BEFORE_FIRST).await;
    get_root(&mut stream).await;
    tokio::time::sleep(BEFORE_SECOND).await;
    get_root(&mut stream).await;
    tokio::time::sleep(BEFORE_CLOSE).await;
    drop(stream);
    wait_until("the connection to close", || {
        reader.value("inbound_connections", &[]) == 0.0
    })
    .await;

    let idle_until_request =
        reader.histogram_totals("connection_idle_seconds", &[("end", "request")]);
    assert_eq!(idle_until_request.count, 1);
    assert!(
        (0.3..5.0).contains(&idle_until_request.sum),
        "gap before the second request {}",
        idle_until_request.sum
    );
    let idle_until_close = reader.histogram_totals("connection_idle_seconds", &[("end", "close")]);
    assert_eq!(idle_until_close.count, 1);
    assert!(
        (0.9..5.0).contains(&idle_until_close.sum),
        "gap before the close {}",
        idle_until_close.sum
    );
}

#[tokio::test]
async fn http2_idle_gaps_are_timed() {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, ok_app());

    let sender = h2_connect(address, h2::client::Builder::new()).await;
    h2_read(h2_get(&sender, "/").await).await;
    wait_until("the first request to end", || completed(&reader) == 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    h2_read(h2_get(&sender, "/").await).await;

    let idle_until_request =
        reader.histogram_totals("connection_idle_seconds", &[("end", "request")]);
    assert_eq!(idle_until_request.count, 1);
    assert!(
        (0.3..5.0).contains(&idle_until_request.sum),
        "gap {}",
        idle_until_request.sum
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
    let address = serve(Config::default(), &metrics, app);

    let sender = h2_connect(address, h2::client::Builder::new()).await;
    let mut responses = vec![];
    for _ in 0..N {
        responses.push(tokio::spawn(h2_read(h2_get(&sender, "/").await)));
    }
    for response in responses {
        assert_eq!(response.await.unwrap(), 2);
    }

    // No request ends before all arrived, so the values are 1, 2, ... N.
    let inflight_requests = reader.histogram_totals("connection_inflight_requests", &[]);
    assert_eq!(inflight_requests.count, N as u64);
    assert_eq!(inflight_requests.sum, (N * (N + 1) / 2) as f64);
    assert_eq!(
        reader.value("connection_inflight_requests_peak", &[]),
        N as f64
    );
    wait_until("the requests to end", || completed(&reader) == N as u64).await;
}

const CHUNK: usize = 256 * 1024;

/// The route `/big` sends `chunks` chunks of 256 KiB; `/full` sends 256 KiB as
/// a single buffer; `/trailers` sends 256 KiB and then trailers, like a gRPC
/// response; `/small` sends 2 bytes.
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
        .route(
            "/trailers",
            get(|| async {
                let frames = futures::stream::iter([
                    Ok::<_, std::convert::Infallible>(Frame::data(Bytes::from(vec![7u8; CHUNK]))),
                    Ok(Frame::trailers(http::HeaderMap::new())),
                ]);
                axum::body::Body::new(StreamBody::new(frames))
            }),
        )
        .route("/small", get(|| async { "ok" }))
}

/// A client that lets the server send 1 KiB of a response.
async fn small_window_client(address: std::net::SocketAddr) -> h2::client::SendRequest<Bytes> {
    h2_connect(
        address,
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
    let address = serve(Config::default(), metrics, big_app(chunks));

    let sender = small_window_client(address).await;
    let big_response = h2_get(&sender, "/big").await;
    let big_body = big_response.await.unwrap().into_body();
    h2_read(h2_get(&sender, "/small").await).await;
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

/// The body reports its end together with its only data frame, like the body
/// of a response that is a single buffer. The request ends when the transport
/// takes that frame, before the client reads it.
#[tokio::test]
async fn a_response_of_one_data_frame_ends_when_the_transport_takes_it() {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, big_app(1));

    let sender = small_window_client(address).await;
    let full_body = h2_get(&sender, "/full").await.await.unwrap().into_body();
    wait_until("the full request to end", || completed(&reader) == 1).await;
    assert_eq!(in_flight(&reader), 0.0);

    assert_eq!(h2_read_body(full_body).await, CHUNK);
}

/// The transport takes the trailers only after the window of the client has
/// taken the data before them.
#[tokio::test]
async fn a_response_with_trailers_stays_in_flight_until_the_client_reads_its_data() {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, big_app(1));

    let sender = small_window_client(address).await;
    let body = h2_get(&sender, "/trailers")
        .await
        .await
        .unwrap()
        .into_body();
    h2_read(h2_get(&sender, "/small").await).await;
    wait_until("the small request to end", || completed(&reader) >= 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(completed(&reader), 1, "only the small request ended");
    assert_eq!(
        in_flight(&reader),
        1.0,
        "the unread response is still in flight"
    );

    assert_eq!(h2_read_body(body).await, CHUNK);
    wait_until("the response with trailers to end", || {
        completed(&reader) == 2
    })
    .await;
}

#[tokio::test]
async fn a_held_response_keeps_the_connection_busy_for_the_next_requests() {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, big_app(3));

    let sender = small_window_client(address).await;
    let big_body = h2_get(&sender, "/big").await.await.unwrap().into_body();
    h2_read(h2_get(&sender, "/small").await).await;
    wait_until("the first small request to end", || completed(&reader) == 1).await;
    h2_read(h2_get(&sender, "/small").await).await;
    wait_until("the second small request to end", || {
        completed(&reader) == 2
    })
    .await;

    let counts = |reader: &MetricsReader| {
        let inflight_requests = reader.histogram_totals("connection_inflight_requests", &[]);
        (inflight_requests.count, inflight_requests.sum)
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
    h2_read(h2_get(&sender, "/small").await).await;
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
    let address = serve_tls(Config::default(), &metrics, ok_app());
    let (_, client_config) = tls_configs();

    let tcp_stream = TcpStream::connect(address).await.unwrap();
    let mut tls_stream = tokio_rustls::TlsConnector::from(Arc::new(client_config))
        .connect(SERVER_NAME.to_string().try_into().unwrap(), tcp_stream)
        .await
        .unwrap();
    tls_stream
        .write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    let mut response = [0u8; 256];
    assert!(tls_stream.read(&mut response).await.unwrap() > 0);

    wait_until("the request to end", || completed(&reader) == 1).await;
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
    let address = serve(Config::default(), &metrics, app);

    let sender = h2_connect(address, h2::client::Builder::new()).await;
    h2_read(h2_get(&sender, "/").await).await;
    wait_until("the request to end", || completed(&reader) == 1).await;

    let completion_latency = reader.histogram_totals("request_completion_latency", &[]);
    let delivery_latency = reader.histogram_totals("response_delivery_latency", &[]);
    assert_eq!(delivery_latency.count, 1);
    assert!(
        completion_latency.sum >= 0.3,
        "completion {}",
        completion_latency.sum
    );
    assert!(
        completion_latency.sum - delivery_latency.sum >= 0.25,
        "the handler time is not in the delivery: {} of {}",
        delivery_latency.sum,
        completion_latency.sum
    );
}

#[tokio::test]
async fn delivery_includes_the_time_the_response_waits_for_the_client_window() {
    const PAUSE: Duration = Duration::from_millis(400);
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, big_app(3));

    let sender = small_window_client(address).await;
    let body = h2_get(&sender, "/big").await.await.unwrap().into_body();
    tokio::time::sleep(PAUSE).await;
    h2_read_body(body).await;
    wait_until("the request to end", || completed(&reader) == 1).await;

    let delivery_latency = reader.histogram_totals("response_delivery_latency", &[]);
    assert_eq!(delivery_latency.count, 1);
    assert!(
        (0.4..10.0).contains(&delivery_latency.sum),
        "delivery {}",
        delivery_latency.sum
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
    let address = serve(Config::default(), &metrics, app);

    let sender = h2_connect(address, h2::client::Builder::new()).await;
    let response = h2_get(&sender, "/").await;
    started.notified().await;
    drop(response);
    drop(sender);

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
    let address = serve(Config::default(), &metrics, app);

    let mut stream = TcpStream::connect(address).await.unwrap();
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
    let address = serve_service(
        Config::default(),
        &metrics,
        tower::service_fn(|_request: ServiceRequest| async {
            Err::<http::Response<axum::body::Body>, std::io::Error>(std::io::Error::other(
                "the service failed",
            ))
        }),
    );

    let mut stream = TcpStream::connect(address).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    let mut rest_of_response = Vec::new();
    stream.read_to_end(&mut rest_of_response).await.unwrap();

    assert_request_ended(&reader).await;
}

#[tokio::test]
async fn an_http2_connection_that_closes_while_a_response_is_held_ends_the_request() {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, big_app(3));

    let (sender, connection) = h2_connect_with_task(
        address,
        h2::client::Builder::new().initial_window_size(1024).clone(),
    )
    .await;
    let body = h2_get(&sender, "/big").await.await.unwrap().into_body();
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
            .histogram_totals("connection_lifetime_seconds", &[])
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
    drop(sender);
}

#[tokio::test]
async fn a_shutdown_while_a_request_is_in_flight_ends_the_request_and_records_one_close() {
    let (reader, metrics) = setup();
    let handle = serve_handle(Config::default(), &metrics, None, big_app(3));

    let sender = small_window_client(*handle.local_addr()).await;
    let body = h2_get(&sender, "/big").await.await.unwrap().into_body();
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
    drop(sender);
}

#[tokio::test]
async fn an_http2_stream_reset_in_the_middle_of_a_response_ends_the_request_and_keeps_the_connection()
 {
    let (reader, metrics) = setup();
    let address = serve(Config::default(), &metrics, big_app(3));

    let sender = small_window_client(address).await;
    let body = h2_get(&sender, "/big").await.await.unwrap().into_body();
    wait_until("the request to start", || in_flight(&reader) == 1.0).await;
    // Dropping the body sends RST_STREAM while the response is still held.
    drop(body);

    assert_request_ended(&reader).await;
    assert_eq!(reader.value("inbound_connections", &[]), 1.0);
    h2_read(h2_get(&sender, "/small").await).await;
    wait_until("the second request to end", || completed(&reader) == 2).await;
    assert_eq!(reader.value("inbound_connections", &[]), 1.0);
}
