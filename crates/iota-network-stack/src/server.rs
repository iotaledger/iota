// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    convert::Infallible,
    num::NonZeroUsize,
    task::{Context, Poll},
    time::Duration,
};

use eyre::{Result, eyre};
use tokio_rustls::rustls::ServerConfig;
use tonic::{
    body::Body,
    codegen::http::{HeaderValue, Request, Response},
    server::NamedService,
};
use tower::{Layer, Service, ServiceBuilder};
use tower_http::{
    propagate_header::PropagateHeaderLayer, set_header::SetRequestHeaderLayer, trace::TraceLayer,
};

use crate::{
    concurrency::ServiceConcurrencyLimit,
    config::Config,
    metrics::{
        DefaultMetricsCallbackProvider, GRPC_ENDPOINT_PATH_HEADER, MetricsCallbackProvider,
        MetricsHandler,
    },
    multiaddr::{Multiaddr, Protocol},
};

/// Server-side deadline for a request whose config sets no `request_timeout`.
///
/// A request holds its admission slot from the moment its headers arrive
/// until the handler responds, including the time spent receiving the body,
/// so without a deadline a peer that never finishes sending pins the slot
/// indefinitely.
pub const DEFAULT_GRPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

pub struct ServerBuilder<M: MetricsCallbackProvider = DefaultMetricsCallbackProvider> {
    config: Config,
    metrics_provider: M,
    router: tonic::service::Routes,
    health_reporter: tonic_health::server::HealthReporter,
}

impl<M: MetricsCallbackProvider> ServerBuilder<M> {
    pub fn from_config(config: &Config, metrics_provider: M) -> Self {
        let (health_reporter, health_service) = tonic_health::server::health_reporter();
        let router = tonic::service::Routes::new(health_service);

        Self {
            config: config.to_owned(),
            metrics_provider,
            router,
            health_reporter,
        }
    }

    pub fn health_reporter(&self) -> tonic_health::server::HealthReporter {
        self.health_reporter.clone()
    }

    /// The server-side request deadline this server enforces, unless a
    /// request carries a shorter `grpc-timeout` header.
    pub fn request_timeout(&self) -> Duration {
        self.config
            .request_timeout
            .unwrap_or(DEFAULT_GRPC_REQUEST_TIMEOUT)
    }

    /// Add a new service to this Server.
    pub fn add_service<S>(mut self, svc: S) -> Self
    where
        S: Service<Request<Body>, Response = Response<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Future: Send + 'static,
    {
        self.router = self.router.add_service(svc);
        self
    }

    /// Add a new service to this Server with its own concurrency limit,
    /// enforced independently of every other service on this server.
    ///
    /// With `load_shed` enabled, requests over the limit are rejected
    /// immediately with gRPC `RESOURCE_EXHAUSTED`; otherwise they wait for a
    /// slot to free up.
    pub fn add_service_with_concurrency_limit<S>(
        mut self,
        svc: S,
        limit: NonZeroUsize,
        load_shed: bool,
    ) -> Self
    where
        S: Service<Request<Body>, Response = Response<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Future: Send + 'static,
    {
        self.router = self
            .router
            .add_service(ServiceConcurrencyLimit::new(svc, limit, load_shed));
        self
    }

