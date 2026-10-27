// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The gRPC server limits the responses in progress on each connection to
//! `concurrency-limit-per-connection`. A response counts from before its
//! handler runs until its last byte has been written to the connection.
//! Further calls wait on the node, in arrival order. A `StreamCheckpoints`
//! subscription does not count.

mod common;

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bytes::Bytes;
use common::{MockGrpcStateReader, PendingExecutor, create_large_object, raw_h2};
use iota_config::node::GrpcApiConfig;
use iota_grpc_server::{GrpcReader, GrpcServerHandle, GrpcServerMetrics, start_grpc_server};
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::{
        ledger_service::{
            CheckpointData as ProtoCheckpointData, GetObjectsRequest, GetObjectsResponse,
            ObjectRequest, ObjectRequests, StreamCheckpointsRequest, checkpoint_data::Payload,
            ledger_service_client::LedgerServiceClient,
        },
        transaction_execution_service::transaction_execution_service_client::TransactionExecutionServiceClient,
        types::ObjectReference,
    },
};
use iota_metrics::test_utils::MetricsReader;
use iota_sdk_types::ObjectId;
use prometheus_filtered::Registry;
use prost::Message;
use tokio::{task::JoinHandle, time::timeout};
use tonic::{Streaming, transport::Channel};

/// The HTTP/2 stream window of a client that stalls its responses: a
/// response of the large object does not fit, so the server cannot complete
/// it until the client reads.
const CLIENT_STREAM_WINDOW_SIZE: u32 = 64 * 1024;

/// The connection window of such a client, large enough that a stalled
/// stream leaves room for the others.
const CLIENT_CONNECTION_WINDOW_SIZE: u32 = 16 * 1024 * 1024;

/// Padding large enough that a single response exceeds
/// [`CLIENT_STREAM_WINDOW_SIZE`].
const LARGE_OBJECT_PADDING: usize = 200_000;

/// How long a test waits for something that must happen.
const WAIT: Duration = Duration::from_secs(10);

/// How long a test waits to see that something does not happen.
const SETTLE: Duration = Duration::from_secs(1);

const GET_OBJECTS_PATH: &str = "/iota.grpc.v1.ledger_service.LedgerService/GetObjects";

/// The prefix of the metrics of the gRPC server.
const METRICS_PREFIX: &str = "node_grpc";

/// The stream limit that the server announces.
const MAX_CONCURRENT_STREAMS: u32 = GrpcApiConfig::MAX_CONCURRENT_STREAMS_PER_CONNECTION;

/// The stream window that the server announces: hyper's default.
const STREAM_WINDOW: u32 = 1024 * 1024;

fn get_objects_request(object_id: ObjectId) -> GetObjectsRequest {
    GetObjectsRequest::default()
        .with_requests(ObjectRequests::default().with_requests(vec![
            ObjectRequest::default().with_object_ref(
                ObjectReference::default().with_object_id(common::object_id_proto(object_id)),
            ),
        ]))
        .with_read_mask(prost_types::FieldMask::from_str("reference,bcs"))
}

/// A reader with one large object, and the checkpoints `checkpoints` in
/// storage.
fn reader_with_a_large_object(
    checkpoints: impl Iterator<Item = u64>,
) -> (Arc<MockGrpcStateReader>, ObjectId) {
    let (id, object) = create_large_object(LARGE_OBJECT_PADDING);
    let mut reader = MockGrpcStateReader::new_from_iter(checkpoints);
    reader.objects = HashMap::from([(id, object)]);
    (Arc::new(reader), id)
}

/// Starts a server with the given limit, and returns its handle and a reader
/// of its metrics.
async fn start_server(
    reader: Arc<MockGrpcStateReader>,
    limit: u32,
) -> (GrpcServerHandle, MetricsReader) {
    start_server_with_executor(reader, None, limit).await
}

