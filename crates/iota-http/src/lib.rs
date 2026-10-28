// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use connection_handler::{OnConnectionClose, SHUTDOWN_GRACE_PERIOD, sleep_or_pending};
pub use http;
use http::{Request, Response};
use hyper_util::service::TowerToHyperService;
use io::ServerIo;
use metrics::{HandshakeResult, RefusedReason, TrackedIo};
use tokio::task::JoinSet;
use tokio_rustls::{TlsAcceptor, rustls};
use tower::{Service, ServiceBuilder, ServiceExt};
use tracing::trace;

use self::{
    body::BoxBody,
    connection_info::{ActiveConnections, ConnectionCountGuard, ConnectionCounts},
};

pub mod body;
mod config;
mod connection_handler;
mod connection_info;
mod fuse;
mod idle;
mod io;
mod listener;
pub mod metrics;

pub use config::{Config, PeerConnectionEvent};
pub use connection_info::{ConnectInfo, ConnectionId, ConnectionInfo, PeerCertificates};
pub use listener::{Listener, ListenerExt};

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;
/// h2 alpn in plain format for rustls.
const ALPN_H2: &[u8] = b"h2";
/// h1 alpn in plain format for rustls.
const ALPN_H1: &[u8] = b"http/1.1";

/// Replaces the protocols a caller advertised with the ones this crate serves.
fn with_alpn_protocols(mut tls: rustls::ServerConfig, accept_http1: bool) -> rustls::ServerConfig {
    tls.alpn_protocols = if accept_http1 {
        vec![ALPN_H2.into(), ALPN_H1.into()]
    } else {
        vec![ALPN_H2.into()]
    };
    tls
}

#[derive(Default)]
pub struct Builder {
    config: Config,
    tls_config: Option<rustls::ServerConfig>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    // Convenience method for configuring TLS with a single server cert
    //
    // Attempts to load PEM formatted files for the certificate chain and private
    // key material from the provided file system paths.
    pub fn tls_single_cert(
        self,
        cert_file: impl AsRef<std::path::Path>,
        private_key_file: impl AsRef<std::path::Path>,
    ) -> Result<Self, BoxError> {
        let tls_config =
            iota_tls::create_rustls_server_config_from_pem(cert_file, private_key_file)?;
        Ok(self.tls_config(tls_config))
    }

    pub fn tls_config(mut self, tls_config: rustls::ServerConfig) -> Self {
        self.tls_config = Some(tls_config);
        self
    }

    pub fn serve<A, S, ResponseBody>(
        self,
        addr: A,
        service: S,
    ) -> Result<ServerHandle<std::net::SocketAddr>, BoxError>
    where
        A: std::net::ToSocketAddrs,
        S: Service<
                Request<BoxBody>,
                Response = Response<ResponseBody>,
                Error: Into<BoxError>,
                Future: Send,
            > + Clone
            + Send
            + 'static,
        ResponseBody: http_body::Body<Data = bytes::Bytes, Error: Into<BoxError>> + Send + 'static,
    {
        let listener = listener::TcpListenerWithOptions::new(
            addr,
            self.config.tcp_keepalive,
            self.config.ipv6_prefix_len,
        )?;

        Self::serve_with_listener(self, listener, service)
    }

    fn serve_with_listener<L, S, ResponseBody>(
        self,
        listener: L,
        service: S,
    ) -> Result<ServerHandle<L::Addr>, BoxError>
    where
        L: Listener<Addr = std::net::SocketAddr>,
        S: Service<
                Request<BoxBody>,
                Response = Response<ResponseBody>,
                Error: Into<BoxError>,
                Future: Send,
            > + Clone
            + Send
            + 'static,
        ResponseBody: http_body::Body<Data = bytes::Bytes, Error: Into<BoxError>> + Send + 'static,
    {
        self.config.validate()?;

        let local_addr = listener.local_addr()?;
        let graceful_shutdown_token = tokio_util::sync::CancellationToken::new();
        let connections = ActiveConnections::default();

        let tls_config = self
            .tls_config
            .map(|tls| Arc::new(with_alpn_protocols(tls, self.config.accept_http1)));

        if let (Some(metrics), Some(_)) = (&self.config.metrics, &tls_config) {
            metrics.enable_tls();
        }

        let (watch_sender, watch_receiver) = tokio::sync::watch::channel(());
        let connection_counts = ConnectionCounts::new(self.config.on_peer_connection_event.clone());
        let server = Server {
            config: self.config,
            tls_config,
            listener,
            local_addr,
            service: ServiceBuilder::new()
                .layer(tower::util::BoxCloneService::layer())
                .map_response(|response: Response<ResponseBody>| response.map(body::boxed))
                .map_err(Into::into)
                .service(service),
            pending_connections: JoinSet::new(),
            connection_handlers: JoinSet::new(),
            connections: connections.clone(),
            graceful_shutdown_token: graceful_shutdown_token.clone(),
            _watch_receiver: watch_receiver,
            connection_counts,
        };

        let handle = ServerHandle(Arc::new(HandleInner {
            local_addr,
            connections,
            graceful_shutdown_token,
            watch_sender,
        }));

        tokio::spawn(server.serve());

        Ok(handle)
    }
}

#[derive(Debug)]
pub struct ServerHandle<A = std::net::SocketAddr>(Arc<HandleInner<A>>);

#[derive(Debug)]
struct HandleInner<A = std::net::SocketAddr> {
    /// The local address of the server.
    local_addr: A,
    connections: ActiveConnections<A>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    watch_sender: tokio::sync::watch::Sender<()>,
}

impl<A> ServerHandle<A> {
    /// Returns the local address of the server
    pub fn local_addr(&self) -> &A {
        &self.0.local_addr
    }

    /// Trigger a graceful shutdown of the server, but don't wait till the
    /// server has completed shutting down. Every connection is asked to close,
    /// and those still open after a short grace are dropped, cutting any
    /// request they are still serving.
    pub fn trigger_shutdown(&self) {
        self.0.graceful_shutdown_token.cancel();
    }

    /// Completes once the network has been shutdown.
    ///
    /// This explicitly *does not* trigger the network to shutdown, see
    /// `trigger_shutdown` or `shutdown` if you want to trigger shutting
    /// down the server.
    pub async fn wait_for_shutdown(&self) {
        self.0.watch_sender.closed().await
    }

    /// Triggers a shutdown of the server and waits for it to complete shutting
    /// down.
    pub async fn shutdown(&self) {
        self.trigger_shutdown();
        self.wait_for_shutdown().await;
    }

    /// Checks if the Server has been shutdown.
    pub fn is_shutdown(&self) -> bool {
        self.0.watch_sender.is_closed()
    }

    pub fn connections(
        &self,
    ) -> std::sync::RwLockReadGuard<'_, HashMap<ConnectionId, ConnectionInfo<A>>> {
        self.0.connections.read().unwrap()
    }

    /// Returns the number of active connections the server is handling
    pub fn number_of_connections(&self) -> usize {
        self.connections().len()
    }
}

impl<A> Clone for ServerHandle<A> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// A connection that completed its handshake, with the places it holds under
/// the per-client limits.
type ConnectingOutput<Io, Addr> =
    Result<(ServerIo<TrackedIo<Io>>, Addr, ConnectionCountGuard), crate::BoxError>;

struct Server<L: Listener> {
    config: Config,
    tls_config: Option<Arc<rustls::ServerConfig>>,

    listener: L,
    local_addr: L::Addr,
    service: tower::util::BoxCloneService<Request<BoxBody>, Response<BoxBody>, crate::BoxError>,

    pending_connections: JoinSet<ConnectingOutput<L::Io, L::Addr>>,
    connection_handlers: JoinSet<()>,
    connections: ActiveConnections<L::Addr>,
    graceful_shutdown_token: tokio_util::sync::CancellationToken,
    // Used to signal to a ServerHandle when the server has completed shutting down
    _watch_receiver: tokio::sync::watch::Receiver<()>,
    connection_counts: ConnectionCounts,
}