    pub async fn bind(self, addr: &Multiaddr, tls_config: Option<ServerConfig>) -> Result<Server> {
        let http_config = self.config.http_config();

        let request_timeout = self.request_timeout();
        let metrics_provider = self.metrics_provider;
        let metrics = MetricsHandler::new(metrics_provider.clone());
        let request_metrics = TraceLayer::new_for_grpc()
            .on_request(metrics.clone())
            .on_response(metrics.clone())
            .on_failure(metrics);

        fn add_path_to_request_header<T>(request: &Request<T>) -> Option<HeaderValue> {
            let path = request.uri().path();
            Some(HeaderValue::from_str(path).unwrap())
        }

        let limiting_layers = ServiceBuilder::new()
            .option_layer(
                self.config
                    .load_shed
                    .unwrap_or_default()
                    .then_some(tower::load_shed::LoadShedLayer::new()),
            )
            .option_layer(
                self.config
                    .global_concurrency_limit
                    .map(tower::limit::GlobalConcurrencyLimitLayer::new),
            );

        let route_layers = ServiceBuilder::new()
            .map_request(|mut request: http::Request<_>| {
                if let Some(connect_info) = request.extensions().get::<iota_http::ConnectInfo>() {
                    let tonic_connect_info = tonic::transport::server::TcpConnectInfo {
                        local_addr: Some(connect_info.local_addr),
                        remote_addr: Some(connect_info.remote_addr),
                    };
                    request.extensions_mut().insert(tonic_connect_info);
                }
                request
            })
            .layer(RequestLifetimeLayer { metrics_provider })
            .layer(SetRequestHeaderLayer::overriding(
                GRPC_ENDPOINT_PATH_HEADER.clone(),
                add_path_to_request_header,
            ))
            .layer(request_metrics)
            .layer(PropagateHeaderLayer::new(GRPC_ENDPOINT_PATH_HEADER.clone()))
            .layer_fn(move |service| {
                crate::grpc_timeout::GrpcTimeout::new(service, Some(request_timeout))
            });

        let mut builder = iota_http::Builder::new().config(http_config);

        let has_tls = tls_config.is_some();
        if let Some(tls_config) = tls_config {
            builder = builder.tls_config(tls_config);
        }

        let server_handle = builder
            .serve(
                addr,
                limiting_layers.service(self.router.into_axum_router().layer(route_layers)),
            )
            .map_err(|e| eyre!(e))?;

        let mut local_addr = update_tcp_port_in_multiaddr(addr, server_handle.local_addr().port());
        if has_tls {
            local_addr = local_addr.rewrite_http_to_https();
        }
        Ok(Server {
            server_handle,
            local_addr,
            health_reporter: self.health_reporter,
        })
    }
}

/// TLS server name to use for the public IOTA validator interface.
pub const IOTA_TLS_SERVER_NAME: &str = "iota";

pub struct Server {
    server_handle: iota_http::ServerHandle,
    local_addr: Multiaddr,
    health_reporter: tonic_health::server::HealthReporter,
}

impl Server {
    pub async fn serve(self) -> Result<(), tonic::transport::Error> {
        self.server_handle.wait_for_shutdown().await;
        Ok(())
    }

    pub fn trigger_shutdown(&self) {
        self.server_handle.trigger_shutdown();
    }

    pub fn local_addr(&self) -> &Multiaddr {
        &self.local_addr
    }

    pub fn health_reporter(&self) -> tonic_health::server::HealthReporter {
        self.health_reporter.clone()
    }

    pub fn handle(&self) -> &iota_http::ServerHandle {
        &self.server_handle
    }
}

fn update_tcp_port_in_multiaddr(addr: &Multiaddr, port: u16) -> Multiaddr {
    addr.replace(1, |protocol| {
        if let Protocol::Tcp(_) = protocol {
            Some(Protocol::Tcp(port))
        } else {
            panic!("expected tcp protocol at index 1");
        }
    })
    .expect("tcp protocol at index 1")
}