async fn start_server_with_executor(
    reader: Arc<MockGrpcStateReader>,
    executor: Option<Arc<dyn iota_types::transaction_executor::TransactionExecutor>>,
    limit: u32,
) -> (GrpcServerHandle, MetricsReader) {
    let registry = Registry::new();
    let metrics = GrpcServerMetrics::new(&registry);
    let (handle, _) =
        common::start_test_server_with(reader, executor, None, None, Some(metrics), |config| {
            config.concurrency_limit_per_connection = limit
        })
        .await;
    (
        handle,
        MetricsReader::new(&registry).with_prefix(METRICS_PREFIX),
    )
}

/// A channel whose responses stall once the client stops reading them.
async fn stalling_channel(address: SocketAddr) -> Channel {
    Channel::from_shared(format!("http://{address}"))
        .unwrap()
        .initial_stream_window_size(CLIENT_STREAM_WINDOW_SIZE)
        .initial_connection_window_size(CLIENT_CONNECTION_WINDOW_SIZE)
        .connect()
        .await
        .expect("connect")
}

type ObjectsResponse = tonic::Response<Streaming<GetObjectsResponse>>;

/// Starts a `GetObjects` call of the large object on `channel`. Its response
/// stalls, so the call keeps its slot until the response is dropped.
fn spawn_get_objects(
    channel: &Channel,
    id: ObjectId,
) -> JoinHandle<Result<ObjectsResponse, tonic::Status>> {
    let mut client = LedgerServiceClient::new(channel.clone());
    tokio::spawn(async move { client.get_objects(get_objects_request(id)).await })
}

/// The number of times that `counter` was incremented.
fn reads(counter: &AtomicUsize) -> u64 {
    counter.load(Ordering::SeqCst) as u64
}

/// The count and sum of a histogram of the server.
fn totals(metrics: &MetricsReader, name: &str) -> (u64, f64) {
    let totals = metrics.histogram_totals(name, &[]);
    (totals.count, totals.sum)
}

/// The count of the histogram of calls that wait for a slot.
fn waits(metrics: &MetricsReader) -> u64 {
    totals(metrics, "responses_waiting_per_connection").0
}

