// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fmt,
    future::Future,
    io,
    net::{SocketAddr, ToSocketAddrs},
    num::NonZeroU32,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{self, Poll},
    time::Instant,
    vec,
};

use eyre::{Context, Result, eyre};
use hyper_timeout::TimeoutConnector;
use hyper_util::client::legacy::connect::{HttpConnector, dns::Name};
use once_cell::sync::OnceCell;
use tokio::task::JoinHandle;
use tokio_rustls::rustls::ClientConfig;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::Service;
use tracing::{info, trace};

use crate::{
    config::Config,
    multiaddr::{Multiaddr, Protocol, parse_dns, parse_ip4, parse_ip6},
};

pub async fn connect(address: &Multiaddr, tls_config: ClientConfig) -> Result<Channel> {
    let channel = endpoint_from_multiaddr(address, tls_config)?
        .connect()
        .await?;
    Ok(channel)
}

pub fn connect_lazy(address: &Multiaddr, tls_config: ClientConfig) -> Result<Channel> {
    let channel = endpoint_from_multiaddr(address, tls_config)?.connect_lazy();
    Ok(channel)
}

pub(crate) async fn connect_with_config(
    address: &Multiaddr,
    tls_config: ClientConfig,
    config: &Config,
) -> Result<Channel> {
    let channel = endpoint_from_multiaddr(address, tls_config)?
        .apply_config(config)
        .connect()
        .await?;
    Ok(channel)
}

pub(crate) fn connect_lazy_with_config(
    address: &Multiaddr,
    tls_config: ClientConfig,
    config: &Config,
) -> Result<Channel> {
    let channel = endpoint_from_multiaddr(address, tls_config)?
        .apply_config(config)
        .connect_lazy();
    Ok(channel)
}

fn endpoint_from_multiaddr(addr: &Multiaddr, tls_config: ClientConfig) -> Result<MyEndpoint> {
    let mut iter = addr.iter();

    let channel = match iter.next().ok_or_else(|| eyre!("address is empty"))? {
        Protocol::Dns(_) => {
            let (dns_name, tcp_port, http_or_https) = parse_dns(addr)?;
            let uri = format!("{http_or_https}://{dns_name}:{tcp_port}");
            MyEndpoint::try_from_uri(uri, tls_config)?
        }
        Protocol::Ip4(_) => {
            let (socket_addr, http_or_https) = parse_ip4(addr)?;
            let uri = format!("{http_or_https}://{socket_addr}");
            MyEndpoint::try_from_uri(uri, tls_config)?
        }
        Protocol::Ip6(_) => {
            let (socket_addr, http_or_https) = parse_ip6(addr)?;
            let uri = format!("{http_or_https}://{socket_addr}");
            MyEndpoint::try_from_uri(uri, tls_config)?
        }
        unsupported => return Err(eyre!("unsupported protocol {unsupported}")),
    };

    Ok(channel)
}

struct MyEndpoint {
    endpoint: Endpoint,
    tls_config: ClientConfig,
}

static DISABLE_CACHING_RESOLVER: OnceCell<bool> = OnceCell::new();

impl MyEndpoint {
    fn new(endpoint: Endpoint, tls_config: ClientConfig) -> Self {
        Self {
            endpoint,
            tls_config,
        }
    }

    fn try_from_uri(uri: String, tls_config: ClientConfig) -> Result<Self> {
        let uri: Uri = uri
            .parse()
            .with_context(|| format!("unable to create Uri from '{uri}'"))?;
        let endpoint = Endpoint::from(uri);
        Ok(Self::new(endpoint, tls_config))
    }

    fn apply_config(mut self, config: &Config) -> Self {
        self.endpoint = apply_config_to_endpoint(config, self.endpoint);
        self
    }