#[cfg(test)]
mod test {
    use std::{
        ops::Deref,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use iota_sdk_crypto::ed25519::Ed25519PrivateKey;
    use tonic::Code;
    use tonic_health::pb::{HealthCheckRequest, health_client::HealthClient};

    use crate::{Multiaddr, config::Config, metrics::MetricsCallbackProvider};

    #[tokio::test]
    async fn test_metrics_layer_successful() {
        #[derive(Clone)]
        struct Metrics {
            /// a flag to figure out whether the
            /// on_request method has been called.
            metrics_called: Arc<Mutex<bool>>,
        }

        impl MetricsCallbackProvider for Metrics {
            fn on_request(&self, path: String) {
                assert_eq!(path, "/grpc.health.v1.Health/Check");
            }

            fn on_response(
                &self,
                path: String,
                _latency: Duration,
                status: u16,
                grpc_status_code: Code,
            ) {
                assert_eq!(path, "/grpc.health.v1.Health/Check");
                assert_eq!(status, 200);
                assert_eq!(grpc_status_code, Code::Ok);
                let mut m = self.metrics_called.lock().unwrap();
                *m = true
            }
        }

        let metrics = Metrics {
            metrics_called: Arc::new(Mutex::new(false)),
        };

        let address: Multiaddr = "/ip4/127.0.0.1/tcp/0/http".parse().unwrap();
        let config = Config::new();
        let private_key = Ed25519PrivateKey::random();

        let server = config
            .server_builder_with_metrics(metrics.clone())
            .bind(
                &address,
                Some(iota_tls::create_rustls_server_config(
                    private_key.clone(),
                    "test".to_string(),
                )),
            )
            .await
            .unwrap();

        let address = server.local_addr().to_owned();
        let channel = config
            .connect(
                &address,
                iota_tls::create_rustls_client_config(
                    private_key.public_key(),
                    "test".to_string(),
                    None,
                ),
            )
            .await
            .unwrap();
        let mut client = HealthClient::new(channel);

        client
            .check(HealthCheckRequest {
                service: "".to_owned(),
            })
            .await
            .unwrap();

        server.server_handle.shutdown().await;

        assert!(metrics.metrics_called.lock().unwrap().deref());
    }

    #[tokio::test]
    async fn test_metrics_layer_error() {
        #[derive(Clone)]
        struct Metrics {
            /// a flag to figure out whether the
            /// on_request method has been called.
            metrics_called: Arc<Mutex<bool>>,
        }

        impl MetricsCallbackProvider for Metrics {
            fn on_request(&self, path: String) {
                assert_eq!(path, "/grpc.health.v1.Health/Check");
            }

            fn on_response(
                &self,
                path: String,
                _latency: Duration,
                status: u16,
                grpc_status_code: Code,
            ) {
                assert_eq!(path, "/grpc.health.v1.Health/Check");
                assert_eq!(status, 200);
                // According to https://github.com/grpc/grpc/blob/master/doc/statuscodes.md#status-codes-and-their-use-in-grpc
                // code 5 is not_found , which is what we expect to get in this case
                assert_eq!(grpc_status_code, Code::NotFound);
                let mut m = self.metrics_called.lock().unwrap();
                *m = true
            }
        }

        let metrics = Metrics {
            metrics_called: Arc::new(Mutex::new(false)),
        };

        let address: Multiaddr = "/ip4/127.0.0.1/tcp/0/http".parse().unwrap();
        let config = Config::new();
        let private_key = Ed25519PrivateKey::random();

        let server = config
            .server_builder_with_metrics(metrics.clone())
            .bind(
                &address,
                Some(iota_tls::create_rustls_server_config(
                    private_key.clone(),
                    "test".to_string(),
                )),
            )
            .await
            .unwrap();
        let address = server.local_addr().to_owned();
        let channel = config
            .connect(
                &address,
                iota_tls::create_rustls_client_config(
                    private_key.public_key(),
                    "test".to_string(),
                    None,
                ),
            )
            .await
            .unwrap();
        let mut client = HealthClient::new(channel);

        // Call the healthcheck for a service that doesn't exist
        // that should give us back an error with code 5 (not_found)
        // https://github.com/grpc/grpc/blob/master/doc/statuscodes.md#status-codes-and-their-use-in-grpc
        let _ = client
            .check(HealthCheckRequest {
                service: "non-existing-service".to_owned(),
            })
            .await;

        server.server_handle.shutdown().await;

        assert!(metrics.metrics_called.lock().unwrap().deref());
    }

    async fn test_multiaddr(address: Multiaddr) {
        let config = Config::new();
        let private_key = Ed25519PrivateKey::random();

        let server_handle = config
            .server_builder()
            .bind(
                &address,
                Some(iota_tls::create_rustls_server_config(
                    private_key.clone(),
                    "test".to_string(),
                )),
            )
            .await
            .unwrap();
        let address = server_handle.local_addr().to_owned();
        let channel = config
            .connect(
                &address,
                iota_tls::create_rustls_client_config(
                    private_key.public_key(),
                    "test".to_string(),
                    None,
                ),
            )
            .await
            .unwrap();
        let mut client = HealthClient::new(channel);

        client
            .check(HealthCheckRequest {
                service: "".to_owned(),
            })
            .await
            .unwrap();

        server_handle.server_handle.shutdown().await;
    }

    #[tokio::test]
    async fn dns() {
        let address: Multiaddr = "/dns/localhost/tcp/0/http".parse().unwrap();
        test_multiaddr(address).await;
    }

    #[tokio::test]
    async fn ip4() {
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/0/http".parse().unwrap();
        test_multiaddr(address).await;
    }

    #[tokio::test]
    async fn ip6() {
        let address: Multiaddr = "/ip6/::1/tcp/0/http".parse().unwrap();
        test_multiaddr(address).await;
    }

    #[test]
    fn request_timeout_falls_back_to_the_default() {
        let unset = Config::new();
        assert_eq!(
            unset.server_builder().request_timeout(),
            super::DEFAULT_GRPC_REQUEST_TIMEOUT
        );

        let mut configured = Config::new();
        configured.request_timeout = Some(Duration::from_secs(7));
        assert_eq!(
            configured.server_builder().request_timeout(),
            Duration::from_secs(7)
        );
    }

    /// Answers only once the whole request body has arrived, so a body that
    /// never completes keeps the request in flight for as long as the peer
    /// likes.
    #[derive(Clone)]
    struct DrainBody;

    impl tower::Service<http::Request<tonic::body::Body>> for DrainBody {
        type Response = http::Response<tonic::body::Body>;
        type Error = std::convert::Infallible;
        type Future = futures::future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

        fn poll_ready(
            &mut self,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), Self::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: http::Request<tonic::body::Body>) -> Self::Future {
            use http_body_util::BodyExt as _;
            Box::pin(async move {
                let _ = request.into_body().collect().await;
                Ok(tonic::Status::new(Code::Ok, "").into_http())
            })
        }
    }

