// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The limit of responses in progress per connection, through a served
//! listener and h2's own client, which keeps to the windows the server
//! announces and resets a stream when its response future is dropped.

use std::{
    convert::Infallible,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use axum::{Extension, Router, routing::get};
use bytes::Bytes;
use iota_http::{
    Builder, Config, ResponseLimitLayer, ResponseSlot, ResponseSlots, ServerHandle,
    chunk_with_slot, metrics::ListenerMetrics,
};
use iota_metrics::test_utils::MetricsReader;
use prometheus_filtered::{MetricLevel, Registry};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// The stream window of a client whose responses stall once it stops
/// reading them: a response of [`BIG`] bytes does not fit.
const CLIENT_STREAM_WINDOW: u32 = 64 * 1024;
/// The connection window of such a client, large enough that a stalled
/// stream leaves room for the others.
const CLIENT_CONNECTION_WINDOW: u32 = 16 * 1024 * 1024;
/// The size of a response of `/big`.
const BIG: usize = 2 * 1024 * 1024;
/// How long a test waits for something that must happen.
const WAIT: Duration = Duration::from_secs(10);
/// How long a test waits to see that something does not happen.
const SETTLE: Duration = Duration::from_millis(300);

/// What a test server counts: the handlers of `/big` that ran.
#[derive(Default)]
struct Served(AtomicUsize);

impl Served {
    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

/// A served listener with the given limit, its metrics, and the count of
/// `/big` handlers that ran.
fn serve(
    limit: Option<usize>,
    config: impl FnOnce(Config) -> Config,
) -> (ServerHandle, MetricsReader, Arc<Served>) {
    let registry = Registry::new();
    let metrics = ListenerMetrics::new("t", &registry, MetricLevel::Info);
    let served = Arc::new(Served::default());
    let app = Router::new()
        .route(
            "/big",
            get({
                let served = served.clone();
                move || async move {
                    served.0.fetch_add(1, Ordering::SeqCst);
                    vec![7u8; BIG]
                }
            }),
        )
        .route("/items", get(items))
        .route(
            "/slots",
            get(|slots: Option<Extension<ResponseSlots>>| async move {
                if slots.is_some() { "some" } else { "none" }
            }),
        )
        .route(
            "/echo",
            axum::routing::post(|body: Bytes| async move { body.len().to_string() }),
        )
        .layer(axum::extract::DefaultBodyLimit::disable())
        .layer(
            ResponseLimitLayer::new().takes_slots_per_item(|parts| parts.uri.path() == "/items"),
        );
    let handle = Builder::new()
        .config(config(
            Config::default()
                .concurrency_limit_per_connection(limit)
                .metrics(Some(metrics)),
        ))
        .serve(("localhost", 0), app)
        .unwrap();
    (
        handle,
        MetricsReader::new(&registry).with_prefix("t"),
        served,
    )
}

/// The size of an item of `/items`: over the client's stream window, so the
/// client must read an item before h2 can drop its chunk.
const ITEM: usize = 100 * 1024;

/// A handler that sends two items, and takes a slot for each of them.
async fn items(Extension(slots): Extension<ResponseSlots>) -> axum::body::Body {
    axum::body::Body::new(ItemsBody {
        slots: Arc::new(slots),
        items_left: 2,
        next_slot: None,
    })
}

/// A body that takes a slot before each item it sends, and puts the slot on
/// the chunk of the item.
struct ItemsBody {
    slots: Arc<ResponseSlots>,
    items_left: usize,
    next_slot: Option<Pin<Box<dyn Future<Output = ResponseSlot> + Send>>>,
}

impl http_body::Body for ItemsBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, Infallible>>> {
        let this = self.get_mut();
        if this.items_left == 0 {
            return Poll::Ready(None);
        }
        let next_slot = this.next_slot.get_or_insert_with(|| {
            let slots = this.slots.clone();
            Box::pin(async move { slots.take().await })
        });
        let slot = std::task::ready!(next_slot.as_mut().poll(cx));
        this.next_slot = None;
        this.items_left -= 1;
        Poll::Ready(Some(Ok(http_body::Frame::data(chunk_with_slot(
            Bytes::from(vec![7u8; ITEM]),
            Arc::new(slot),
        )))))
    }
}

/// An h2 client connection whose responses stall once the client stops
/// reading them.
async fn stalling_client(address: SocketAddr) -> h2::client::SendRequest<Bytes> {
    let tcp = tokio::net::TcpStream::connect(address).await.unwrap();
    let (send_request, connection) = h2::client::Builder::new()
        .initial_window_size(CLIENT_STREAM_WINDOW)
        .initial_connection_window_size(CLIENT_CONNECTION_WINDOW)
        .handshake::<_, Bytes>(tcp)
        .await
        .unwrap();
    tokio::spawn(connection);
    send_request.ready().await.unwrap()
}

/// Sends a GET request of `path`.
fn send_get(
    send_request: &mut h2::client::SendRequest<Bytes>,
    path: &str,
) -> h2::client::ResponseFuture {
    let request = http::Request::builder()
        .uri(format!("http://localhost{path}"))
        .body(())
        .unwrap();
    send_request.send_request(request, true).unwrap().0
}

/// Reads the whole body of `response`, and returns its length. Fails if the
/// body does not end in time.
async fn read_body(response: http::Response<h2::RecvStream>) -> usize {
    assert_eq!(response.status(), http::StatusCode::OK);
    let mut body = response.into_body();
    tokio::time::timeout(WAIT, async {
        let mut read = 0;
        while let Some(data) = body.data().await {
            let data = data.unwrap();
            read += data.len();
            body.flow_control().release_capacity(data.len()).unwrap();
        }
        read
    })
    .await
    .expect("the body must end")
}

/// Waits until `condition` holds, or fails with `what`.
async fn wait_until(condition: impl Fn() -> bool, what: &str) {
    tokio::time::timeout(WAIT, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}"));
}

/// The count and sum of a histogram of the listener.
fn totals(metrics: &MetricsReader, name: &str) -> (u64, f64) {
    let totals = metrics.histogram_totals(name, &[]);
    (totals.count, totals.sum)
}

/// A request over the limit waits on the server, before its handler runs,
/// until the response before it has been read in full by the client: not
/// until its handler returned, and not until its body was handed to hyper.
/// It is then served, not refused.
#[tokio::test]
async fn a_request_over_the_limit_waits_until_the_response_before_it_has_been_read() {
    let (handle, metrics, served) = serve(Some(1), |config| config);
    let mut client = stalling_client(*handle.local_addr()).await;

    let first = send_get(&mut client, "/big");
    let first = tokio::time::timeout(WAIT, first).await.unwrap().unwrap();
    assert_eq!(served.count(), 1);

    let mut second = send_get(&mut client, "/big");
    wait_until(
        || totals(&metrics, "responses_waiting_per_connection") == (1, 1.0),
        "the second request must wait",
    )
    .await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(
        served.count(),
        1,
        "the handler of the second request must not run while the first response is unread"
    );
    assert!(
        tokio::time::timeout(SETTLE, &mut second).await.is_err(),
        "the second request must not be answered while the first response is unread"
    );

    assert_eq!(read_body(first).await, BIG);
    let second = tokio::time::timeout(WAIT, second)
        .await
        .expect("the second request must be served once the first response is read")
        .unwrap();
    assert_eq!(read_body(second).await, BIG);
    assert_eq!(served.count(), 2);
    assert_eq!(
        totals(&metrics, "responses_in_progress_per_connection"),
        (2, 2.0),
        "each response found the connection without another in progress"
    );

    handle.shutdown().await;
}

/// With a limit of 0 nothing waits, and the responses are still counted:
/// three stalled responses observe 1, 2 and 3 in progress.
#[tokio::test]
async fn with_a_limit_of_zero_nothing_waits_and_the_responses_are_counted() {
    let (handle, metrics, served) = serve(Some(0), |config| config);
    let mut client = stalling_client(*handle.local_addr()).await;

    let mut responses = Vec::new();
    for _ in 0..3 {
        let response = send_get(&mut client, "/big");
        responses.push(tokio::time::timeout(WAIT, response).await.unwrap().unwrap());
    }
    assert_eq!(served.count(), 3);
    assert_eq!(
        totals(&metrics, "responses_in_progress_per_connection"),
        (3, 6.0)
    );
    assert_eq!(
        totals(&metrics, "responses_waiting_per_connection"),
        (0, 0.0)
    );

    drop(responses);
    handle.shutdown().await;
}

/// A request without the limit gets no slots.
#[tokio::test]
async fn without_the_limit_a_request_gets_no_slots() {
    for (limit, expected) in [(None, "none"), (Some(0), "some"), (Some(1), "some")] {
        let (handle, _, _) = serve(limit, |config| config);
        let mut client = stalling_client(*handle.local_addr()).await;
        let response = send_get(&mut client, "/slots").await.unwrap();
        let mut body = response.into_body();
        let data = body.data().await.unwrap().unwrap();
        assert_eq!(data, expected, "limit {limit:?}");
        handle.shutdown().await;
    }
}

/// A route that takes slots per item takes none for its whole response: its
/// handler runs while another response holds a slot, and its items take
/// their slots in turn. With a limit of 2 and one slot held, the two items
/// observe 2 in progress each: the second item waits until the chunk of the
/// first has been read.
#[tokio::test]
async fn a_route_that_takes_slots_per_item_takes_none_for_its_whole_response() {
    let (handle, metrics, _) = serve(Some(2), |config| config);
    let mut client = stalling_client(*handle.local_addr()).await;

    let holder = send_get(&mut client, "/big").await.unwrap();
    let items = tokio::time::timeout(WAIT, send_get(&mut client, "/items"))
        .await
        .expect("the handler of the items route must run next to the held slot")
        .unwrap();
    assert_eq!(read_body(items).await, 2 * ITEM);
    assert_eq!(
        totals(&metrics, "responses_in_progress_per_connection"),
        (3, 5.0),
        "the held slot, then one slot per item, each next to the held one"
    );

    drop(holder);
    handle.shutdown().await;
}

/// A waiting request that its client cancels just before a slot frees does
/// not take that slot. The client resets the holder and then the waiting
/// request in one write. The server reads both resets before the tasks of
/// the two streams run. The task of the holder runs first, frees the slot
/// and gives it to the waiting request, whose task then finds both its slot
/// and its reset when it runs.
#[tokio::test]
async fn a_request_cancelled_just_before_a_slot_frees_does_not_take_it() {
    const ROUNDS: usize = 20;
    let (handle, metrics, served) = serve(Some(1), |config| config);
    let mut client = stalling_client(*handle.local_addr()).await;

    let mut holder = send_get(&mut client, "/big").await.unwrap();
    for round in 0..ROUNDS {
        let waits_before = totals(&metrics, "responses_waiting_per_connection").0;
        let cancelled = send_get(&mut client, "/big");
        wait_until(
            || totals(&metrics, "responses_waiting_per_connection").0 == waits_before + 1,
            "the request that is cancelled waits",
        )
        .await;
        drop(holder);
        drop(cancelled);
        holder = tokio::time::timeout(WAIT, send_get(&mut client, "/big"))
            .await
            .expect("the next holder starts")
            .unwrap();
        assert_eq!(
            served.count(),
            round + 2,
            "round {round}: the cancelled request must not run its handler"
        );
    }

    drop(holder);
    handle.shutdown().await;
}

/// What the server announces on a new connection: its stream limit, its
/// stream window, and the increment of the connection window in the first
/// `WINDOW_UPDATE`, if it sends one within a second of its settings.
#[derive(Debug, PartialEq)]
struct Announced {
    max_streams: Option<u32>,
    stream_window: Option<u32>,
    connection_window_increment: Option<u32>,
}

async fn announced_settings(address: SocketAddr) -> Announced {
    // The client connection preface: the fixed string and an empty SETTINGS
    // frame (RFC 9113, section 3.4). Frame types and settings identifiers
    // from sections 6.5 and 6.9.
    const HTTP2_CLIENT_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n\0\0\0\x04\0\0\0\0\0";
    const SETTINGS_FRAME: u8 = 0x4;
    const WINDOW_UPDATE_FRAME: u8 = 0x8;
    const SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
    const SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;

    let mut connection = tokio::net::TcpStream::connect(address).await.unwrap();
    connection.write_all(HTTP2_CLIENT_PREFACE).await.unwrap();
    let mut announced = Announced {
        max_streams: None,
        stream_window: None,
        connection_window_increment: None,
    };
    let mut settings_seen = false;
    loop {
        let read_frame = async {
            let mut header = [0u8; 9];
            connection.read_exact(&mut header).await.unwrap();
            let length = u32::from_be_bytes([0, header[0], header[1], header[2]]) as usize;
            let mut payload = vec![0; length];
            connection.read_exact(&mut payload).await.unwrap();
            (header, payload)
        };
        let deadline = if settings_seen {
            Duration::from_secs(1)
        } else {
            WAIT
        };
        let Ok((header, payload)) = tokio::time::timeout(deadline, read_frame).await else {
            assert!(settings_seen, "the server must send its SETTINGS");
            break;
        };
        let stream_id = u32::from_be_bytes([header[5], header[6], header[7], header[8]]);
        match header[3] {
            SETTINGS_FRAME if header[4] & 0x1 == 0 => {
                settings_seen = true;
                for setting in payload.as_chunks::<6>().0 {
                    let value =
                        u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
                    match u16::from_be_bytes([setting[0], setting[1]]) {
                        SETTINGS_MAX_CONCURRENT_STREAMS => announced.max_streams = Some(value),
                        SETTINGS_INITIAL_WINDOW_SIZE => announced.stream_window = Some(value),
                        _ => {}
                    }
                }
            }
            WINDOW_UPDATE_FRAME if stream_id == 0 => {
                announced.connection_window_increment = Some(
                    u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]])
                        & 0x7fff_ffff,
                );
                break;
            }
            _ => {}
        }
    }
    announced
}