    fn connect_lazy(self) -> Channel {
        let disable_caching_resolver = *DISABLE_CACHING_RESOLVER.get_or_init(|| {
            let disable_caching_resolver = std::env::var("DISABLE_CACHING_RESOLVER").is_ok();
            info!("DISABLE_CACHING_RESOLVER: {disable_caching_resolver}");
            disable_caching_resolver
        });

        // Apply the endpoint's configured connect timeout (Config::connect_timeout
        // applied in apply_config_to_endpoint) to the connectors, as tonic's
        // Endpoint::connect_with_connector_lazy does. tonic's Channel::new path does
        // not apply Endpoint::connect_timeout to a provided connector, so without this
        // the lazy channel can sit in State::Connecting forever when a peer is
        // unreachable (msim with dropped SYNs, real network partitions, etc) or accepts
        // TCP and never completes the TLS handshake, blocking every gRPC call on that
        // channel. The HTTP connector's timeout bounds each TCP connect attempt; the
        // TimeoutConnector bounds the whole attempt, TLS handshake included.
        // connect_with_connector_lazy itself can't be used here: once any crate in the
        // build enables a tonic TLS feature, it rejects an https URI unless tonic does
        // the TLS.
        let connect_timeout = self.endpoint.get_connect_timeout();

        if disable_caching_resolver {
            let mut http = HttpConnector::new();
            http.enforce_http(false);
            http.set_nodelay(true);
            http.set_keepalive(None);
            http.set_connect_timeout(connect_timeout);

            let mut connector = TimeoutConnector::new(
                hyper_rustls::HttpsConnectorBuilder::new()
                    .with_tls_config(self.tls_config)
                    .https_only()
                    .enable_http2()
                    .wrap_connector(http),
            );
            connector.set_connect_timeout(connect_timeout);
            Channel::new(connector, self.endpoint)
        } else {
            let mut http = HttpConnector::new_with_resolver(CachingResolver::new());
            http.enforce_http(false);
            http.set_nodelay(true);
            http.set_keepalive(None);
            http.set_connect_timeout(connect_timeout);

            let https = hyper_rustls::HttpsConnectorBuilder::new()
                .with_tls_config(self.tls_config)
                .https_only()
                .enable_http2()
                .wrap_connector(http);
            let mut connector = TimeoutConnector::new(https);
            connector.set_connect_timeout(connect_timeout);
            Channel::new(connector, self.endpoint)
        }
    }

    async fn connect(self) -> Result<Channel> {
        let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(self.tls_config)
            .https_only()
            .enable_http2()
            .build();
        Channel::connect(https_connector, self.endpoint)
            .await
            .map_err(Into::into)
    }
}

fn apply_config_to_endpoint(config: &Config, mut endpoint: Endpoint) -> Endpoint {
    if let Some(limit) = config.concurrency_limit_per_connection {
        endpoint = endpoint.concurrency_limit(limit);
    }

    if let Some(timeout) = config.request_timeout {
        endpoint = endpoint.timeout(timeout);
    }

    if let Some(timeout) = config.connect_timeout {
        endpoint = endpoint.connect_timeout(timeout);
    }

    if let Some(http2_keepalive_interval) = config.http2_keepalive_interval {
        endpoint = endpoint.http2_keep_alive_interval(http2_keepalive_interval);
    }

    if let Some(http2_keepalive_timeout) = config.http2_keepalive_timeout {
        endpoint = endpoint.keep_alive_timeout(http2_keepalive_timeout);
    }

    if let Some((limit, duration)) = config.rate_limit {
        endpoint = endpoint.rate_limit(limit, duration);
    }

    endpoint
        .initial_stream_window_size(config.http2_initial_stream_window_size.map(NonZeroU32::get))
        .initial_connection_window_size(
            config
                .http2_initial_connection_window_size
                .map(NonZeroU32::get),
        )
        .tcp_keepalive(config.tcp_keepalive)
}

type CacheEntry = (Instant, Vec<SocketAddr>);

/// A caching resolver based on hyper_util GaiResolver
#[derive(Clone)]
pub struct CachingResolver {
    cache: Arc<Mutex<HashMap<Name, CacheEntry>>>,
}

type SocketAddrs = vec::IntoIter<SocketAddr>;

pub struct CachingFuture {
    inner: JoinHandle<Result<SocketAddrs, io::Error>>,
}

impl CachingResolver {
    pub fn new() -> Self {
        CachingResolver {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for CachingResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Service<Name> for CachingResolver {
    type Response = SocketAddrs;
    type Error = io::Error;
    type Future = CachingFuture;

    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: Name) -> Self::Future {
        let blocking = {
            let cache = self.cache.clone();
            tokio::task::spawn_blocking(move || {
                let entry = cache.lock().unwrap().get(&name).cloned();

                if let Some((when, addrs)) = entry {
                    trace!("cached host={:?}", name.as_str());

                    if when.elapsed().as_secs() > 60 {
                        trace!("refreshing cache for host={:?}", name.as_str());
                        // Start a new task to update the cache later.
                        tokio::task::spawn_blocking(move || {
                            if let Ok(addrs) = (name.as_str(), 0).to_socket_addrs() {
                                let addrs: Vec<_> = addrs.collect();
                                trace!("updating cached host={:?}", name.as_str());
                                cache.lock().unwrap().insert(name, (Instant::now(), addrs));
                            }
                        });
                    }

                    Ok(addrs.into_iter())
                } else {
                    trace!("resolving host={:?}", name.as_str());
                    match (name.as_str(), 0).to_socket_addrs() {
                        Ok(addrs) => {
                            let addrs: Vec<_> = addrs.collect();
                            cache
                                .lock()
                                .unwrap()
                                .insert(name, (Instant::now(), addrs.clone()));
                            Ok(addrs.into_iter())
                        }
                        res => res,
                    }
                }
            })
        };

        CachingFuture { inner: blocking }
    }
}

impl fmt::Debug for CachingResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("CachingResolver")
    }
}