    impl tonic::server::NamedService for DrainBody {
        const NAME: &'static str = "test.DrainBody";
    }

    /// A request body that either is already complete or never produces its
    /// data, mirroring a peer that sends HEADERS and then goes silent.
    enum TestBody {
        Empty,
        Stalled,
    }

    impl http_body::Body for TestBody {
        type Data = bytes::Bytes;
        type Error = std::convert::Infallible;

        fn poll_frame(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
            match *self {
                TestBody::Empty => std::task::Poll::Ready(None),
                TestBody::Stalled => std::task::Poll::Pending,
            }
        }

        fn is_end_stream(&self) -> bool {
            matches!(self, TestBody::Empty)
        }
    }

    /// A request that never finishes sending its body must be answered by the
    /// server-side deadline, releasing the admission slot it was holding.
    #[tokio::test]
    async fn server_timeout_releases_a_stalled_request_and_its_admission_slot() {
        const REQUEST_TIMEOUT: Duration = Duration::from_millis(200);

        let mut config = Config::new();
        config.request_timeout = Some(REQUEST_TIMEOUT);
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/0/http".parse().unwrap();
        let server = config
            .server_builder()
            .add_service_with_concurrency_limit(DrainBody, std::num::NonZeroUsize::MIN, false)
            .bind(&address, None)
            .await
            .unwrap();

        let stream = tokio::net::TcpStream::connect(server.local_addr().to_socket_addr().unwrap())
            .await
            .unwrap();
        let (mut sender, connection) = hyper::client::conn::http2::handshake(
            hyper_util::rt::TokioExecutor::new(),
            hyper_util::rt::TokioIo::new(stream),
        )
        .await
        .unwrap();
        tokio::spawn(connection);

        let request = |body| {
            http::Request::post("/test.DrainBody/Drain")
                .header("content-type", "application/grpc")
                .body(body)
                .unwrap()
        };
        let grpc_status = |response: &http::Response<hyper::body::Incoming>| {
            response.headers()["grpc-status"]
                .to_str()
                .unwrap()
                .to_owned()
        };

        // Occupies the only admission slot without ever sending its body.
        let stalled = tokio::time::timeout(
            REQUEST_TIMEOUT * 25,
            sender.send_request(request(TestBody::Stalled)),
        )
        .await
        .expect("the server must answer a stalled request once its deadline passes")
        .unwrap();
        assert_eq!(
            grpc_status(&stalled),
            (Code::DeadlineExceeded as i32).to_string()
        );

        // The slot is free again, so a complete request is served.
        let served = sender.send_request(request(TestBody::Empty)).await.unwrap();
        assert_eq!(grpc_status(&served), (Code::Ok as i32).to_string());

        server.server_handle.shutdown().await;
    }