/// Waits until `value()` is `expected`, or fails with `what`.
async fn wait_until(value: impl Fn() -> u64, expected: u64, what: &str) {
    timeout(WAIT, async {
        while value() != expected {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("expected {expected} {what}, got {}", value()));
}

/// More calls than the limit on one connection: the node serves 2 at once,
/// and the others wait on the node, not in the client, before their
/// handlers run. When one response is dropped, exactly one more call starts.
#[tokio::test]
async fn extra_calls_wait_on_the_node_and_start_one_at_a_time() {
    let (reader, id) = reader_with_a_large_object(std::iter::empty());
    let (handle, metrics) = start_server(reader.clone(), 2).await;
    let channel = stalling_channel(handle.address()).await;

    let mut calls: Vec<_> = (0..5).map(|_| spawn_get_objects(&channel, id)).collect();
    wait_until(|| reads(&reader.object_reads), 2, "handlers that ran").await;
    wait_until(|| waits(&metrics), 3, "calls that wait on the node").await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(reads(&reader.object_reads), 2);
    assert_eq!(calls.iter().filter(|call| call.is_finished()).count(), 2);

    let first = calls
        .iter()
        .position(|call| call.is_finished())
        .expect("a served call");
    drop(calls.remove(first).await.unwrap().unwrap());
    wait_until(|| reads(&reader.object_reads), 3, "handlers that ran").await;
    tokio::time::sleep(SETTLE).await;
    assert_eq!(
        reads(&reader.object_reads),
        3,
        "dropping one response must let exactly one more call start"
    );

    for call in calls {
        call.abort();
    }
    drop(channel);
    handle.shutdown().await.expect("shutdown");
}

/// Opens `calls` stalled `GetObjects` calls on one connection, and waits
/// until all of them have their response headers.
async fn stalled_calls_on_one_connection(
    handle: &GrpcServerHandle,
    id: ObjectId,
    calls: usize,
) -> Vec<ObjectsResponse> {
    let channel = stalling_channel(handle.address()).await;
    let mut responses = Vec::new();
    for i in 0..calls {
        let mut client = LedgerServiceClient::new(channel.clone());
        responses.push(
            timeout(WAIT, client.get_objects(get_objects_request(id)))
                .await
                .unwrap_or_else(|_| panic!("call {i} must start"))
                .unwrap_or_else(|e| panic!("call {i} should start: {e:?}")),
        );
    }
    responses
}

/// Each call that starts observes the responses in progress on its
/// connection, with the new one: three calls observe 1, 2 and 3. The same
/// holds with the limit off, where no call waits.
#[tokio::test]
async fn the_responses_on_a_connection_are_observed_with_the_limit_on_and_off() {
    for limit in [16, 0] {
        let (reader, id) = reader_with_a_large_object(std::iter::empty());
        let (handle, metrics) = start_server(reader, limit).await;

        let responses = stalled_calls_on_one_connection(&handle, id, 3).await;

        assert_eq!(
            totals(&metrics, "responses_in_progress_per_connection"),
            (3, 6.0),
            "limit {limit}"
        );
        assert_eq!(
            totals(&metrics, "responses_waiting_per_connection"),
            (0, 0.0),
            "limit {limit}"
        );

        drop(responses);
        handle.shutdown().await.expect("shutdown");
    }
}

/// With the limit off, no call waits: every handler runs at once.
#[tokio::test]
async fn with_the_limit_off_every_call_runs_at_once() {
    let (reader, id) = reader_with_a_large_object(std::iter::empty());
    let (handle, _) = start_server(reader.clone(), 0).await;

    let responses = stalled_calls_on_one_connection(&handle, id, 20).await;
    assert_eq!(reads(&reader.object_reads), 20);

    drop(responses);
    handle.shutdown().await.expect("shutdown");
}

// ---------------------------------------------------------------------------
// StreamCheckpoints
// ---------------------------------------------------------------------------

/// Starts a `StreamCheckpoints` call on `channel`, or fails if it gets no
/// answer.
async fn subscribe(channel: &Channel, start: u64) -> Streaming<ProtoCheckpointData> {
    timeout(
        WAIT,
        LedgerServiceClient::new(channel.clone()).stream_checkpoints(
            StreamCheckpointsRequest::default().with_start_sequence_number(start),
        ),
    )
    .await
    .expect("the subscription must get an answer")
    .expect("subscribe")
    .into_inner()
}

/// Reads messages until the end marker of checkpoint `sequence_number`.
async fn read_through_checkpoint(
    stream: &mut Streaming<ProtoCheckpointData>,
    sequence_number: u64,
) {
    timeout(WAIT, async {
        loop {
            let message = stream
                .message()
                .await
                .expect("the stream must not fail")
                .expect("the stream must not end");
            if let Some(Payload::EndMarker(end)) = &message.payload {
                if end.sequence_number == Some(sequence_number) {
                    return;
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("checkpoint {sequence_number} must arrive"));
}

/// Subscriptions that have sent their checkpoint and wait for the next one
/// hold no slot: with a limit of 2 and 4 idle subscriptions on the same
/// connection, two stalled calls both run their handlers, and the
/// subscriptions are not counted as responses in progress.
#[tokio::test]
async fn idle_subscriptions_do_not_block_other_calls() {
    let (reader, id) = reader_with_a_large_object(0..=0);
    let (handle, metrics) = start_server(reader.clone(), 2).await;
    let channel = stalling_channel(handle.address()).await;

    let mut subscriptions = Vec::new();
    for _ in 0..4 {
        let mut stream = subscribe(&channel, 0).await;
        read_through_checkpoint(&mut stream, 0).await;
        subscriptions.push(stream);
    }
    assert_eq!(
        totals(&metrics, "responses_in_progress_per_connection"),
        (0, 0.0),
        "a subscription takes no slot for its whole response"
    );

    let calls: Vec<_> = (0..2).map(|_| spawn_get_objects(&channel, id)).collect();
    wait_until(
        || reads(&reader.object_reads),
        2,
        "handlers that ran next to idle subscriptions",
    )
    .await;
    assert_eq!(
        totals(&metrics, "responses_in_progress_per_connection"),
        (2, 3.0)
    );

    for call in calls {
        call.abort();
    }
    drop(subscriptions);
    drop(channel);
    handle.shutdown().await.expect("shutdown");
}

// ---------------------------------------------------------------------------
// Waits that end early
// ---------------------------------------------------------------------------

/// A call that its client cancels while its handler runs gives its slot
/// back: the call that waits behind it starts.
#[tokio::test]
async fn a_call_cancelled_while_its_handler_runs_gives_its_slot_back() {
    let (state_reader, id) = reader_with_a_large_object(std::iter::empty());
    let executor = Arc::new(PendingExecutor::default());
    let (handle, metrics) =
        start_server_with_executor(state_reader.clone(), Some(executor.clone()), 1).await;
    let channel = stalling_channel(handle.address()).await;

    let mut execution = TransactionExecutionServiceClient::new(channel.clone());
    let stuck = tokio::spawn(async move {
        execution
            .execute_transactions(common::execute_transactions_request())
            .await
    });
    timeout(WAIT, executor.started.notified())
        .await
        .expect("the handler that holds the slot must start");
    let waiting = spawn_get_objects(&channel, id);
    wait_until(|| waits(&metrics), 1, "calls that wait behind the handler").await;
    assert_eq!(reads(&state_reader.object_reads), 0);

    stuck.abort();
    let started = timeout(WAIT, waiting)
        .await
        .expect("the waiting call starts once the cancelled call is gone")
        .unwrap()
        .unwrap();
    assert_eq!(reads(&state_reader.object_reads), 1);

    drop(started);
    drop(channel);
    handle.shutdown().await.expect("shutdown");
}

/// An h2 client connection whose responses stall once the client stops
/// reading them. h2's own client sets no timer for `grpc-timeout`, unlike
/// tonic's, so a test sees what the server does at the deadline.
async fn h2_stalling_client(address: SocketAddr) -> h2::client::SendRequest<Bytes> {
    let tcp = tokio::net::TcpStream::connect(address).await.unwrap();
    let (send_request, connection) = h2::client::Builder::new()
        .initial_window_size(CLIENT_STREAM_WINDOW_SIZE)
        .initial_connection_window_size(CLIENT_CONNECTION_WINDOW_SIZE)
        .handshake::<_, Bytes>(tcp)
        .await
        .unwrap();
    tokio::spawn(connection);
    send_request.ready().await.unwrap()
}

/// Sends a `GetObjects` call of the large object, with the extra header
/// fields `fields`.
fn h2_get_objects(
    send_request: &mut h2::client::SendRequest<Bytes>,
    id: ObjectId,
    fields: &[(&str, &str)],
) -> h2::client::ResponseFuture {
    let mut request = http::Request::builder()
        .method("POST")
        .uri(format!("http://localhost{GET_OBJECTS_PATH}"))
        .header("content-type", "application/grpc");
    for (name, value) in fields {
        request = request.header(*name, *value);
    }
    let (response, mut stream) = send_request
        .send_request(request.body(()).unwrap(), false)
        .unwrap();
    stream
        .send_data(
            Bytes::from(raw_h2::grpc_message(
                &get_objects_request(id).encode_to_vec(),
            )),
            true,
        )
        .unwrap();
    response
}

/// A call whose `grpc-timeout` expires while it waits gets
/// `DEADLINE_EXCEEDED` from the server, runs no handler, and is not counted
/// as in progress. The client is h2's own, so the status comes from the
/// server's deadline, not from a timer of the client.
#[tokio::test]
async fn the_server_ends_a_call_whose_deadline_expires_while_it_waits() {
    const ROUNDS: usize = 3;
    let (reader, id) = reader_with_a_large_object(std::iter::empty());
    let (handle, metrics) = start_server(reader.clone(), 1).await;
    let mut send_request = h2_stalling_client(handle.address()).await;

    for round in 0..ROUNDS {
        let holder = timeout(WAIT, h2_get_objects(&mut send_request, id, &[]))
            .await
            .expect("the holder starts")
            .unwrap();
        wait_until(
            || reads(&reader.object_reads),
            round as u64 + 1,
            "the call that holds the slot",
        )
        .await;

        let response = timeout(
            WAIT,
            h2_get_objects(&mut send_request, id, &[("grpc-timeout", "300m")]),
        )
        .await
        .expect("the server must end the call at its deadline")
        .unwrap();
        assert_eq!(
            response
                .headers()
                .get("grpc-status")
                .map(|v| v.to_str().unwrap()),
            Some(&*(tonic::Code::DeadlineExceeded as i32).to_string()),
            "round {round}: {:?}",
            response.headers()
        );
        assert_eq!(
            reads(&reader.object_reads),
            round as u64 + 1,
            "round {round}: the call past its deadline must run no handler"
        );

        drop(holder);
    }
    let last = timeout(WAIT, h2_get_objects(&mut send_request, id, &[]))
        .await
        .expect("the last call starts")
        .unwrap();
    assert_eq!(
        totals(&metrics, "responses_in_progress_per_connection"),
        (ROUNDS as u64 + 1, ROUNDS as f64 + 1.0),
        "only the calls that ran were in progress, each alone"
    );

    drop(last);
    handle.shutdown().await.expect("shutdown");
}

// ---------------------------------------------------------------------------
// A burst of calls, and the HTTP/2 settings
// ---------------------------------------------------------------------------

/// A client that sends 100 calls on a new connection before it reads anything
/// of the server, also not its settings, gets all 100 answered with OK: the
/// server refuses none of them, and serves the calls over its concurrency
/// limit in turn. The client is h2's own, which keeps no call back: before
/// it has the server's settings, it allows any number of streams. Its reads
/// stay closed until the server shows that all calls have arrived: 16 are
/// served, and 84 wait.
#[tokio::test]
async fn a_burst_of_calls_before_the_server_settings_is_answered_in_full() {
    const CALLS: usize = 100;
    let (reader, id) = reader_with_a_large_object(std::iter::empty());
    let (handle, metrics) = start_server(
        reader,
        GrpcApiConfig::default().concurrency_limit_per_connection,
    )
    .await;
    let (io, gate) = raw_h2::GatedIo::connect(handle.address()).await;
    // A small stream window stalls the responses of the served calls, so
    // that they keep their slots until the client reads.
    let (send_request, connection) = h2::client::Builder::new()
        .initial_window_size(CLIENT_STREAM_WINDOW_SIZE)
        .initial_connection_window_size(CLIENT_CONNECTION_WINDOW_SIZE)
        .handshake::<_, Bytes>(io)
        .await
        .unwrap();
    tokio::spawn(connection);

    let mut send_request = send_request.ready().await.unwrap();
    let mut responses = Vec::new();
    for _ in 0..CALLS {
        responses.push(h2_get_objects(&mut send_request, id, &[]));
    }
    wait_until(
        || waits(&metrics),
        (CALLS - 16) as u64,
        "calls that reached the node and wait there",
    )
    .await;
    gate.open();

    let mut refused = 0;
    let mut ok = 0;
    for response in responses {
        match timeout(WAIT, response)
            .await
            .expect("every call must be answered")
        {
            Ok(response) => {
                let mut body = response.into_body();
                while let Some(data) = body.data().await {
                    let data = data.unwrap();
                    body.flow_control().release_capacity(data.len()).unwrap();
                }
                let trailers = body.trailers().await.unwrap().expect("trailers");
                if trailers
                    .get("grpc-status")
                    .is_some_and(|status| status == "0")
                {
                    ok += 1;
                }
            }
            Err(error) if error.reason() == Some(h2::Reason::REFUSED_STREAM) => refused += 1,
            Err(error) => panic!("unexpected error: {error:?}"),
        }
    }
    assert_eq!(refused, 0, "no call may be refused");
    assert_eq!(ok, CALLS, "every call must be answered with OK");

    handle.shutdown().await.expect("shutdown");
}

/// The server announces its stream limit of 100 and a stream window of
/// 1 MiB, and opens the connection window to 100 such windows, so that calls
/// that wait for their turn cannot take the window of a call that is served.
/// The same holds with `concurrency-limit-per-connection` at 0.
#[tokio::test]
async fn the_server_announces_its_stream_limit_and_windows_with_any_concurrency_limit() {
    // The window of a connection before any `WINDOW_UPDATE` (RFC 9113).
    const DEFAULT_CONNECTION_WINDOW: u32 = 65_535;
    for limit in [16, 0] {
        let (handle, _) = start_server(Arc::new(MockGrpcStateReader::default()), limit).await;
        let mut connection = raw_h2::Connection::open(handle.address()).await;

        let mut settings = None;
        let mut increment = None;
        while settings.is_none() || increment.is_none() {
            let frame = connection
                .reader
                .next()
                .await
                .expect("the server must send its SETTINGS and a WINDOW_UPDATE");
            match frame.kind {
                raw_h2::SETTINGS if !frame.is_ack() => settings = Some(frame),
                raw_h2::WINDOW_UPDATE if frame.stream_id == 0 => {
                    increment.get_or_insert(frame.window_increment());
                }
                _ => {}
            }
        }
        let settings = settings.unwrap();
        assert_eq!(
            settings.setting(raw_h2::SETTINGS_MAX_CONCURRENT_STREAMS),
            Some(MAX_CONCURRENT_STREAMS),
            "limit {limit}"
        );
        assert_eq!(
            settings.setting(raw_h2::SETTINGS_INITIAL_WINDOW_SIZE),
            Some(STREAM_WINDOW),
            "limit {limit}"
        );
        assert_eq!(
            increment.unwrap(),
            MAX_CONCURRENT_STREAMS * STREAM_WINDOW - DEFAULT_CONNECTION_WINDOW,
            "limit {limit}"
        );

        drop(connection);
        handle.shutdown().await.expect("shutdown");
    }
}

/// A `concurrency-limit-per-connection` above the stream limit of the
/// server could never be reached, so the server does not start with it.
#[tokio::test]
async fn a_limit_above_the_stream_limit_is_rejected_at_startup() {
    for (limit, starts) in [
        (MAX_CONCURRENT_STREAMS, true),
        (MAX_CONCURRENT_STREAMS + 1, false),
    ] {
        let reader = Arc::new(GrpcReader::new(
            Arc::new(MockGrpcStateReader::default()),
            Some("test".to_string()),
        ));
        let localhost = iota_config::local_ip_utils::localhost_for_testing();
        let port = iota_config::local_ip_utils::get_available_port(&localhost);
        let config = GrpcApiConfig {
            address: format!("{localhost}:{port}").parse().unwrap(),
            concurrency_limit_per_connection: limit,
            ..GrpcApiConfig::default()
        };
        let started = start_grpc_server(
            reader,
            None,
            config,
            tokio_util::sync::CancellationToken::new(),
            iota_types::digests::ChainIdentifier::default(),
            None,
            None,
            None,
        )
        .await;
        match started {
            Ok(handle) => {
                assert!(starts, "limit {limit} must be rejected");
                handle.shutdown().await.expect("shutdown");
            }
            Err(error) => {
                assert!(!starts, "limit {limit} must be accepted: {error}");
                assert!(
                    error
                        .to_string()
                        .contains("`grpc-api-config.concurrency-limit-per-connection`"),
                    "{error}"
                );
            }
        }
    }
}
