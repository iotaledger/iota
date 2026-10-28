// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the HTTP/2 connections that serve the gRPC API.

mod common;

use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use bytes::Bytes;
use common::{
    MockGrpcStateReader, PendingExecutor, create_test_transaction, start_test_server,
    start_test_server_with,
};
use http_body_util::Full;
use iota_grpc_types::v1::{
    bcs::BcsData,
    signatures::UserSignatures,
    transaction::Transaction as ProtoTransaction,
    transaction_execution_service::{
        ExecuteTransactionItem, ExecuteTransactionsRequest,
        transaction_execution_service_client::TransactionExecutionServiceClient,
    },
};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpStream,
};
use tonic::transport::Channel;

/// The server speaks only HTTP/2: an HTTP/1 request gets the SETTINGS frame
/// of an HTTP/2 server, and its connection is closed.
#[tokio::test]
async fn an_http1_request_is_not_served() {
    // The type of an HTTP/2 SETTINGS frame (RFC 9113, section 6.5).
    const HTTP2_SETTINGS_FRAME: u8 = 0x4;

    let (handle, _reader) =
        start_test_server(Arc::new(MockGrpcStateReader::default()), |_| {}).await;

    let mut connection = TcpStream::connect(handle.address()).await.unwrap();
    connection
        .write_all(
            b"POST /iota.grpc.v1.ledger_service.LedgerService/GetServiceInfo HTTP/1.1\r\n\
              host: localhost\r\ncontent-type: application/grpc\r\ncontent-length: 0\r\n\r\n",
        )
        .await
        .unwrap();

    let mut received = Vec::new();
    let closed = tokio::time::timeout(
        Duration::from_secs(5),
        connection.read_to_end(&mut received),
    )
    .await;
    assert_eq!(
        received.get(3),
        Some(&HTTP2_SETTINGS_FRAME),
        "the server must answer as HTTP/2, got {:?}",
        String::from_utf8_lossy(&received)
    );
    assert!(closed.is_ok(), "the server must close the connection");
}

/// After `shutdown` returns, the server accepts no connections.
#[tokio::test]
async fn shutdown_stops_the_server() {
    let (handle, _reader) =
        start_test_server(Arc::new(MockGrpcStateReader::default()), |_| {}).await;
    let address = handle.address();
    TcpStream::connect(address)
        .await
        .expect("the server must accept connections before its shutdown");

    tokio::time::timeout(Duration::from_secs(5), handle.shutdown())
        .await
        .expect("the server must shut down")
        .unwrap();

    assert!(
        TcpStream::connect(address).await.is_err(),
        "the server must not accept connections after its shutdown"
    );
}

/// The server announces only hyper's default window, frame and header list
/// sizes: it sets no limit on concurrent streams and does not offer the
/// extended CONNECT protocol.
#[tokio::test]
async fn the_server_announces_no_stream_limit_and_no_extended_connect() {
    // SETTINGS parameters (RFC 9113, section 6.5.2).
    const SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
    const SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;
    const SETTINGS_MAX_HEADER_LIST_SIZE: u16 = 0x6;

    let (handle, _reader) =
        start_test_server(Arc::new(MockGrpcStateReader::default()), |_| {}).await;

    assert_eq!(
        server_settings(handle.address()).await,
        BTreeMap::from([
            (SETTINGS_INITIAL_WINDOW_SIZE, 1024 * 1024),
            (SETTINGS_MAX_FRAME_SIZE, 16 * 1024),
            (SETTINGS_MAX_HEADER_LIST_SIZE, 16 * 1024),
        ])
    );
}

/// A call whose `grpc-timeout` passes before its handler answers ends with
/// `DEADLINE_EXCEEDED`.
#[tokio::test]
async fn a_call_past_its_grpc_timeout_ends_with_deadline_exceeded() {
    const EXECUTE_TRANSACTIONS_PATH: &str = "/iota.grpc.v1.transaction_execution_service.TransactionExecutionService/ExecuteTransactions";

    let (handle, _reader) = start_test_server_with(
        Arc::new(MockGrpcStateReader::default()),
        Some(Arc::new(PendingExecutor::default())),
        None,
        None,
        None,
        |_| {},
    )
    .await;

    // Unlike tonic's client, hyper's client does not end a call itself when
    // its `grpc-timeout` passes, so the status comes from the server.
    let io = hyper_util::rt::TokioIo::new(TcpStream::connect(handle.address()).await.unwrap());
    let (mut sender, connection) =
        hyper::client::conn::http2::handshake(hyper_util::rt::TokioExecutor::new(), io)
            .await
            .unwrap();
    tokio::spawn(connection);
    let started = tokio::time::Instant::now();
    let call = sender.send_request(
        http::Request::post(format!(
            "http://{}{EXECUTE_TRANSACTIONS_PATH}",
            handle.address()
        ))
        .header(http::header::CONTENT_TYPE, "application/grpc")
        .header(http::header::TE, "trailers")
        .header("grpc-timeout", "200m")
        .body(Full::new(grpc_message(&execute_transactions_request())))
        .unwrap(),
    );
    let response = tokio::time::timeout(Duration::from_secs(10), call)
        .await
        .expect("the server must end the call when its deadline passes")
        .unwrap();
    assert!(
        started.elapsed() >= Duration::from_millis(200),
        "the server must give the call its whole deadline"
    );

    let status = tonic::Status::from_header_map(response.headers());
    assert_eq!(
        status.as_ref().map(tonic::Status::code),
        Some(tonic::Code::DeadlineExceeded),
        "{status:?}"
    );
}