    /// The configured stream cap is advertised to the peer, so requests beyond
    /// it on one connection never reach the server until an earlier one ends.
    #[tokio::test]
    async fn stream_cap_bounds_the_requests_one_connection_can_open() {
        #[derive(Clone, Default)]
        struct Started(Arc<std::sync::atomic::AtomicUsize>);

        impl MetricsCallbackProvider for Started {
            fn on_request(&self, _path: String) {}

            fn on_response(
                &self,
                _path: String,
                _latency: Duration,
                _status: u16,
                _grpc_status_code: Code,
            ) {
            }

            fn on_start(&self, _path: &str) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }

        const STREAM_CAP: u32 = 2;

        let started = Started::default();
        let mut config = Config::new();
        config.http2_max_concurrent_streams = Some(STREAM_CAP);
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/0/http".parse().unwrap();
        let server = config
            .server_builder_with_metrics(started.clone())
            .add_service(DrainBody)
            .bind(&address, None)
            .await
            .unwrap();

        let stream = tokio::net::TcpStream::connect(server.local_addr().to_socket_addr().unwrap())
            .await
            .unwrap();
        let (sender, connection) = hyper::client::conn::http2::handshake(
            hyper_util::rt::TokioExecutor::new(),
            hyper_util::rt::TokioIo::new(stream),
        )
        .await
        .unwrap();
        tokio::spawn(connection);

        // One more stalled request than the cap allows; the surplus waits on
        // the client side for a stream to free up.
        let in_flight: Vec<_> = (0..STREAM_CAP + 1)
            .map(|_| {
                let mut sender = sender.clone();
                tokio::spawn(async move {
                    sender
                        .send_request(
                            http::Request::post("/test.DrainBody/Drain")
                                .body(TestBody::Stalled)
                                .unwrap(),
                        )
                        .await
                })
            })
            .collect();
        tokio::time::sleep(Duration::from_millis(300)).await;

        assert_eq!(
            started.0.load(std::sync::atomic::Ordering::SeqCst),
            STREAM_CAP as usize,
            "the server must only see as many requests as the stream cap allows"
        );

        for request in in_flight {
            request.abort();
        }
        server.server_handle.shutdown().await;
    }
}

#[derive(Clone)]
struct RequestLifetimeLayer<M: MetricsCallbackProvider> {
    metrics_provider: M,
}

impl<M: MetricsCallbackProvider, S> Layer<S> for RequestLifetimeLayer<M> {
    type Service = RequestLifetime<M, S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestLifetime {
            inner,
            metrics_provider: self.metrics_provider.clone(),
            path: None,
        }
    }
}

#[derive(Clone)]
struct RequestLifetime<M: MetricsCallbackProvider, S> {
    inner: S,
    metrics_provider: M,
    path: Option<String>,
}

impl<M: MetricsCallbackProvider, S, RequestBody> Service<Request<RequestBody>>
    for RequestLifetime<M, S>
where
    S: Service<Request<RequestBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<RequestBody>) -> Self::Future {
        if self.path.is_none() {
            let path = request.uri().path().to_string();
            self.metrics_provider.on_start(&path);
            self.path = Some(path);
        }
        self.inner.call(request)
    }
}

impl<M: MetricsCallbackProvider, S> Drop for RequestLifetime<M, S> {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            self.metrics_provider.on_drop(path)
        }
    }
}