impl<L> Server<L>
where
    L: Listener<Addr = std::net::SocketAddr>,
{
    async fn serve(mut self) -> Result<(), BoxError> {
        loop {
            tokio::select! {
                _ = self.graceful_shutdown_token.cancelled() => {
                    trace!("signal received, shutting down");
                    break;
                },
                // While the handshake limit is reached, leave new connections in the kernel
                // backlog rather than accepting them into an unbounded set of pending tasks.
                (io, remote_addr) = self.listener.accept(),
                    if !self.at_max_pending_connections() => {
                    self.handle_incoming(io, remote_addr);
                },
                // A failed task affects only its own connection, so the loop keeps serving
                // the others.
                Some(maybe_connection) = self.pending_connections.join_next() => {
                    let (io, remote_addr, count_guard) = match maybe_connection {
                        Ok(Ok((io, remote_addr, count_guard))) => {
                            (io, remote_addr, count_guard)
                        }
                        Ok(Err(e)) => {
                            tracing::debug!(error = %e, "error accepting connection");
                            continue;
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "connection handshake task failed");
                            continue;
                        }
                    };

                    trace!("connection accepted");
                    self.handle_connection(io, remote_addr, count_guard);
                },
                Some(connection_handler_output) = self.connection_handlers.join_next() => {
                    if let Err(e) = connection_handler_output {
                        tracing::error!(error = %e, "connection task failed");
                    }
                },
            }
        }

        // Shutting down, wait for all connection handlers to finish
        self.shutdown().await;

        Ok(())
    }

    /// Whether the connections still in their TLS handshake have reached
    /// `max_pending_connections`.
    fn at_max_pending_connections(&self) -> bool {
        self.config
            .max_pending_connections
            .is_some_and(|max| self.pending_connections.len() >= max.get())
    }

    fn handle_incoming(&mut self, io: L::Io, remote_addr: L::Addr) {
        let io = TrackedIo::new(io, self.config.metrics.as_ref(), remote_addr);
        // Closed rather than left in the kernel backlog, and before any
        // handshake, which a full listener would only waste.
        if self.at_max_connections() {
            // Dropping the connection closes it, releasing its file descriptor.
            trace!("listener is already serving its maximum connections, closing the new one");
            if let Some(metrics) = &self.config.metrics {
                metrics.record_refused(RefusedReason::ListenerFull);
            }
            return;
        }
        let mut count_guard = self.connection_counts.guard();
        if let Some(max) = self.config.max_connections_per_client_ip {
            let client_ip = self.listener.client_prefix(&remote_addr).into_boxed_slice();
            if !count_guard.add_client_ip(client_ip, max.get()) {
                // Dropping the connection closes it, releasing its file descriptor.
                trace!("client address already holds {max} connections, closing the new one");
                if let Some(metrics) = &self.config.metrics {
                    metrics.record_refused(RefusedReason::ClientIpLimit);
                }
                return;
            }
        }
        if let Some(tls) = self.tls_config.clone() {
            let tls_acceptor = TlsAcceptor::from(tls);
            let handshake_timeout = self.config.handshake_timeout;
            let handshake_guard = io.connection().and_then(|c| c.record_handshake_start());
            self.pending_connections.spawn(async move {
                let result = tokio::select! {
                    result = handshake(io, remote_addr, tls_acceptor) => Some(result),
                    // Dropping the handshake closes the connection, releasing its file descriptor.
                    _ = sleep_or_pending(handshake_timeout) => None,
                };
                if let Some(guard) = handshake_guard {
                    guard.record_result(match &result {
                        Some(Ok(_)) => HandshakeResult::Completed,
                        Some(Err(_)) => HandshakeResult::Failed,
                        None => HandshakeResult::TimedOut,
                    });
                }
                result
                    .unwrap_or_else(|| {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "TLS handshake timed out",
                        )
                        .into())
                    })
                    // A failed handshake gives its places back with the task.
                    .map(|(io, remote_addr)| (io, remote_addr, count_guard))
            });
        } else {
            self.handle_connection(ServerIo::new_io(io), remote_addr, count_guard);
        }
    }

    /// Whether the connections being served and those still in their
    /// handshake have reached `max_connections`.
    fn at_max_connections(&self) -> bool {
        self.config.max_connections.is_some_and(|max| {
            self.connection_handlers.len() + self.pending_connections.len() >= max.get()
        })
    }

    fn handle_connection(
        &mut self,
        io: ServerIo<TrackedIo<L::Io>>,
        remote_addr: L::Addr,
        mut count_guard: ConnectionCountGuard,
    ) {
        if let (Some(max), Some(peer)) =
            (self.config.max_connections_per_peer, peer_public_key(&io))
        {
            if !count_guard.add_peer_public_key(peer, max.get()) {
                // Dropping the connection closes it, releasing its file descriptor.
                trace!("peer already holds {max} connections, closing the new one");
                if let Some(metrics) = &self.config.metrics {
                    metrics.record_refused(RefusedReason::PeerLimit);
                }
                return;
            }
        }

        let connection_shutdown_token = self.graceful_shutdown_token.child_token();
        let idle_timer = idle::idle_sleep(self.config.max_connection_idle);
        let idle = idle_timer.handle();
        let connection_info = ConnectionInfo::new(
            remote_addr,
            io.peer_certs(),
            connection_shutdown_token.clone(),
            idle.clone(),
        );
        let connection_id = connection_info.id();
        let connect_info = connection_info::ConnectInfo {
            local_addr: self.local_addr,
            remote_addr: *connection_info.remote_address(),
        };
        let peer_certificates = connection_info.peer_certificates().cloned();
        let hyper_io = hyper_util::rt::TokioIo::new(io);

        let hyper_svc = TowerToHyperService::new(
            self.service
                .clone()
                .map_request(move |mut request: Request<hyper::body::Incoming>| {
                    request.extensions_mut().insert(connect_info.clone());
                    if let Some(peer_certificates) = peer_certificates.clone() {
                        request.extensions_mut().insert(peer_certificates);
                    }

                    request.map(body::boxed)
                })
                .map_future(move |future| {
                    // Held by the response body rather than dropped here, so a
                    // streaming response counts as work until its last frame.
                    let guard = idle.guard();
                    async move {
                        let response: Result<Response<BoxBody>, BoxError> = future.await;
                        response.map(|response| {
                            response.map(|inner| body::boxed(body::GuardedBody::new(inner, guard)))
                        })
                    }
                }),
        );

        self.connections
            .write()
            .unwrap()
            .insert(connection_id, connection_info);
        let on_connection_close =
            OnConnectionClose::new(connection_id, self.connections.clone(), count_guard);

        self.connection_handlers
            .spawn(connection_handler::serve_connection(
                hyper_io,
                hyper_svc,
                self.config.connection_builder(),
                connection_shutdown_token,
                self.config.max_connection_age,
                idle_timer,
                self.config.metrics.clone(),
                on_connection_close,
            ));
    }

    async fn shutdown(mut self) {
        // Just to be careful make sure the token is canceled
        self.graceful_shutdown_token.cancel();

        // Terminate any in-progress pending connections
        self.pending_connections.shutdown().await;

        // Wait for all connection handlers to terminate
        trace!(
            "waiting for {} connections to close",
            self.connection_handlers.len()
        );

        let graceful_shutdown =
            async { while self.connection_handlers.join_next().await.is_some() {} };

        // A single grace, not extended for connections still serving as when
        // one connection is closed: stopping the server must not wait on
        // long-lived streams, which are always serving.
        if tokio::time::timeout(SHUTDOWN_GRACE_PERIOD, graceful_shutdown)
            .await
            .is_err()
        {
            tracing::warn!(
                "Failed to stop all connection handlers in {:?}. Forcing shutdown.",
                SHUTDOWN_GRACE_PERIOD
            );
            self.connection_handlers.shutdown().await;
        }
    }
}

/// Runs the TLS handshake to completion.
async fn handshake<Io, Addr>(
    io: TrackedIo<Io>,
    remote_addr: Addr,
    tls_acceptor: TlsAcceptor,
) -> Result<(ServerIo<TrackedIo<Io>>, Addr), crate::BoxError>
where
    Io: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    tracing::trace!("accepting TLS connection");
    let io = tls_acceptor.accept(io).await?;
    Ok((ServerIo::new_tls_io(io), remote_addr))
}