/// With the limit on, the server announces its stream limit, hyper's default
/// of 200 when none is set, and opens the connection window to one stream
/// window for each stream. Without the limit, it announces what is
/// configured: no stream limit, and hyper's connection window of 1 MiB.
#[tokio::test]
async fn with_the_limit_the_connection_window_holds_a_window_for_each_stream() {
    // The window of a connection before any `WINDOW_UPDATE` (RFC 9113).
    const DEFAULT_CONNECTION_WINDOW: u32 = 65_535;
    const STREAM_WINDOW: u32 = 64 * 1024;

    let (handle, _, _) = serve(Some(1), |config| config);
    assert_eq!(
        announced_settings(*handle.local_addr()).await,
        Announced {
            max_streams: Some(200),
            stream_window: Some(1024 * 1024),
            connection_window_increment: Some(200 * 1024 * 1024 - DEFAULT_CONNECTION_WINDOW),
        }
    );
    handle.shutdown().await;

    let (handle, _, _) = serve(Some(0), |config| {
        config
            .max_concurrent_streams(Some(4))
            .initial_stream_window_size(Some(STREAM_WINDOW))
    });
    assert_eq!(
        announced_settings(*handle.local_addr()).await,
        Announced {
            max_streams: Some(4),
            stream_window: Some(STREAM_WINDOW),
            connection_window_increment: Some(4 * STREAM_WINDOW - DEFAULT_CONNECTION_WINDOW),
        }
    );
    handle.shutdown().await;

    let (handle, _, _) = serve(None, |config| config);
    assert_eq!(
        announced_settings(*handle.local_addr()).await,
        Announced {
            max_streams: None,
            stream_window: Some(1024 * 1024),
            connection_window_increment: Some(1024 * 1024 - DEFAULT_CONNECTION_WINDOW),
        }
    );
    handle.shutdown().await;
}

