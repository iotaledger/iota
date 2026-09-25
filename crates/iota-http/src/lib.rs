// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc, time::Duration};

use connection_handler::{OnConnectionClose, sleep_or_pending};
pub use http;
use http::{Request, Response};
use hyper_util::service::TowerToHyperService;
use io::ServerIo;
use tokio::task::JoinSet;
use tokio_rustls::{TlsAcceptor, rustls};
use tower::{Service, ServiceBuilder, ServiceExt};
use tracing::trace;

use self::{
    body::BoxBody,
    connection_info::{ActiveConnections, PeerConnectionCounts},
};

pub mod body;
mod config;
mod connection_handler;
mod connection_info;
mod fuse;
mod io;
mod listener;

pub use config::{Config, PeerConnectionEvent};
pub use connection_info::{ConnectInfo, ConnectionId, ConnectionInfo, PeerCertificates};
pub use listener::{Listener, ListenerExt};

pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;
/// h2 alpn in plain format for rustls.
const ALPN_H2: &[u8] = b"h2";
/// h1 alpn in plain format for rustls.
const ALPN_H1: &[u8] = b"http/1.1";

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
            self.config.tcp_nodelay,
            self.config.tcp_keepalive,
        )?;

        Self::serve_with_listener(self, listener, service)
    }

    fn serve_with_listener<L, S, ResponseBody>(
        self,
        listener: L,
        service: S,
    ) -> Result<ServerHandle<L::Addr>, BoxError>
    where
        L: Listener,
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

        let tls_config = self.tls_config.map(|mut tls| {
            tls.alpn_protocols.push(ALPN_H2.into());
            if self.config.accept_http1 {
                tls.alpn_protocols.push(ALPN_H1.into());
            }
            Arc::new(tls)
        });

        let (watch_sender, watch_receiver) = tokio::sync::watch::channel(());
        let peer_connection_counts =
            PeerConnectionCounts::new(self.config.on_peer_connection_event.clone());
        let server = Server {
            config: self.config,
            tls_config,
            listener,
            local_addr: local_addr.clone(),
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
            peer_connection_counts,
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
    /// server has completed shutting down
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

type ConnectingOutput<Io, Addr> = Result<(ServerIo<Io>, Addr), crate::BoxError>;

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
    peer_connection_counts: PeerConnectionCounts,
}