impl Future for CachingFuture {
    type Output = Result<SocketAddrs, io::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx).map(|res| match res {
            Ok(Ok(addrs)) => Ok(addrs),
            Ok(Err(err)) => Err(err),
            Err(join_err) => {
                if join_err.is_cancelled() {
                    Err(io::Error::new(io::ErrorKind::Interrupted, join_err))
                } else {
                    panic!("background task failed: {join_err:?}")
                }
            }
        })
    }
}

impl fmt::Debug for CachingFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("CachingFuture")
    }
}

impl Drop for CachingFuture {
    fn drop(&mut self) {
        self.inner.abort();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use iota_sdk_crypto::ed25519::Ed25519PrivateKey;
    use tokio::net::TcpListener;
    use tonic_health::pb::{HealthCheckRequest, health_client::HealthClient};

    use crate::{Multiaddr, config::Config};

    const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
    /// How long a test waits for a call before deciding that no deadline
    /// ended it. Far longer than `CONNECT_TIMEOUT`, so a slow machine does not
    /// fail the tests.
    const GIVE_UP: Duration = Duration::from_secs(10);

    fn config() -> Config {
        let mut config = Config::new();
        config.connect_timeout = Some(CONNECT_TIMEOUT);
        config
    }

    /// Starts a peer that accepts every TCP connection, keeps it open and never
    /// sends a byte, so no TLS handshake with it finishes. Returns its address
    /// and the number of connections it has accepted.
    async fn peer_that_never_answers_tls() -> (Multiaddr, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = Arc::new(AtomicUsize::new(0));
        tokio::spawn({
            let accepted = accepted.clone();
            async move {
                let mut open = Vec::new();
                while let Ok((stream, _)) = listener.accept().await {
                    accepted.fetch_add(1, Ordering::SeqCst);
                    open.push(stream);
                }
            }
        });
        let address = format!("/ip4/127.0.0.1/tcp/{port}/https").parse().unwrap();
        (address, accepted)
    }

    async fn check(client: &mut HealthClient<tonic::transport::Channel>) -> Result<(), String> {
        tokio::time::timeout(GIVE_UP, client.check(HealthCheckRequest::default()))
            .await
            .map_err(|_| format!("the call was still pending after {GIVE_UP:?}"))?
            .map(drop)
            .map_err(|status| status.to_string())
    }

    /// A lazy channel to a peer that accepts TCP but never answers the TLS
    /// handshake fails each call once the connect timeout passes, and dials
    /// again for the next call.
    #[tokio::test]
    async fn lazy_channel_gives_up_on_a_peer_that_stalls_the_tls_handshake() {
        let (address, accepted) = peer_that_never_answers_tls().await;
        let channel = config()
            .connect_lazy(
                &address,
                iota_tls::create_rustls_client_config(
                    Ed25519PrivateKey::random().public_key(),
                    "test".to_string(),
                    None,
                ),
            )
            .unwrap();
        let mut client = HealthClient::new(channel);

        for call in 1..=2 {
            let started = tokio::time::Instant::now();
            let result = check(&mut client).await;
            assert!(
                result
                    .as_ref()
                    .is_err_and(|err| !err.starts_with("the call was still pending")),
                "call {call} must fail at the connect timeout, got {result:?}"
            );
            assert!(
                started.elapsed() >= CONNECT_TIMEOUT,
                "call {call} failed after {:?}, before the connect timeout: {result:?}",
                started.elapsed()
            );
        }
        assert_eq!(
            accepted.load(Ordering::SeqCst),
            2,
            "the channel must dial again after a stalled handshake"
        );
    }

    /// A lazy channel with a connect timeout still completes calls to a TLS
    /// server.
    #[tokio::test]
    async fn lazy_channel_with_a_connect_timeout_reaches_a_tls_server() {
        let private_key = Ed25519PrivateKey::random();
        let config = config();
        let server = config
            .server_builder()
            .bind(
                &"/ip4/127.0.0.1/tcp/0/http".parse().unwrap(),
                Some(iota_tls::create_rustls_server_config(
                    private_key.clone(),
                    "test".to_string(),
                )),
            )
            .await
            .unwrap();

        let channel = config
            .connect_lazy(
                server.local_addr(),
                iota_tls::create_rustls_client_config(
                    private_key.public_key(),
                    "test".to_string(),
                    None,
                ),
            )
            .unwrap();
        let mut client = HealthClient::new(channel);
        check(&mut client).await.unwrap();

        server.trigger_shutdown();
    }
}