/// With a limit of 1 and 4 streams, one request is served and still
/// receives its body while the other 3 streams of the connection wait, each
/// with its full stream window of unread body: the connection window holds
/// the windows of all streams. The client sends the rest of the served
/// request only after it has assigned the full stream window to each
/// waiting request, so a connection window smaller than 4 stream windows
/// leaves nothing for the served request, and it hangs.
#[tokio::test]
async fn a_served_request_receives_its_body_while_waiting_requests_fill_their_windows() {
    const STREAMS: u32 = 4;
    const STREAM_WINDOW: usize = 1024 * 1024;
    const BODY: usize = 2 * STREAM_WINDOW;
    const FIRST_PART: usize = 512 * 1024;
    let (handle, metrics, _) = serve(Some(1), |config| {
        config.max_concurrent_streams(Some(STREAMS))
    });
    // h2's client lets a stream use at most its send buffer of the window,
    // so that buffer must hold a full stream window.
    let tcp = tokio::net::TcpStream::connect(handle.local_addr())
        .await
        .unwrap();
    let (send_request, connection) = h2::client::Builder::new()
        .max_send_buffer_size(2 * STREAM_WINDOW)
        .handshake::<_, Bytes>(tcp)
        .await
        .unwrap();
    tokio::spawn(connection);
    let mut send_request = send_request.ready().await.unwrap();
    let post = || {
        http::Request::builder()
            .method("POST")
            .uri("http://localhost/echo")
            .body(())
            .unwrap()
    };

    let body = Bytes::from(vec![7u8; BODY]);
    let (served, mut served_stream) = send_request.send_request(post(), false).unwrap();
    served_stream
        .send_data(body.slice(..FIRST_PART), false)
        .unwrap();
    wait_until(
        || totals(&metrics, "responses_in_progress_per_connection").0 == 1,
        "the first request must be served",
    )
    .await;

    let mut waiting = Vec::new();
    for _ in 1..STREAMS {
        send_request = send_request.ready().await.unwrap();
        let (response, mut stream) = send_request.send_request(post(), false).unwrap();
        stream.reserve_capacity(STREAM_WINDOW);
        waiting.push((response, stream));
    }
    wait_until(
        || totals(&metrics, "responses_waiting_per_connection").0 == u64::from(STREAMS - 1),
        "the other requests must wait",
    )
    .await;
    for (i, (_, stream)) in waiting.iter_mut().enumerate() {
        tokio::time::timeout(
            WAIT,
            std::future::poll_fn(|cx| {
                while stream.capacity() < STREAM_WINDOW {
                    match stream.poll_capacity(cx) {
                        Poll::Ready(Some(Ok(_))) => {}
                        Poll::Ready(other) => panic!("stream {i}: {other:?}"),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                Poll::Ready(())
            }),
        )
        .await
        .unwrap_or_else(|_| panic!("waiting request {i} must get its full stream window"));
        stream
            .send_data(Bytes::from(vec![0u8; STREAM_WINDOW]), false)
            .unwrap();
    }

    served_stream
        .send_data(body.slice(FIRST_PART..), true)
        .unwrap();
    let response = tokio::time::timeout(WAIT, served)
        .await
        .expect("the served request must receive the rest of its body and be answered")
        .unwrap();
    let mut body = response.into_body();
    let data = body.data().await.unwrap().unwrap();
    assert_eq!(data, BODY.to_string());

    drop(waiting);
    handle.shutdown().await;
}

/// A limit that the HTTP/2 settings do not allow is rejected when the
/// server starts: a limit above the stream limit, a connection window too
/// small for all streams, a window product over the HTTP/2 maximum, and an
/// adaptive window.
#[tokio::test]
async fn a_limit_the_http2_settings_do_not_allow_is_rejected() {
    let start = |config: Config| {
        Builder::new()
            .config(config)
            .serve(("localhost", 0), Router::new())
    };
    let with_limit = |limit| Config::default().concurrency_limit_per_connection(Some(limit));

    assert!(
        start(with_limit(200)).is_ok(),
        "the stream limit itself is allowed"
    );
    assert!(
        start(with_limit(201)).is_err(),
        "a limit above hyper's stream limit of 200 must be rejected"
    );
    assert!(
        start(with_limit(5).max_concurrent_streams(Some(4))).is_err(),
        "a limit above the configured stream limit must be rejected"
    );
    assert!(
        start(with_limit(4).max_concurrent_streams(Some(4))).is_ok(),
        "a limit at the configured stream limit is allowed"
    );
    assert!(
        start(with_limit(1).initial_connection_window_size(Some(200 * 1024 * 1024 - 1))).is_err(),
        "a connection window below one stream window per stream must be rejected"
    );
    assert!(
        start(with_limit(1).initial_connection_window_size(Some(200 * 1024 * 1024))).is_ok(),
        "a connection window of one stream window per stream is allowed"
    );
    assert!(
        start(with_limit(1).max_concurrent_streams(Some(2048))).is_err(),
        "2048 streams of 1 MiB are over the largest HTTP/2 window"
    );
    assert!(
        start(with_limit(1).max_concurrent_streams(Some(2047))).is_ok(),
        "2047 streams of 1 MiB fit the largest HTTP/2 window"
    );
    assert!(
        start(with_limit(1).http2_adaptive_window(Some(true))).is_err(),
        "an adaptive window must be rejected with the limit"
    );
    assert!(
        start(
            Config::default()
                .concurrency_limit_per_connection(None)
                .max_concurrent_streams(Some(4))
                .initial_connection_window_size(Some(1024))
                .http2_adaptive_window(Some(true))
        )
        .is_ok(),
        "without the limit the settings are free"
    );
}