impl<L> Server<L>
where
    L: Listener,
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
                (io, remote_addr) = self.listener.accept(), if self.accepts_more_connections() => {
                    self.handle_incoming(io, remote_addr);
                },
                // A failed task affects only its own connection, so the loop keeps serving
                // the others.
                Some(maybe_connection) = self.pending_connections.join_next() => {
                    let (io, remote_addr) = match maybe_connection {
                        Ok(Ok((io, remote_addr))) => {
                            (io, remote_addr)
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
                    self.handle_connection(io, remote_addr);
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

    /// Whether another connection may be accepted, or the limit on concurrent
    /// TLS handshakes is currently reached.
    fn accepts_more_connections(&self) -> bool {
        self.config
            .max_pending_connections
            .is_none_or(|max| self.pending_connections.len() < max)
    }

    fn handle_incoming(&mut self, io: L::Io, remote_addr: L::Addr) {
        if let Some(tls) = self.tls_config.clone() {
            let tls_acceptor = TlsAcceptor::from(tls);
            let handshake_timeout = self.config.handshake_timeout;
            self.pending_connections.spawn(async move {
                tokio::select! {
                    result = handshake(io, remote_addr, tls_acceptor) => result,
                    // Dropping the handshake closes the connection, releasing its file descriptor.
                    _ = sleep_or_pending(handshake_timeout) => Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "TLS handshake timed out",
                    )
                    .into()),
                }
            });
        } else {
            self.handle_connection(ServerIo::new_io(io), remote_addr);
        }
    }

    fn handle_connection(&mut self, io: ServerIo<L::Io>, remote_addr: L::Addr) {
        let mut peer_connection_guard = None;
        if let (Some(max), Some(peer)) =
            (self.config.max_connections_per_peer, peer_public_key(&io))
        {
            let Some(guard) = self.peer_connection_counts.register(&peer, max) else {
                // Dropping the connection closes it, releasing its file descriptor.
                trace!("peer already holds {max} connections, closing the new one");
                return;
            };
            peer_connection_guard = Some(guard);
        }

        let connection_shutdown_token = self.graceful_shutdown_token.child_token();
        let connection_info = ConnectionInfo::new(
            remote_addr,
            io.peer_certs(),
            connection_shutdown_token.clone(),
        );
        let connection_id = connection_info.id();
        let connect_info = connection_info::ConnectInfo {
            local_addr: self.local_addr.clone(),
            remote_addr: connection_info.remote_address().clone(),
        };
        let peer_certificates = connection_info.peer_certificates().cloned();
        let hyper_io = hyper_util::rt::TokioIo::new(io);

        let hyper_svc = TowerToHyperService::new(self.service.clone().map_request(
            move |mut request: Request<hyper::body::Incoming>| {
                request.extensions_mut().insert(connect_info.clone());
                if let Some(peer_certificates) = peer_certificates.clone() {
                    request.extensions_mut().insert(peer_certificates);
                }

                request.map(body::boxed)
            },
        ));

        self.connections
            .write()
            .unwrap()
            .insert(connection_id, connection_info);
        let on_connection_close = OnConnectionClose::new(
            connection_id,
            self.connections.clone(),
            peer_connection_guard,
        );

        self.connection_handlers
            .spawn(connection_handler::serve_connection(
                hyper_io,
                hyper_svc,
                self.config.connection_builder(),
                connection_shutdown_token,
                self.config.max_connection_age,
                on_connection_close,
            ));
    }

    async fn shutdown(mut self) {
        // The time we are willing to wait for a connection to get gracefully shutdown
        // before we attempt to forcefully shutdown all active connections
        const CONNECTION_SHUTDOWN_GRACE_PERIOD: Duration = Duration::from_secs(1);

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

        if tokio::time::timeout(CONNECTION_SHUTDOWN_GRACE_PERIOD, graceful_shutdown)
            .await
            .is_err()
        {
            tracing::warn!(
                "Failed to stop all connection handlers in {:?}. Forcing shutdown.",
                CONNECTION_SHUTDOWN_GRACE_PERIOD
            );
            self.connection_handlers.shutdown().await;
        }
    }
}

/// Runs the TLS handshake to completion.
async fn handshake<Io, Addr>(
    io: Io,
    remote_addr: Addr,
    tls_acceptor: TlsAcceptor,
) -> ConnectingOutput<Io, Addr>
where
    Io: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    tracing::trace!("accepting TLS connection");
    let io = tls_acceptor.accept(io).await?;
    Ok((ServerIo::new_tls_io(io), remote_addr))
}

/// Identifies the peer by the public key of the single certificate it
/// authenticated with, or `None` if it presented no certificate.
fn peer_public_key<Io>(io: &ServerIo<Io>) -> Option<Vec<u8>> {
    let certs = io.peer_certs()?;
    let [certificate] = certs.as_slice() else {
        trace!("unexpected number of peer certificates: {}", certs.len());
        return None;
    };

    match iota_tls::public_key_from_certificate(certificate) {
        Ok(public_key) => Some(AsRef::<[u8]>::as_ref(&public_key).to_vec()),
        Err(e) => {
            trace!("failed to extract the public key from the peer certificate: {e:?}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::Router;

    use super::*;

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
                    .max_pending_connections(Some(1)),
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
                    .max_connections_per_peer(Some(MAX_PER_PEER))
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
            .config(Config::default().max_connections_per_peer(Some(1)))
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

    /// A limit the accept loop can never fall below, and a limit whose slots
    /// nothing releases, both leave the server unable to accept.
    #[tokio::test]
    async fn unrecoverable_limits_are_rejected() {
        let served = |config| {
            Builder::new()
                .config(config)
                .tls_config(test_tls_configs().0)
                .serve(("localhost", 0), Router::new())
        };

        assert!(
            served(Config::default().max_pending_connections(Some(0))).is_err(),
            "a zero limit must be rejected"
        );
        assert!(
            served(
                Config::default()
                    .handshake_timeout(None)
                    .max_pending_connections(Some(8))
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
}