/// On shutdown, a running call and a connection that has sent nothing are
/// closed 1 s after the shutdown starts.
#[tokio::test]
async fn shutdown_closes_a_running_call_after_one_second() {
    let executor = Arc::new(PendingExecutor::default());
    let (handle, _reader) = start_test_server_with(
        Arc::new(MockGrpcStateReader::default()),
        Some(executor.clone()),
        None,
        None,
        None,
        |_| {},
    )
    .await;
    let mut silent_connection = TcpStream::connect(handle.address()).await.unwrap();
    let channel = Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let call = tokio::spawn(async move {
        TransactionExecutionServiceClient::new(channel)
            .execute_transactions(execute_transactions_request())
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), executor.started.notified())
        .await
        .expect("the call must reach the executor");

    let shutdown_started = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(5), handle.shutdown())
        .await
        .expect("the shutdown must not wait for the running call")
        .unwrap();
    assert!(
        shutdown_started.elapsed() >= Duration::from_secs(1),
        "the shutdown must give the running call 1 s"
    );
    assert!(
        call.await.unwrap().is_err(),
        "the running call must be closed"
    );
    let closed = tokio::time::timeout(
        Duration::from_secs(5),
        silent_connection.read_to_end(&mut Vec::new()),
    )
    .await;
    assert!(
        closed.is_ok(),
        "the connection that has sent nothing must be closed"
    );
}

/// An `ExecuteTransactions` request with one transaction that passes the
/// checks of the handler and reaches the executor.
fn execute_transactions_request() -> ExecuteTransactionsRequest {
    let (_digest, transaction, _effects) = create_test_transaction();
    let transaction_bcs =
        bcs::to_bytes(&transaction.data().signed_transaction().transaction).unwrap();
    ExecuteTransactionsRequest::default().with_transactions(vec![
        ExecuteTransactionItem::default()
            .with_transaction(
                ProtoTransaction::default().with_bcs(BcsData::default().with_data(transaction_bcs)),
            )
            .with_signatures(UserSignatures::default()),
    ])
}

/// Reads the SETTINGS the server sends first on a new HTTP/2 connection.
async fn server_settings(address: SocketAddr) -> BTreeMap<u16, u32> {
    // The client connection preface: the fixed string and an empty SETTINGS
    // frame (RFC 9113, section 3.4).
    const HTTP2_CLIENT_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n\0\0\0\x04\0\0\0\0\0";
    // The type of an HTTP/2 SETTINGS frame (RFC 9113, section 6.5).
    const HTTP2_SETTINGS_FRAME: u8 = 0x4;

    let mut connection = TcpStream::connect(address).await.unwrap();
    connection.write_all(HTTP2_CLIENT_PREFACE).await.unwrap();

    let read_settings = async {
        let mut header = [0u8; 9];
        connection.read_exact(&mut header).await.unwrap();
        assert_eq!(
            header[3], HTTP2_SETTINGS_FRAME,
            "the server must start with a SETTINGS frame"
        );
        let length = u32::from_be_bytes([0, header[0], header[1], header[2]]) as usize;
        let mut payload = vec![0; length];
        connection.read_exact(&mut payload).await.unwrap();
        payload
    };
    let payload = tokio::time::timeout(Duration::from_secs(5), read_settings)
        .await
        .expect("the server must send its SETTINGS");

    let (settings, _) = payload.as_chunks::<6>();
    settings
        .iter()
        .map(|&[id_0, id_1, value_0, value_1, value_2, value_3]| {
            (
                u16::from_be_bytes([id_0, id_1]),
                u32::from_be_bytes([value_0, value_1, value_2, value_3]),
            )
        })
        .collect()
}

/// Encodes `message` as one uncompressed gRPC message.
fn grpc_message(message: &impl prost::Message) -> Bytes {
    let encoded = message.encode_to_vec();
    let mut framed = Vec::with_capacity(5 + encoded.len());
    framed.push(0);
    framed.extend_from_slice(&(encoded.len() as u32).to_be_bytes());
    framed.extend_from_slice(&encoded);
    framed.into()
}