/// Identifies the peer by the public key of the single certificate it
/// authenticated with, or `None` if it presented no certificate.
fn peer_public_key<Io>(io: &ServerIo<Io>) -> Option<Box<[u8]>> {
    let certs = io.peer_certs()?;
    let [certificate] = certs.as_slice() else {
        trace!("unexpected number of peer certificates: {}", certs.len());
        return None;
    };

    match iota_tls::public_key_from_certificate(certificate) {
        Ok(public_key) => Some(AsRef::<[u8]>::as_ref(&public_key).into()),
        Err(e) => {
            trace!("failed to extract the public key from the peer certificate: {e:?}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::{NonZeroU32, NonZeroUsize},
        time::Duration,
    };

    use axum::Router;

    use super::*;
    use crate::connection_handler::MAX_GRACE_PERIODS;

    #[tokio::test]
    async fn simple() {
        const MESSAGE: &str = "Hello, World!";

        let app = Router::new().route("/", axum::routing::get(|| async { MESSAGE }));

        let handle = Builder::new().serve(("localhost", 0), app).unwrap();

        let url = format!("http://{}", handle.local_addr());

        let _ = rustls::crypto::ring::default_provider().install_default();
        let response = reqwest::get(url).await.unwrap().bytes().await.unwrap();

        assert_eq!(response, MESSAGE.as_bytes());
    }

    #[tokio::test]
    async fn shutdown() {
        const MESSAGE: &str = "Hello, World!";

        let app = Router::new().route("/", axum::routing::get(|| async { MESSAGE }));

        let handle = Builder::new().serve(("localhost", 0), app).unwrap();

        let url = format!("http://{}", handle.local_addr());

        let _ = rustls::crypto::ring::default_provider().install_default();
        let response = reqwest::get(url).await.unwrap().bytes().await.unwrap();

        // a request was just made so we should have 1 active connection
        assert_eq!(handle.connections().len(), 1);

        assert_eq!(response, MESSAGE.as_bytes());

        assert!(!handle.is_shutdown());

        handle.shutdown().await;

        assert!(handle.is_shutdown());

        // Now that the network has been shutdown there should be zero connections
        assert_eq!(handle.connections().len(), 0);
    }

    const SERVER_NAME: &str = "iota-http-test";

    /// A server config and a client config that trusts it, without client
    /// authentication.
    fn test_tls_configs() -> (rustls::ServerConfig, rustls::ClientConfig) {
        use fastcrypto::{
            ed25519::{Ed25519KeyPair, Ed25519PrivateKey},
            traits::{KeyPair, ToFromBytes},
        };

        let keypair = Ed25519KeyPair::from(Ed25519PrivateKey::from_bytes(&[42; 32]).unwrap());
        let public_key = keypair.public().to_owned();
        (
            iota_tls::create_rustls_server_config(keypair.private(), SERVER_NAME.to_string()),
            iota_tls::create_rustls_client_config(public_key, SERVER_NAME.to_string(), None),
        )
    }

    /// A caller may hand over a `ServerConfig` that already advertises
    /// protocols; what this listener serves is not up to them.
    #[test]
    fn the_listener_decides_which_protocols_it_advertises() {
        let (server_tls_config, _) = test_tls_configs();
        let mut caller_set_h2 = server_tls_config;
        caller_set_h2.alpn_protocols = vec![ALPN_H2.into(), b"spdy/3".to_vec()];

        let http2_only = with_alpn_protocols(caller_set_h2.clone(), false);
        assert_eq!(http2_only.alpn_protocols, vec![ALPN_H2.to_vec()]);

        let with_http1 = with_alpn_protocols(caller_set_h2, true);
        assert_eq!(
            with_http1.alpn_protocols,
            vec![ALPN_H2.to_vec(), ALPN_H1.to_vec()]
        );
    }

    /// The peer is unauthenticated until its handshake completes, so a peer
    /// that never starts one must not hold the connection open indefinitely.
    #[tokio::test]
    async fn silent_peer_is_dropped_after_the_handshake_timeout() {
        use tokio::io::AsyncReadExt as _;

        const HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(200);

        let (server_tls_config, _) = test_tls_configs();
        let handle = Builder::new()
            .config(Config::default().handshake_timeout(Some(HANDSHAKE_TIMEOUT)))
            .tls_config(server_tls_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();

        // Connect, then never send a ClientHello.
        let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();

        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(HANDSHAKE_TIMEOUT * 25, connection.read(&mut buf))
            .await
            .expect("the server must not wait for the handshake past the deadline");

        assert!(
            matches!(read, Ok(0) | Err(_)),
            "the server must close the connection, got {read:?}"
        );
    }

    /// A TLS-configured listener only speaks TLS: a peer that starts with a
    /// plaintext HTTP/2 preface is closed before any request is dispatched.
    #[tokio::test]
    async fn plaintext_peer_is_refused_by_a_tls_listener() {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let (server_tls_config, _) = test_tls_configs();
        let served = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let app = Router::new().route(
            "/",
            axum::routing::get({
                let served = served.clone();
                move || async move {
                    served.store(true, std::sync::atomic::Ordering::SeqCst);
                    "served"
                }
            }),
        );
        let handle = Builder::new()
            .tls_config(server_tls_config)
            .serve(("localhost", 0), app)
            .unwrap();

        let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();
        connection
            .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
            .await
            .unwrap();
        connection
            .write_all(&[0, 0, 0, 0x4, 0, 0, 0, 0, 0])
            .await
            .unwrap();

        // The TLS acceptor answers garbage with an alert record and closes;
        // anything else would mean the plaintext bytes were served.
        let received = tokio::time::timeout(Duration::from_secs(5), async {
            let mut received = Vec::new();
            let mut buf = [0u8; 64];
            loop {
                match connection.read(&mut buf).await {
                    Ok(0) | Err(_) => break received,
                    Ok(n) => received.extend_from_slice(&buf[..n]),
                }
            }
        })
        .await
        .expect("the server must close a plaintext connection promptly");
        const TLS_ALERT_RECORD: u8 = 0x15;
        assert!(
            received
                .first()
                .is_none_or(|first| *first == TLS_ALERT_RECORD),
            "the server must answer plaintext with a TLS alert at most, got {received:?}"
        );
        assert!(
            !served.load(std::sync::atomic::Ordering::SeqCst),
            "no request may reach the service over plaintext"
        );
    }

    /// An HTTP/1 peer that never finishes sending its request headers is
    /// closed once the header deadline passes.
    #[tokio::test]
    async fn http1_peer_that_never_finishes_its_headers_is_closed() {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        const HEADER_DEADLINE: Duration = Duration::from_millis(200);

        let handle = Builder::new()
            .config(Config::default().http1_header_read_timeout(Some(HEADER_DEADLINE)))
            .serve(("localhost", 0), Router::new())
            .unwrap();

        let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();
        // A request line and one header, but never the blank line that ends
        // the header block.
        connection
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n")
            .await
            .unwrap();
        let sent_at = tokio::time::Instant::now();

        // Without the deadline the peer is held forever, so a wide window here
        // only guards against a slow machine, it does not weaken the assertion.
        let closed = tokio::time::timeout(HEADER_DEADLINE * 25, async {
            let mut buf = [0u8; 1024];
            loop {
                match connection.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => continue,
                }
            }
        })
        .await;
        assert!(
            closed.is_ok(),
            "the server must close a peer that stalls its request headers"
        );
        assert!(
            sent_at.elapsed() >= HEADER_DEADLINE,
            "the peer must be given the whole deadline before it is closed"
        );
    }

    /// A server that does not accept HTTP/1 refuses it on plaintext, and on
    /// TLS whether or not the client chose HTTP/2 through ALPN. A plaintext
    /// HTTP/2 request is still served.
    #[tokio::test]
    async fn a_server_without_http1_does_not_serve_an_http1_request() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let (server_tls_config, client_tls_config) = test_tls_configs();
        let served = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/",
            axum::routing::get({
                let served = served.clone();
                move || async move {
                    served.fetch_add(1, Ordering::SeqCst);
                    "served"
                }
            }),
        );
        let config = Config::default().accept_http1(false);
        let plaintext = Builder::new()
            .config(config.clone())
            .serve(("localhost", 0), app.clone())
            .unwrap();
        let tls = Builder::new()
            .config(config)
            .tls_config(server_tls_config)
            .serve(("localhost", 0), app)
            .unwrap();

        let connection = tokio::net::TcpStream::connect(plaintext.local_addr())
            .await
            .unwrap();
        assert_http1_request_is_refused(connection).await;

        for alpn_protocols in [vec![], vec![ALPN_H2.to_vec()]] {
            let mut client_tls_config = client_tls_config.clone();
            client_tls_config.alpn_protocols = alpn_protocols;
            let connection = tokio::net::TcpStream::connect(tls.local_addr())
                .await
                .unwrap();
            let connection = tokio_rustls::TlsConnector::from(Arc::new(client_tls_config))
                .connect(
                    rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap(),
                    connection,
                )
                .await
                .unwrap();
            assert_http1_request_is_refused(connection).await;
        }

        assert_eq!(
            served.load(Ordering::SeqCst),
            0,
            "no HTTP/1 request may reach the service"
        );

        let _ = rustls::crypto::ring::default_provider().install_default();
        let response = reqwest::Client::builder()
            .http2_prior_knowledge()
            .build()
            .unwrap()
            .get(format!("http://{}", plaintext.local_addr()))
            .send()
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), "served");
    }

    /// Sends an HTTP/1 request and asserts that the server answers as HTTP/2
    /// and closes the connection.
    async fn assert_http1_request_is_refused(
        mut connection: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    ) {
        use tokio::io::AsyncWriteExt as _;

        // The type of an HTTP/2 SETTINGS frame (RFC 9113, section 6.5).
        const HTTP2_SETTINGS_FRAME: u8 = 0x4;

        connection
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();

        let mut received = Vec::new();
        let closed = read_until_done_or_closed(&mut connection, &mut received, |_| false).await;
        assert_eq!(
            frame_types(&received).first(),
            Some(&HTTP2_SETTINGS_FRAME),
            "the server must answer as HTTP/2, got {:?}",
            String::from_utf8_lossy(&received)
        );
        assert!(closed, "the server must close the connection");
    }

    /// A served HTTP/2 connection gets a GOAWAY frame when the server shuts
    /// down and when the connection reaches its maximum age, whether or not
    /// the server accepts HTTP/1.
    #[tokio::test]
    async fn an_http2_connection_gets_goaway_on_shutdown_and_at_its_maximum_age() {
        use tokio::io::AsyncWriteExt as _;

        // The client connection preface: the fixed string and an empty
        // SETTINGS frame (RFC 9113, section 3.4).
        const HTTP2_CLIENT_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n\0\0\0\x04\0\0\0\0\0";
        // The types of HTTP/2 SETTINGS and GOAWAY frames (RFC 9113, sections
        // 6.5 and 6.8).
        const HTTP2_SETTINGS_FRAME: u8 = 0x4;
        const HTTP2_GOAWAY_FRAME: u8 = 0x7;

        for accept_http1 in [true, false] {
            for max_connection_age in [None, Some(Duration::from_millis(500))] {
                let mut config = Config::default().accept_http1(accept_http1);
                if let Some(max_connection_age) = max_connection_age {
                    config = config.max_connection_age(max_connection_age);
                }
                let handle = Builder::new()
                    .config(config)
                    .serve(("localhost", 0), Router::new())
                    .unwrap();
                let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
                    .await
                    .unwrap();
                connection.write_all(HTTP2_CLIENT_PREFACE).await.unwrap();

                let mut received = Vec::new();
                read_until_done_or_closed(&mut connection, &mut received, |received| {
                    !frame_types(received).is_empty()
                })
                .await;
                assert_eq!(
                    frame_types(&received).first(),
                    Some(&HTTP2_SETTINGS_FRAME),
                    "the server must serve the connection as HTTP/2"
                );

                if max_connection_age.is_none() {
                    handle.trigger_shutdown();
                }
                read_until_done_or_closed(&mut connection, &mut received, |received| {
                    frame_types(received).contains(&HTTP2_GOAWAY_FRAME)
                })
                .await;
                assert!(
                    frame_types(&received).contains(&HTTP2_GOAWAY_FRAME),
                    "accept_http1: {accept_http1}, max_connection_age: {max_connection_age:?}, \
                     frames: {:?}",
                    frame_types(&received)
                );
            }
        }
    }

    /// Reads from `connection` into `received` until `done(received)` holds or
    /// the peer closes the connection. Returns `false` if neither happens
    /// within 5 s.
    async fn read_until_done_or_closed(
        connection: &mut (impl tokio::io::AsyncRead + Unpin),
        received: &mut Vec<u8>,
        done: impl Fn(&[u8]) -> bool,
    ) -> bool {
        use tokio::io::AsyncReadExt as _;

        tokio::time::timeout(Duration::from_secs(5), async {
            let mut buf = [0u8; 1024];
            while !done(received) {
                match connection.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => received.extend_from_slice(&buf[..n]),
                }
            }
        })
        .await
        .is_ok()
    }

    /// The types of the HTTP/2 frames whose header is in `bytes`.
    fn frame_types(bytes: &[u8]) -> Vec<u8> {
        let mut types = Vec::new();
        let mut rest = bytes;
        while let Some((header, payload)) = rest.split_first_chunk::<9>() {
            types.push(header[3]);
            let length = u32::from_be_bytes([0, header[0], header[1], header[2]]) as usize;
            rest = payload.get(length..).unwrap_or_default();
        }
        types
    }

    /// A peer that completes the HTTP/2 preface and then falls silent is
    /// closed once it fails to answer the keepalive ping.
    #[tokio::test]
    async fn silent_http2_peer_is_closed_by_keepalive() {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        const KEEPALIVE: Duration = Duration::from_millis(100);

        let handle = Builder::new()
            .config(
                Config::default()
                    .http2_keepalive_interval(Some(KEEPALIVE))
                    .http2_keepalive_timeout(Some(KEEPALIVE)),
            )
            .serve(("localhost", 0), Router::new())
            .unwrap();

        let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();
        // Connection preface and an empty SETTINGS frame, then nothing more.
        connection
            .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
            .await
            .unwrap();
        connection
            .write_all(&[0, 0, 0, 0x4, 0, 0, 0, 0, 0])
            .await
            .unwrap();

        let closed = tokio::time::timeout(KEEPALIVE * 50, async {
            let mut buf = [0u8; 1024];
            loop {
                match connection.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => continue,
                }
            }
        })
        .await;
        assert!(
            closed.is_ok(),
            "the server must close a peer that ignores keepalive pings"
        );
    }

    /// While the pending limit is reached the server stops accepting, and
    /// resumes once a slot frees.
    #[tokio::test]
    async fn pending_connections_are_capped() {
        let (server_tls_config, client_tls_config) = test_tls_configs();
        let handle = Builder::new()
            .config(
                Config::default()
                    // Long enough that the stalled peer below is released by the test
                    // rather than by the deadline.
                    .handshake_timeout(Some(Duration::from_secs(60)))
                    .max_pending_connections(Some(NonZeroUsize::MIN)),
            )
            .tls_config(server_tls_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();
        let addr = *handle.local_addr();

        // Occupy the only handshake slot with a peer that never speaks.
        let stalled = tokio::net::TcpStream::connect(addr).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_tls_config));
        let server_name = rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap();
        let handshake = async {
            let io = tokio::net::TcpStream::connect(addr).await.unwrap();
            connector.connect(server_name, io).await
        };
        tokio::pin!(handshake);

        // The connection reaches the kernel backlog but is not accepted. Without the
        // limit the handshake completes in milliseconds, so a wide window here only
        // guards against a slow machine, it does not weaken the assertion.
        assert!(
            tokio::time::timeout(Duration::from_secs(2), &mut handshake)
                .await
                .is_err(),
            "the server must not handshake while the pending limit is reached"
        );

        drop(stalled);

        tokio::time::timeout(Duration::from_secs(10), &mut handshake)
            .await
            .expect("the server must resume accepting once a slot frees")
            .expect("the handshake must succeed");
    }

    /// A peer at its connection limit gets no further connections, while every
    /// other peer is served as usual and the peer itself recovers a slot as
    /// soon as one of its connections closes.
    #[tokio::test]
    async fn connections_per_peer_are_capped() {
        use fastcrypto::{
            ed25519::{Ed25519KeyPair, Ed25519PrivateKey},
            traits::{KeyPair, ToFromBytes},
        };
        use tokio::io::AsyncReadExt as _;

        const MAX_PER_PEER: usize = 2;

        let client_key =
            |seed: u8| Ed25519KeyPair::from(Ed25519PrivateKey::from_bytes(&[seed; 32]).unwrap());
        let server_keypair = client_key(1);
        let server_public_key = server_keypair.public().to_owned();
        let server_config = iota_tls::create_rustls_server_config_with_client_verifier(
            server_keypair.private(),
            SERVER_NAME.to_string(),
            iota_tls::AllowPublicKeys::new(
                [
                    client_key(2).public().to_owned(),
                    client_key(3).public().to_owned(),
                ]
                .into(),
            ),
        );

        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let handle = Builder::new()
            .config(
                Config::default()
                    .max_connections_per_peer(NonZeroUsize::new(MAX_PER_PEER))
                    .on_peer_connection_event({
                        let events = events.clone();
                        move |peer, event| events.lock().unwrap().push((peer.to_vec(), event))
                    }),
            )
            .tls_config(server_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();
        let addr = *handle.local_addr();

        let server_name = rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap();
        let connect = |seed: u8| {
            let connector =
                tokio_rustls::TlsConnector::from(Arc::new(iota_tls::create_rustls_client_config(
                    server_public_key.clone(),
                    SERVER_NAME.to_string(),
                    Some(client_key(seed).private()),
                )));
            let server_name = server_name.clone();
            async move {
                let io = tokio::net::TcpStream::connect(addr).await.unwrap();
                connector.connect(server_name, io).await.unwrap()
            }
        };

        let events_of = |seed: u8| -> Vec<PeerConnectionEvent> {
            let peer = client_key(seed).public().as_ref().to_vec();
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|(reported, _)| *reported == peer)
                .map(|(_, event)| *event)
                .collect()
        };

        // The connection is established before the server has necessarily
        // registered or dropped it, so settle on the count rather than assert
        // on it immediately.
        let established = async |expected: usize| {
            for _ in 0..100 {
                if handle.number_of_connections() == expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!(
                "expected {expected} connections, the server holds {}",
                handle.number_of_connections()
            );
        };

        let mut peer = Vec::new();
        for _ in 0..MAX_PER_PEER {
            peer.push(connect(2).await);
        }
        established(MAX_PER_PEER).await;

        let mut refused = connect(2).await;
        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(10), refused.read(&mut buf))
            .await
            .expect("the server must close a connection past the limit");
        assert!(
            matches!(read, Ok(0) | Err(_)),
            "the server must close the connection, got {read:?}"
        );
        established(MAX_PER_PEER).await;
        assert_eq!(
            events_of(2),
            [
                PeerConnectionEvent::Established { held: 1 },
                PeerConnectionEvent::Established { held: 2 },
                PeerConnectionEvent::RefusedAtLimit { held: 2 },
            ],
            "the refusal must be reported against the peer that caused it"
        );

        // A peer sitting at its limit must not consume anyone else's budget.
        let other_peer = connect(3).await;
        established(MAX_PER_PEER + 1).await;

        peer.pop();
        established(MAX_PER_PEER).await;
        let reconnected = connect(2).await;
        established(MAX_PER_PEER + 1).await;
        assert_eq!(
            events_of(2),
            [
                PeerConnectionEvent::Established { held: 1 },
                PeerConnectionEvent::Established { held: 2 },
                PeerConnectionEvent::RefusedAtLimit { held: 2 },
                PeerConnectionEvent::Closed { held: 1 },
                PeerConnectionEvent::Established { held: 2 },
            ],
            "closing and reconnecting must be reported with the count held afterwards"
        );
        assert_eq!(
            events_of(3),
            [PeerConnectionEvent::Established { held: 1 }],
            "another peer's events must not leak into this one's"
        );

        drop((peer, other_peer, reconnected));
    }

    /// An authenticated connection counts under its client's address from
    /// accept and under its peer's key from the end of its handshake, and
    /// gives both back when it closes. Only the key is reported to
    /// `on_peer_connection_event`.
    #[tokio::test]
    async fn a_connection_is_counted_under_its_address_and_its_key() {
        use fastcrypto::{
            ed25519::{Ed25519KeyPair, Ed25519PrivateKey},
            traits::{KeyPair, ToFromBytes},
        };
        use tokio::io::AsyncReadExt as _;

        const MAX_PER_CLIENT: usize = 2;

        let client_key =
            |seed: u8| Ed25519KeyPair::from(Ed25519PrivateKey::from_bytes(&[seed; 32]).unwrap());
        let server_keypair = client_key(1);
        let server_public_key = server_keypair.public().to_owned();
        let server_config = iota_tls::create_rustls_server_config_with_client_verifier(
            server_keypair.private(),
            SERVER_NAME.to_string(),
            iota_tls::AllowPublicKeys::new(
                [2, 3, 4]
                    .map(|seed| client_key(seed).public().to_owned())
                    .into(),
            ),
        );

        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let handle = Builder::new()
            .config(
                Config::default()
                    .max_connections_per_client_ip(NonZeroUsize::new(MAX_PER_CLIENT))
                    .max_connections_per_peer(Some(NonZeroUsize::MIN))
                    .on_peer_connection_event({
                        let events = events.clone();
                        move |peer, event| events.lock().unwrap().push((peer.to_vec(), event))
                    }),
            )
            .tls_config(server_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();
        let addr = *handle.local_addr();

        let server_name = rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap();
        let connect = |seed: u8| {
            let connector =
                tokio_rustls::TlsConnector::from(Arc::new(iota_tls::create_rustls_client_config(
                    server_public_key.clone(),
                    SERVER_NAME.to_string(),
                    Some(client_key(seed).private()),
                )));
            let server_name = server_name.clone();
            async move {
                let io = tokio::net::TcpStream::connect(addr).await.unwrap();
                tokio::time::timeout(Duration::from_secs(10), connector.connect(server_name, io))
                    .await
                    .expect("the server must answer")
            }
        };
        let events_of = |seed: u8| -> Vec<PeerConnectionEvent> {
            let peer = client_key(seed).public().as_ref().to_vec();
            events
                .lock()
                .unwrap()
                .iter()
                .filter(|(reported, _)| *reported == peer)
                .map(|(_, event)| *event)
                .collect()
        };

        let first = connect(2).await.expect("the first peer must be served");
        wait_for_connections(&handle, 1, "the first peer must be counted").await;

        // Refused for its key after the handshake, so the client sees the
        // handshake complete and then the connection close.
        let mut refused = connect(2)
            .await
            .expect("a peer at its own limit is refused only after its handshake");
        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(10), refused.read(&mut buf))
            .await
            .expect("the server must close a connection past the peer's limit");
        assert!(
            matches!(read, Ok(0) | Err(_)),
            "the server must close the connection, got {read:?}"
        );

        // Takes the address's second place, which the refused connection
        // must have given back.
        let second = connect(3)
            .await
            .expect("a connection refused for its key must give its address's place back");
        wait_for_connections(&handle, 2, "the second peer must be counted").await;

        assert!(
            connect(4).await.is_err(),
            "an address at its limit must be refused before the handshake"
        );

        drop(first);
        wait_for_connections(&handle, 1, "the first peer's connection must close").await;
        let third = connect(4)
            .await
            .expect("a closed connection must give its address's place back");
        wait_for_connections(&handle, 2, "the third peer must be counted").await;

        assert_eq!(
            events_of(2),
            [
                PeerConnectionEvent::Established { held: 1 },
                PeerConnectionEvent::RefusedAtLimit { held: 1 },
                PeerConnectionEvent::Closed { held: 0 },
            ]
        );
        assert_eq!(events_of(3), [PeerConnectionEvent::Established { held: 1 }]);
        assert_eq!(events_of(4), [PeerConnectionEvent::Established { held: 1 }]);
        assert_eq!(
            events.lock().unwrap().len(),
            5,
            "only the peers' keys must be reported, never the client's address"
        );

        drop((second, third));
    }

    /// The cap identifies a peer by its single certificate, so a peer sending
    /// a longer chain is refused at the handshake and never holds a
    /// connection.
    #[tokio::test]
    async fn peer_with_extra_certificates_is_refused_at_the_handshake() {
        use fastcrypto::{
            ed25519::{Ed25519KeyPair, Ed25519PrivateKey},
            traits::{KeyPair, ToFromBytes},
        };
        use tokio::io::AsyncReadExt as _;

        let key =
            |seed: u8| Ed25519KeyPair::from(Ed25519PrivateKey::from_bytes(&[seed; 32]).unwrap());
        let server_public_key = key(1).public().to_owned();
        let client_public_key = key(2).public().to_owned();
        let server_config = iota_tls::create_rustls_server_config_with_client_verifier(
            key(1).private(),
            SERVER_NAME.to_string(),
            iota_tls::AllowPublicKeys::new([client_public_key].into()),
        );
        let handle = Builder::new()
            .config(Config::default().max_connections_per_peer(Some(NonZeroUsize::MIN)))
            .tls_config(server_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();

        // The allowed certificate followed by an unrelated one.
        let client_certificate =
            iota_tls::SelfSignedCertificate::new(key(2).private(), SERVER_NAME);
        let extra_certificate = iota_tls::SelfSignedCertificate::new(key(3).private(), SERVER_NAME);
        let client_config =
            iota_tls::ServerCertVerifier::new(server_public_key, SERVER_NAME.to_string())
                .rustls_client_config_with_client_auth(
                    vec![
                        client_certificate.rustls_certificate(),
                        extra_certificate.rustls_certificate(),
                    ],
                    client_certificate.rustls_private_key(),
                )
                .unwrap();

        // In TLS 1.3 the client may consider the handshake done before the
        // server's rejection arrives, so the refusal can surface on the first
        // read instead.
        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));
        let io = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();
        let server_name = rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap();
        if let Ok(mut connection) = connector.connect(server_name, io).await {
            let mut buf = [0u8; 1];
            let read = tokio::time::timeout(Duration::from_secs(10), connection.read(&mut buf))
                .await
                .expect("the server must close a connection with more than one certificate");
            assert!(
                matches!(read, Ok(0) | Err(_)),
                "the server must close the connection, got {read:?}"
            );
        }
        assert_eq!(handle.number_of_connections(), 0);
    }

    /// A limit whose slots nothing releases leaves the server unable to
    /// accept.
    #[tokio::test]
    async fn unrecoverable_limits_are_rejected() {
        let served = |config| {
            Builder::new()
                .config(config)
                .tls_config(test_tls_configs().0)
                .serve(("localhost", 0), Router::new())
        };

        assert!(
            served(
                Config::default()
                    .handshake_timeout(None)
                    .max_pending_connections(NonZeroUsize::new(8))
            )
            .is_err(),
            "a limit without a handshake deadline must be rejected"
        );
        assert!(
            served(
                Config::default()
                    .handshake_timeout(None)
                    .max_pending_connections(None)
            )
            .is_ok(),
            "removing both bounds stays allowed"
        );
    }

    #[tokio::test]
    async fn settings_that_cannot_work_are_rejected() {
        let served = |config| {
            Builder::new()
                .config(config)
                .serve(("localhost", 0), Router::new())
        };
        let zero = Some(Duration::ZERO);
        let size = |size| NonZeroU32::new(size);

        for (setting, config) in [
            ("tcp_keepalive", Config::default().tcp_keepalive(zero)),
            (
                "http2_keepalive_interval",
                Config::default().http2_keepalive_interval(zero),
            ),
            (
                "http2_keepalive_timeout",
                Config::default().http2_keepalive_timeout(zero),
            ),
            (
                "http1_header_read_timeout",
                Config::default().http1_header_read_timeout(zero),
            ),
            (
                "max_connection_age",
                Config::default().max_connection_age(Duration::ZERO),
            ),
            (
                "handshake_timeout",
                Config::default().handshake_timeout(zero),
            ),
            (
                "max_connection_idle",
                Config::default().max_connection_idle(zero),
            ),
            (
                "initial_stream_window_size",
                Config::default().initial_stream_window_size(size(1 << 31)),
            ),
            (
                "initial_connection_window_size",
                Config::default().initial_connection_window_size(size(1 << 31)),
            ),
            (
                "max_frame_size",
                Config::default().max_frame_size(size((1 << 14) - 1)),
            ),
            (
                "max_frame_size",
                Config::default().max_frame_size(size(1 << 24)),
            ),
            (
                "ipv6_prefix_len",
                Config::default().ipv6_prefix_len(listener::MIN_IPV6_PREFIX_LEN - 1),
            ),
            ("ipv6_prefix_len", Config::default().ipv6_prefix_len(129)),
        ] {
            let error = served(config).err().map(|error| error.to_string());
            assert!(
                error.as_ref().is_some_and(|error| error.contains(setting)),
                "'{setting}' must be rejected naming it, got {error:?}"
            );
        }

        for config in [
            Config::default()
                .initial_stream_window_size(size((1 << 31) - 1))
                .initial_connection_window_size(size((1 << 31) - 1)),
            Config::default().max_frame_size(size(1 << 14)),
            Config::default().max_frame_size(size((1 << 24) - 1)),
            Config::default().ipv6_prefix_len(listener::MIN_IPV6_PREFIX_LEN),
            Config::default().ipv6_prefix_len(128),
        ] {
            assert!(
                served(config.clone()).is_ok(),
                "the largest and smallest allowed values must be accepted: {config:?}"
            );
        }
    }

    /// An id derived from the connection's address in memory comes back as
    /// soon as the allocator reuses that address, so two live connections can
    /// share one. The listener's table is keyed by it: the second insertion
    /// would replace the first, and the first connection's own close would
    /// then remove the newcomer's entry.
    #[test]
    fn a_connection_id_is_never_reused() {
        let connection = || {
            ConnectionInfo::new(
                "127.0.0.1:1".parse::<std::net::SocketAddr>().unwrap(),
                None,
                tokio_util::sync::CancellationToken::new(),
                idle::idle_sleep(None).handle(),
            )
        };

        // Dropped before the next is made, which is what frees the address for
        // the next allocation to be handed.
        let first = connection();
        let first_id = first.id();
        drop(first);

        let second = connection();
        assert_ne!(
            first_id,
            second.id(),
            "an id must not be reused once its connection has gone"
        );
    }

    #[tokio::test]
    async fn a_peer_that_never_picks_a_protocol_is_closed_when_idle() {
        use tokio::io::AsyncReadExt as _;

        const IDLE: Duration = Duration::from_millis(200);

        let handle = Builder::new()
            .config(Config::default().max_connection_idle(Some(IDLE)))
            .serve(("localhost", 0), Router::new())
            .unwrap();

        let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();

        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(IDLE * 50, connection.read(&mut buf))
            .await
            .expect("an idle connection must be closed by its deadline");
        assert!(
            matches!(read, Ok(0) | Err(_)),
            "the server must close the connection, got {read:?}"
        );
    }

    /// A streaming response resolves its future long before its last frame, so
    /// the guard rides the body. While one is alive the connection is busy.
    #[tokio::test]
    async fn a_request_in_flight_keeps_a_connection_from_going_idle() {
        const IDLE: Duration = Duration::from_millis(100);

        let timer = idle::idle_sleep(Some(IDLE));
        let handle = timer.handle();
        let mut timer = std::pin::pin!(timer);

        let guard = handle.guard();
        assert_eq!(handle.is_busy(), Some(true));

        // Well past the deadline, but the request has not finished.
        assert!(
            tokio::time::timeout(IDLE * 10, &mut timer).await.is_err(),
            "a connection serving a request must not be considered idle"
        );

        // Once it does, the deadline runs from that point.
        drop(guard);
        assert_eq!(handle.is_busy(), Some(false));
        assert!(
            tokio::time::timeout(IDLE * 10, &mut timer).await.is_ok(),
            "a connection must go idle once its last request finishes"
        );
    }

    /// A timer with no deadline is never reached, however long the connection
    /// sits there.
    #[tokio::test]
    async fn a_connection_with_no_idle_deadline_is_never_closed_for_idleness() {
        let timer = idle::idle_sleep(None);
        let mut timer = std::pin::pin!(timer);

        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut timer)
                .await
                .is_err(),
            "a connection with no deadline must not be closed for being idle"
        );
    }

    /// A connection with no idle deadline is not tracked: the guard a request
    /// takes counts nothing, and the connection reports no idle time.
    #[test]
    fn a_connection_with_no_idle_deadline_is_not_tracked() {
        let timer = idle::idle_sleep(None);
        let handle = timer.handle();

        assert_eq!(
            handle.idle_for(),
            None,
            "an untracked connection must report no idle time"
        );

        let _guard = handle.guard();
        assert_eq!(
            timer.is_busy(),
            None,
            "a request on an untracked connection must not be counted"
        );
    }

    /// The deadline runs from the last request rather than being sampled once
    /// a period: a connection busy for several periods and then quiet must be
    /// given a full period from the moment it went quiet, not closed at the
    /// next tick.
    #[tokio::test]
    async fn the_deadline_runs_from_the_last_request() {
        const IDLE: Duration = Duration::from_millis(100);

        let timer = idle::idle_sleep(Some(IDLE));
        let handle = timer.handle();
        let mut timer = std::pin::pin!(timer);

        let guard = handle.guard();
        // Held for several deadlines, so a timer that merely ticks would have
        // fired more than once by now.
        tokio::time::sleep(IDLE * 3).await;
        let became_idle = tokio::time::Instant::now();
        drop(guard);

        tokio::time::timeout(IDLE * 10, &mut timer)
            .await
            .expect("the connection must go idle after its last request");
        assert!(
            became_idle.elapsed() >= IDLE,
            "the deadline must run from the last request, elapsed only {:?}",
            became_idle.elapsed()
        );
    }

    /// A connection goes idle, serves another request and goes idle again any
    /// number of times, and each time it is given a full deadline from the end
    /// of its last request.
    #[tokio::test]
    async fn a_connection_goes_idle_again_after_each_request() {
        const IDLE: Duration = Duration::from_millis(50);

        let timer = idle::idle_sleep(Some(IDLE));
        let handle = timer.handle();
        let mut timer = std::pin::pin!(timer);

        tokio::time::timeout(IDLE * 10, &mut timer)
            .await
            .expect("a connection that is never used must go idle");

        for request in 0..3 {
            let guard = handle.guard();
            assert!(
                tokio::time::timeout(IDLE * 3, &mut timer).await.is_err(),
                "request {request}: a connection serving a request must not be considered idle"
            );
            let became_idle = tokio::time::Instant::now();
            drop(guard);

            tokio::time::timeout(IDLE * 10, &mut timer)
                .await
                .unwrap_or_else(|_| panic!("request {request}: the connection must go idle again"));
            assert!(
                became_idle.elapsed() >= IDLE,
                "request {request}: the deadline must run from the end of the request, elapsed only {:?}",
                became_idle.elapsed()
            );
        }
    }

    /// Once reached, the timer stays pending until the connection has served
    /// another request, so a loop that keeps polling it does not pick it on
    /// every pass.
    #[tokio::test]
    async fn a_timer_is_ready_once_per_idle_period() {
        const IDLE: Duration = Duration::from_millis(50);

        let timer = idle::idle_sleep(Some(IDLE));
        let mut timer = std::pin::pin!(timer);

        (&mut timer).await;
        assert!(
            tokio::time::timeout(IDLE * 10, &mut timer).await.is_err(),
            "a timer must not be ready twice for one idle period"
        );
    }

    /// A request can start and finish between two polls of the timer. It must
    /// still push the deadline back to a full period from its end.
    #[tokio::test]
    async fn a_request_shorter_than_a_poll_pushes_the_deadline_back() {
        const IDLE: Duration = Duration::from_millis(100);

        let timer = idle::idle_sleep(Some(IDLE));
        let handle = timer.handle();
        let mut timer = std::pin::pin!(timer);

        // Polled once, so it is waiting on the deadline counted from creation.
        assert!(
            tokio::time::timeout(IDLE / 2, &mut timer).await.is_err(),
            "the deadline must not be reached early"
        );

        let became_idle = tokio::time::Instant::now();
        drop(handle.guard());

        tokio::time::timeout(IDLE * 10, &mut timer)
            .await
            .expect("the connection must go idle after the request");
        assert!(
            became_idle.elapsed() >= IDLE,
            "the deadline must run from the end of the request, elapsed only {:?}",
            became_idle.elapsed()
        );
    }

    /// A request can start and finish between two polls of a timer that has
    /// already been reached. The connection has been used again since, so the
    /// timer must be ready again a full period after the request ends.
    #[tokio::test]
    async fn a_request_shorter_than_a_poll_makes_a_reached_timer_ready_again() {
        const IDLE: Duration = Duration::from_millis(100);

        let timer = idle::idle_sleep(Some(IDLE));
        let handle = timer.handle();
        let mut timer = std::pin::pin!(timer);

        (&mut timer).await;

        let became_idle = tokio::time::Instant::now();
        drop(handle.guard());

        tokio::time::timeout(IDLE * 10, &mut timer)
            .await
            .expect("a connection used after its timer was reached must go idle again");
        assert!(
            became_idle.elapsed() >= IDLE,
            "the deadline must run from the end of the request, elapsed only {:?}",
            became_idle.elapsed()
        );
    }

    /// A reached timer can be waiting while a request starts and ends on
    /// another task, as a response body does when the task sending it drops
    /// it. The end of that request must wake the timer, or it would not be
    /// polled again until something else woke its task.
    #[tokio::test]
    async fn a_request_ending_on_another_task_wakes_a_reached_timer() {
        const IDLE: Duration = Duration::from_millis(100);

        let timer = idle::idle_sleep(Some(IDLE));
        let handle = timer.handle();
        let mut timer = std::pin::pin!(timer);

        (&mut timer).await;

        let started = tokio::time::Instant::now();
        let request = tokio::spawn(async move {
            tokio::time::sleep(IDLE).await;
            drop(handle.guard());
        });

        tokio::time::timeout(IDLE * 10, &mut timer)
            .await
            .expect("a timer reached before a request must be ready again after it");
        request.await.unwrap();
        // Ready a full period after the request, not when the timeout above
        // happens to poll it again.
        assert!(
            started.elapsed() < IDLE * 5,
            "the end of the request must wake the timer, ready only after {:?}",
            started.elapsed()
        );
    }

    /// A response body with a frame still to come: what a streaming response
    /// looks like between its headers and its last frame.
    struct NeverEnds;

    impl http_body::Body for NeverEnds {
        type Data = bytes::Bytes;
        type Error = BoxError;

        fn poll_frame(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            std::task::Poll::Pending
        }
    }

    /// A streaming response resolves its future as soon as the headers are
    /// ready, while the work continues for as long as frames are produced.
    /// Dropping the guard when the future resolves would call such a
    /// connection idle while it is still sending.
    #[tokio::test]
    async fn a_streaming_response_holds_its_connection() {
        const IDLE: Duration = Duration::from_millis(100);

        let timer = idle::idle_sleep(Some(IDLE));
        let idle = timer.handle();
        let mut timer = std::pin::pin!(timer);

        // What the server does per request: take a guard and hand it to the
        // response body.
        let guard = idle.guard();
        let streaming = body::GuardedBody::new(NeverEnds, guard);

        assert!(
            tokio::time::timeout(IDLE * 10, &mut timer).await.is_err(),
            "a connection still sending its response must not be considered idle"
        );

        // The response ends when its body is dropped.
        drop(streaming);
        assert!(
            tokio::time::timeout(IDLE * 10, &mut timer).await.is_ok(),
            "the connection must go idle once the body ends"
        );
    }

    /// Protocol traffic is not work. A peer that keeps the bytes flowing
    /// without ever sending a request must still be closed, which is why
    /// idleness is measured in requests rather than in bytes.
    #[tokio::test]
    async fn protocol_traffic_alone_does_not_keep_a_connection_alive() {
        use tokio::io::AsyncWriteExt as _;

        const IDLE: Duration = Duration::from_millis(200);
        const EMPTY_SETTINGS: [u8; 9] = [0, 0, 0, 0x4, 0, 0, 0, 0, 0];

        let handle = Builder::new()
            .config(Config::default().max_connection_idle(Some(IDLE)))
            .serve(("localhost", 0), Router::new())
            .unwrap();

        let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();
        connection
            .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
            .await
            .unwrap();

        // Keep sending frames the server must read and answer, but never a
        // request, until the write fails because it closed the connection.
        let chattering = async {
            loop {
                if connection.write_all(&EMPTY_SETTINGS).await.is_err() {
                    return;
                }
                tokio::time::sleep(IDLE / 10).await;
            }
        };
        tokio::time::timeout(IDLE * 50, chattering)
            .await
            .expect("traffic without requests must not hold a connection open");
    }

    /// A connection past its age is dropped even if the peer ignores the
    /// shutdown, because the grace that follows it is bounded. With nothing
    /// in flight it is given that grace only once.
    #[tokio::test]
    async fn a_connection_past_its_age_is_dropped_even_if_it_ignores_the_request() {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        const AGE: Duration = Duration::from_millis(200);
        const EMPTY_SETTINGS: [u8; 9] = [0, 0, 0, 0x4, 0, 0, 0, 0, 0];

        let handle = Builder::new()
            .config(
                Config::default()
                    .max_connection_age(AGE)
                    // Counts its requests, so it can tell it has none in flight.
                    .max_connection_idle(Some(AGE * 100)),
            )
            .serve(("localhost", 0), Router::new())
            .unwrap();

        let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
            .await
            .unwrap();
        // Started HTTP/2, so the server's shutdown waits for the peer to
        // acknowledge it, which this one never does. A peer that has not
        // picked a protocol yet would be closed at once.
        connection
            .write_all(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n")
            .await
            .unwrap();
        connection.write_all(&EMPTY_SETTINGS).await.unwrap();

        // What the server sends first is its own settings and the shutdown,
        // so read until the connection ends rather than just once.
        let closed = async {
            let mut buf = [0u8; 1024];
            while let Ok(read) = connection.read(&mut buf).await {
                if read == 0 {
                    return;
                }
            }
        };
        tokio::time::timeout(AGE + SHUTDOWN_GRACE_PERIOD * 3, closed)
            .await
            .expect("a connection past its age must be dropped");
    }

    /// The grace a closing connection is given is extended while it is still
    /// serving, so reaching the age mid-request does not discard the response
    /// the client is waiting for.
    #[tokio::test]
    async fn a_request_in_flight_when_the_age_expires_is_finished() {
        const AGE: Duration = Duration::from_millis(300);
        // Longer than the grace a connection with nothing in flight gets, so
        // a fixed grace would cut this response.
        let handler = SHUTDOWN_GRACE_PERIOD * 3;
        const MESSAGE: &str = "finished anyway";

        let serving = Arc::new(tokio::sync::Notify::new());
        let app = Router::new().route(
            "/",
            axum::routing::get({
                let serving = serving.clone();
                move || {
                    let serving = serving.clone();
                    async move {
                        serving.notify_one();
                        tokio::time::sleep(handler).await;
                        MESSAGE
                    }
                }
            }),
        );
        let handle = Builder::new()
            .config(
                Config::default()
                    .max_connection_age(AGE)
                    // Counts its requests, so the grace is extended because
                    // one is in flight rather than because it cannot tell.
                    .max_connection_idle(Some(handler * 10))
                    .accept_http1(true),
            )
            .serve(("localhost", 0), app)
            .unwrap();

        let url = format!("http://{}", handle.local_addr());
        let _ = rustls::crypto::ring::default_provider().install_default();
        // No client timeout: once the clock is paused it would be skipped to
        // while the response is still on its way.
        let request = tokio::spawn(async move { reqwest::get(&url).await?.bytes().await });

        // Paused only once the request is being served, as in
        // `assert_an_unfinished_request_is_cut_after_the_last_grace`, so the
        // age, the graces and the handler pass at once.
        serving.notified().await;
        tokio::time::pause();

        let body = request
            .await
            .unwrap()
            .expect("the response must arrive in full, not be cut by the connection closing");
        assert_eq!(body, MESSAGE.as_bytes());
    }

    /// Sends a request that never finishes to a connection that reaches its
    /// age while serving it, and checks the connection is dropped when the
    /// last grace ends and not before.
    ///
    /// Runs on real time until the request is being served, then pauses the
    /// clock. A paused clock skips ahead whenever the runtime has nothing ready,
    /// even while a socket still has data on the way, so it is paused only once
    /// the server is left waiting on its own timers alone.
    async fn assert_an_unfinished_request_is_cut_after_the_last_grace(
        max_connection_idle: Option<Duration>,
    ) {
        /// Sends the time it is dropped. Held by the handler, whose future
        /// HTTP/1 keeps inside the connection, so it is dropped when the
        /// connection is.
        struct SendWhenDropped(Option<tokio::sync::oneshot::Sender<tokio::time::Instant>>);

        impl Drop for SendWhenDropped {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(tokio::time::Instant::now());
                }
            }
        }

        const AGE: Duration = Duration::from_millis(200);
        let last_grace_ends = AGE + SHUTDOWN_GRACE_PERIOD * (MAX_GRACE_PERIODS + 1);

        let (serving_tx, serving_rx) = tokio::sync::oneshot::channel();
        let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
        // The handler is cloned for every request, so the one-shot senders are
        // handed out from behind a lock to the single request this sends.
        let senders = Arc::new(std::sync::Mutex::new(Some((serving_tx, dropped_tx))));
        let app = Router::new().route(
            "/",
            axum::routing::get(move || {
                let senders = senders.lock().unwrap().take();
                async move {
                    let (serving, dropped) = senders.expect("only one request is sent");
                    let _dropped = SendWhenDropped(Some(dropped));
                    let _ = serving.send(());
                    std::future::pending::<&'static str>().await
                }
            }),
        );
        let handle = Builder::new()
            .config(
                Config::default()
                    .max_connection_age(AGE)
                    .max_connection_idle(max_connection_idle)
                    .accept_http1(true),
            )
            .serve(("localhost", 0), app)
            .unwrap();

        let url = format!("http://{}", handle.local_addr());
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Taken before the connection is made, so the age counts from later.
        let started = tokio::time::Instant::now();
        let request = tokio::spawn(async move { reqwest::Client::new().get(&url).send().await });

        serving_rx
            .await
            .expect("the request must reach the handler");
        tokio::time::pause();

        let cut_after = dropped_rx
            .await
            .expect("the handler must be dropped with its connection")
            - started;
        assert!(
            cut_after >= last_grace_ends,
            "the request must be given every grace, cut after only {cut_after:?}"
        );
        assert!(
            cut_after < last_grace_ends + SHUTDOWN_GRACE_PERIOD,
            "the request must be cut once the last grace ends, cut after {cut_after:?}"
        );

        let response = request.await.unwrap();
        assert!(
            response.is_err(),
            "a request that never finishes must not get a response, got {response:?}"
        );
    }

    /// A closing connection still serving a request is given the grace again,
    /// but only so many times, so a request that never finishes is cut.
    #[tokio::test]
    async fn a_request_that_never_finishes_is_cut_after_the_last_grace() {
        assert_an_unfinished_request_is_cut_after_the_last_grace(Some(Duration::from_secs(3600)))
            .await;
    }

    /// A connection with no idle deadline does not count its requests, so it
    /// cannot tell whether it is serving one and is given every grace.
    #[tokio::test]
    async fn a_connection_with_no_idle_deadline_is_given_every_grace() {
        assert_an_unfinished_request_is_cut_after_the_last_grace(None).await;
    }

    /// The listener's own limit is what bounds the connections it serves. A
    /// per-peer limit cannot: it permits one peer's worth per peer, and the
    /// peers are whoever connects.
    #[tokio::test]
    async fn connections_are_capped_for_the_whole_listener() {
        use tokio::io::AsyncReadExt as _;

        const MAX_CONNECTIONS: usize = 4;
        const ATTEMPTS: usize = MAX_CONNECTIONS * 4;

        let handle = Builder::new()
            .config(Config::default().max_connections(NonZeroUsize::new(MAX_CONNECTIONS)))
            .serve(("localhost", 0), Router::new())
            .unwrap();

        // Each connection waits for the server to close it, which only the
        // refused ones see. The served ones stay open while their tasks wait.
        let mut closed_by_server = tokio::task::JoinSet::new();
        for _ in 0..ATTEMPTS {
            let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
                .await
                .unwrap();
            closed_by_server.spawn(async move {
                let mut buf = [0u8; 1];
                let _ = connection.read(&mut buf).await;
            });
        }

        tokio::time::timeout(Duration::from_secs(10), async {
            for _ in 0..ATTEMPTS - MAX_CONNECTIONS {
                closed_by_server.join_next().await;
            }
        })
        .await
        .expect("every connection past the limit must be refused");

        assert_eq!(
            handle.number_of_connections(),
            MAX_CONNECTIONS,
            "the listener must fill to its limit and never past it"
        );
    }

    /// A full listener refuses a new connection before its TLS handshake, so
    /// it spends nothing on a connection it will not serve.
    #[tokio::test]
    async fn a_full_listener_refuses_before_the_tls_handshake() {
        let (server_tls_config, client_tls_config) = test_tls_configs();
        let handle = Builder::new()
            .config(Config::default().max_connections(Some(NonZeroUsize::MIN)))
            .tls_config(server_tls_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();
        let addr = *handle.local_addr();

        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_tls_config));
        let server_name = rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap();
        let connect = || async {
            let io = tokio::net::TcpStream::connect(addr).await.unwrap();
            connector.connect(server_name.clone(), io).await
        };

        let _served = connect()
            .await
            .expect("the first connection must be served");
        // The listener counts it only once the accept loop has taken it from
        // its handshake.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while handle.number_of_connections() < 1 {
            assert!(
                std::time::Instant::now() < deadline,
                "the first connection must be counted"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Refused after its handshake, the client would see the handshake
        // complete and only then the connection close.
        let refused = tokio::time::timeout(Duration::from_secs(10), connect())
            .await
            .expect("a full listener must answer a new peer");
        assert!(
            refused.is_err(),
            "a full listener must not complete a TLS handshake"
        );
    }

    /// A connection still in its TLS handshake counts toward the limit, so
    /// peers connecting at the same moment cannot all be served.
    #[tokio::test]
    async fn a_connection_in_its_handshake_counts_toward_the_limit() {
        let (server_tls_config, client_tls_config) = test_tls_configs();
        let handle = Builder::new()
            .config(
                Config::default()
                    .max_connections(Some(NonZeroUsize::MIN))
                    // Long enough that the stalled peer below keeps its slot
                    // for the whole test.
                    .handshake_timeout(Some(Duration::from_secs(60))),
            )
            .tls_config(server_tls_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();
        let addr = *handle.local_addr();

        // Takes the only slot by never starting its handshake. The listener
        // accepts in the order peers connected, so it is taken first.
        let _stalled = tokio::net::TcpStream::connect(addr).await.unwrap();

        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_tls_config));
        let server_name = rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap();
        let refused = tokio::time::timeout(Duration::from_secs(10), async {
            let io = tokio::net::TcpStream::connect(addr).await.unwrap();
            connector.connect(server_name, io).await
        })
        .await
        .expect("a full listener must answer a new peer");
        assert!(
            refused.is_err(),
            "a connection must be refused while another holds the only slot in its handshake"
        );
    }

    /// Waits until the listener counts `expected` connections.
    async fn wait_for_connections(handle: &ServerHandle, expected: usize, what: &str) {
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while handle.number_of_connections() != expected {
            assert!(std::time::Instant::now() < deadline, "{what}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Every connection in this test comes from the same loopback address.
    #[tokio::test]
    async fn connections_per_client_ip_are_capped() {
        use tokio::io::AsyncReadExt as _;

        const MAX_PER_CLIENT: usize = 2;
        const ATTEMPTS: usize = MAX_PER_CLIENT * 4;

        let handle = Builder::new()
            .config(
                Config::default().max_connections_per_client_ip(NonZeroUsize::new(MAX_PER_CLIENT)),
            )
            .serve(("localhost", 0), Router::new())
            .unwrap();

        // Each connection waits for the server to close it, which only the
        // refused ones see. The served ones stay open while their tasks wait.
        let mut closed_by_server = tokio::task::JoinSet::new();
        for _ in 0..ATTEMPTS {
            let mut connection = tokio::net::TcpStream::connect(handle.local_addr())
                .await
                .unwrap();
            closed_by_server.spawn(async move {
                let mut buf = [0u8; 1];
                let _ = connection.read(&mut buf).await;
            });
        }

        tokio::time::timeout(Duration::from_secs(10), async {
            for _ in 0..ATTEMPTS - MAX_PER_CLIENT {
                closed_by_server.join_next().await;
            }
        })
        .await
        .expect("every connection past the client's limit must be refused");
        assert_eq!(
            handle.number_of_connections(),
            MAX_PER_CLIENT,
            "the client must hold its limit and never more"
        );

        // Closing the served connections gives their slots back.
        closed_by_server.shutdown().await;
        wait_for_connections(&handle, 0, "the closed connections must leave the listener").await;
        let mut held = Vec::new();
        for _ in 0..MAX_PER_CLIENT {
            held.push(
                tokio::net::TcpStream::connect(handle.local_addr())
                    .await
                    .unwrap(),
            );
        }
        wait_for_connections(
            &handle,
            MAX_PER_CLIENT,
            "a client whose connections closed must be served again",
        )
        .await;
    }

    /// A client at its limit is refused before its TLS handshake, and a
    /// connection still in its handshake holds a slot until the handshake
    /// ends.
    #[tokio::test]
    async fn a_client_ip_at_its_limit_is_refused_before_the_tls_handshake() {
        use tokio::io::AsyncReadExt as _;

        let (server_tls_config, client_tls_config) = test_tls_configs();
        let handle = Builder::new()
            .config(
                Config::default()
                    .max_connections_per_client_ip(Some(NonZeroUsize::MIN))
                    .handshake_timeout(Some(Duration::from_millis(500))),
            )
            .tls_config(server_tls_config)
            .serve(("localhost", 0), Router::new())
            .unwrap();
        let addr = *handle.local_addr();

        let connector = tokio_rustls::TlsConnector::from(Arc::new(client_tls_config));
        let server_name = rustls::pki_types::ServerName::try_from(SERVER_NAME).unwrap();
        let connect = || async {
            let io = tokio::net::TcpStream::connect(addr).await.unwrap();
            connector.connect(server_name.clone(), io).await
        };

        // Takes the client's only slot by never starting its handshake. The
        // listener accepts in the order peers connected, so it is taken first.
        let mut stalled = tokio::net::TcpStream::connect(addr).await.unwrap();

        // Refused after its handshake, the client would see the handshake
        // complete and only then the connection close.
        let refused = tokio::time::timeout(Duration::from_secs(10), connect())
            .await
            .expect("a client at its limit must be answered");
        assert!(
            refused.is_err(),
            "a client at its limit must not complete a TLS handshake"
        );

        let mut buf = [0u8; 1];
        tokio::time::timeout(Duration::from_secs(10), stalled.read(&mut buf))
            .await
            .expect("the stalled handshake must time out")
            .ok();
        let _served = tokio::time::timeout(Duration::from_secs(10), connect())
            .await
            .expect("the listener must answer")
            .expect("a handshake that timed out must give its slot back");

        // The served connection keeps its slot once its handshake is done.
        let refused = tokio::time::timeout(Duration::from_secs(10), connect())
            .await
            .expect("a client at its limit must be answered");
        assert!(
            refused.is_err(),
            "a connection past its handshake must still hold its slot"
        );
    }

    /// Counts each connection as a client of its own.
    struct ClientPerPort(tokio::net::TcpListener);

    impl Listener for ClientPerPort {
        type Io = tokio::net::TcpStream;
        type Addr = std::net::SocketAddr;

        async fn accept(&mut self) -> (Self::Io, Self::Addr) {
            Listener::accept(&mut self.0).await
        }

        fn local_addr(&self) -> std::io::Result<Self::Addr> {
            Listener::local_addr(&self.0)
        }

        fn client_prefix(&self, client: &Self::Addr) -> Vec<u8> {
            client.port().to_be_bytes().to_vec()
        }
    }

    #[tokio::test]
    async fn the_listener_decides_which_connections_are_one_client() {
        const CONNECTIONS: usize = 4;

        let listener = ClientPerPort(tokio::net::TcpListener::bind("localhost:0").await.unwrap());
        let handle = Builder::new()
            .config(Config::default().max_connections_per_client_ip(Some(NonZeroUsize::MIN)))
            .serve_with_listener(listener, Router::new())
            .unwrap();

        let mut held = Vec::new();
        for _ in 0..CONNECTIONS {
            held.push(
                tokio::net::TcpStream::connect(handle.local_addr())
                    .await
                    .unwrap(),
            );
        }
        wait_for_connections(
            &handle,
            CONNECTIONS,
            "connections the listener names as different clients must all be served",
        )
        .await;
    }
}
