// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Helpers shared by the tests of the listener metrics.

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{Router, routing::get};
use bytes::Bytes;
use fastcrypto::traits::KeyPair as _;
use iota_http::{Builder, Config, metrics::ListenerMetrics};
pub use iota_metrics::test_utils::MetricsReader;
use prometheus_filtered::{MetricLevel, Registry};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const PREFIX: &str = "t";

pub async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

pub fn setup() -> (MetricsReader, ListenerMetrics) {
    let registry = Registry::new();
    let metrics = ListenerMetrics::new(PREFIX, &registry, MetricLevel::Info);
    (MetricsReader::new(&registry).with_prefix(PREFIX), metrics)
}

/// The request a listener gives to its service.
pub type ServiceRequest = http::Request<iota_http::body::BoxBody>;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Starts a listener on a free port, with TLS if `tls_config` is given. The
/// server stops with the handle.
pub fn serve_handle<S, B>(
    config: Config,
    metrics: &ListenerMetrics,
    tls_config: Option<rustls::ServerConfig>,
    service: S,
) -> iota_http::ServerHandle<SocketAddr>
where
    S: tower::Service<
            ServiceRequest,
            Response = http::Response<B>,
            Error: Into<BoxError>,
            Future: Send,
        > + Clone
        + Send
        + 'static,
    B: http_body::Body<Data = Bytes, Error: Into<BoxError>> + Send + 'static,
{
    let mut builder = Builder::new().config(config.metrics(Some(metrics.clone())));
    if let Some(tls_config) = tls_config {
        builder = builder.tls_config(tls_config);
    }
    builder.serve(("127.0.0.1", 0), service).unwrap()
}

/// Lets the server run until the test ends and the runtime is dropped.
fn keep_running(handle: iota_http::ServerHandle<SocketAddr>) -> SocketAddr {
    let addr = *handle.local_addr();
    std::mem::forget(handle);
    addr
}

/// Like [`serve_handle`], with no TLS, and the server runs until the test ends.
pub fn serve_service<S, B>(config: Config, metrics: &ListenerMetrics, service: S) -> SocketAddr
where
    S: tower::Service<
            ServiceRequest,
            Response = http::Response<B>,
            Error: Into<BoxError>,
            Future: Send,
        > + Clone
        + Send
        + 'static,
    B: http_body::Body<Data = Bytes, Error: Into<BoxError>> + Send + 'static,
{
    keep_running(serve_handle(config, metrics, None, service))
}

pub fn serve(config: Config, metrics: &ListenerMetrics, app: Router) -> SocketAddr {
    serve_service(config, metrics, app)
}

pub fn ok_app() -> Router {
    Router::new().route("/", get(|| async { "ok" }))
}

/// Reads one HTTP/1.1 response with a content-length and returns its body.
async fn read_response(stream: &mut TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&data[..end]).to_lowercase();
            let length: usize = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length: "))
                .map_or(0, |v| v.trim().parse().unwrap());
            if data.len() >= end + 4 + length {
                return data[end + 4..end + 4 + length].to_vec();
            }
        }
        let n = stream.read(&mut chunk).await.unwrap();
        assert!(n > 0, "connection closed before the response ended");
        data.extend_from_slice(&chunk[..n]);
    }
}

pub async fn get_root(stream: &mut TcpStream) {
    stream
        .write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    assert_eq!(read_response(stream).await, b"ok");
}

pub async fn h2_connect(
    address: SocketAddr,
    builder: h2::client::Builder,
) -> h2::client::SendRequest<Bytes> {
    h2_connect_with_task(address, builder).await.0
}

/// Like [`h2_connect`]. Aborting the task closes the TCP connection.
pub async fn h2_connect_with_task(
    address: SocketAddr,
    builder: h2::client::Builder,
) -> (h2::client::SendRequest<Bytes>, tokio::task::JoinHandle<()>) {
    let tcp_stream = TcpStream::connect(address).await.unwrap();
    let (sender, connection) = builder.handshake::<_, Bytes>(tcp_stream).await.unwrap();
    let task = tokio::spawn(async move {
        let _ = connection.await;
    });
    (sender, task)
}

pub async fn h2_get(
    sender: &h2::client::SendRequest<Bytes>,
    path: &str,
) -> h2::client::ResponseFuture {
    let mut sender = sender.clone().ready().await.unwrap();
    let request = http::Request::get(format!("http://localhost{path}"))
        .body(())
        .unwrap();
    sender.send_request(request, true).unwrap().0
}

/// Reads the body of a response to its end and returns the number of bytes.
pub async fn h2_read_body(mut body: h2::RecvStream) -> usize {
    let mut body_len = 0;
    while let Some(chunk) = body.data().await {
        let chunk = chunk.unwrap();
        body_len += chunk.len();
        body.flow_control().release_capacity(chunk.len()).unwrap();
    }
    body_len
}

/// Reads a response to its end and returns the number of body bytes.
pub async fn h2_read(response: h2::client::ResponseFuture) -> usize {
    h2_read_body(response.await.unwrap().into_body()).await
}

pub const SERVER_NAME: &str = "iota-http-test";

pub fn tls_configs() -> (
    tokio_rustls::rustls::ServerConfig,
    tokio_rustls::rustls::ClientConfig,
) {
    let keypair = key_pair(SERVER_SEED);
    let public_key = keypair.public().to_owned();
    (
        iota_tls::create_rustls_server_config(keypair.private(), SERVER_NAME.to_string()),
        iota_tls::create_rustls_client_config(public_key, SERVER_NAME.to_string(), None),
    )
}

pub fn serve_tls(config: Config, metrics: &ListenerMetrics, app: Router) -> SocketAddr {
    let (server_config, _) = tls_configs();
    keep_running(serve_handle(config, metrics, Some(server_config), app))
}

/// A TLS listener that accepts the client key `CLIENT_SEED` only.
pub fn serve_mutual_tls(config: Config, metrics: &ListenerMetrics, app: Router) -> SocketAddr {
    let server_config = iota_tls::create_rustls_server_config_with_client_verifier(
        key_pair(SERVER_SEED).private(),
        SERVER_NAME.to_string(),
        iota_tls::AllowPublicKeys::new([key_pair(CLIENT_SEED).public().to_owned()].into()),
    );
    keep_running(serve_handle(config, metrics, Some(server_config), app))
}

/// Opens a TLS connection with the client key `CLIENT_SEED`.
pub async fn connect_mutual_tls(addr: SocketAddr) -> tokio_rustls::client::TlsStream<TcpStream> {
    let client_config = iota_tls::create_rustls_client_config(
        key_pair(SERVER_SEED).public().to_owned(),
        SERVER_NAME.to_string(),
        Some(key_pair(CLIENT_SEED).private()),
    );
    let tcp = TcpStream::connect(addr).await.unwrap();
    tokio_rustls::TlsConnector::from(Arc::new(client_config))
        .connect(SERVER_NAME.to_string().try_into().unwrap(), tcp)
        .await
        .unwrap()
}

const SERVER_SEED: u8 = 42;
const CLIENT_SEED: u8 = 43;

fn key_pair(seed: u8) -> fastcrypto::ed25519::Ed25519KeyPair {
    use fastcrypto::{ed25519::Ed25519PrivateKey, traits::ToFromBytes};
    fastcrypto::ed25519::Ed25519KeyPair::from(Ed25519PrivateKey::from_bytes(&[seed; 32]).unwrap())
}
