// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use pin_project_lite::pin_project;
use prometheus::{
    HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, Registry,
    register_histogram_vec_with_registry, register_int_counter_vec_with_registry,
    register_int_gauge_vec_with_registry, register_int_gauge_with_registry,
};
use tonic::{Code, Status};
use tower::{Layer, Service};

pub const SPAM_LABEL: &str = "SPAM";

pub const LATENCY_SEC_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 20., 30., 60., 90.,
];

/// Metrics for the public-facing gRPC server.
///
/// Tracks in-flight requests, total request counts (by method and gRPC status),
/// and request latency per RPC method.
#[derive(Clone)]
pub struct GrpcServerMetrics {
    inflight_requests: IntGaugeVec,
    num_requests: IntCounterVec,
    request_latency: HistogramVec,
    inflight_checkpoint_stream_subscribers: IntGauge,
}

impl GrpcServerMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            inflight_requests: register_int_gauge_vec_with_registry!(
                "node_grpc_inflight_requests",
                "Total in-flight node gRPC requests per method",
                &["method"],
                registry,
            )
            .unwrap(),
            num_requests: register_int_counter_vec_with_registry!(
                "node_grpc_requests",
                "Total node gRPC requests per method and status code",
                &["method", "status"],
                registry,
            )
            .unwrap(),
            request_latency: register_histogram_vec_with_registry!(
                "node_grpc_request_latency",
                "Latency of node gRPC requests per method in seconds",
                &["method"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
            inflight_checkpoint_stream_subscribers: register_int_gauge_with_registry!(
                "node_grpc_inflight_checkpoint_stream_subscribers",
                "Number of active subscribers to the checkpoint data broadcast",
                registry,
            )
            .unwrap(),
        }
    }

    /// Gauge tracking the number of active subscribers to the checkpoint
    /// data broadcast. Cheap to clone (wraps an `Arc` internally).
    pub fn inflight_checkpoint_stream_subscribers(&self) -> IntGauge {
        self.inflight_checkpoint_stream_subscribers.clone()
    }
}

/// Tower [`Layer`] that adds gRPC request metrics to a service.
///
/// Only records per-method metrics for paths that exactly match a known gRPC
/// method. All other requests (e.g. non-gRPC HTTP traffic that reaches
/// the port) are aggregated under a single `"SPAM"` label to prevent
/// unbounded cardinality.
#[derive(Clone)]
pub struct GrpcMetricsLayer {
    metrics: Arc<GrpcServerMetrics>,
    /// Exact set of known gRPC method paths (e.g.
    /// `"/iota.grpc.v1.ledger_service.LedgerService/GetCheckpoint"`).
    /// Only paths in this set get their own metric label; everything else
    /// is labelled `"SPAM"`.
    known_methods: Arc<HashSet<&'static str>>,
}

impl GrpcMetricsLayer {
    pub fn new(metrics: Arc<GrpcServerMetrics>, method_paths: &[&'static str]) -> Self {
        Self {
            metrics,
            known_methods: Arc::new(method_paths.iter().copied().collect()),
        }
    }
}

impl<S> Layer<S> for GrpcMetricsLayer {
    type Service = GrpcMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcMetricsService {
            inner,
            metrics: self.metrics.clone(),
            known_methods: self.known_methods.clone(),
        }
    }
}

/// Tower [`Service`] wrapper that records gRPC request metrics.
#[derive(Clone)]
pub struct GrpcMetricsService<S> {
    inner: S,
    metrics: Arc<GrpcServerMetrics>,
    known_methods: Arc<HashSet<&'static str>>,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for GrpcMetricsService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = GrpcMetricsFuture<S::Future, S::Response>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let raw_path = req.uri().path();

        if !self.known_methods.contains(raw_path) {
            // SPAM: bump counter and reject immediately without calling the
            // inner service, avoiding unnecessary router work.
            self.metrics
                .num_requests
                .with_label_values(&[SPAM_LABEL, "Unimplemented"])
                .inc();

            let response = Status::unimplemented("").into_http();

            return GrpcMetricsFuture::Rejected {
                response: Some(response),
            };
        }

        let method = raw_path.to_owned();
        let metrics = self.metrics.clone();

        metrics
            .inflight_requests
            .with_label_values(&[&method])
            .inc();

        let guard = InFlightGuard {
            metrics,
            method,
            start: Instant::now(),
            completed: false,
        };

        let future = self.inner.call(req);

        GrpcMetricsFuture::Inner {
            inner: future,
            guard,
        }
    }
}

/// RAII guard that tracks in-flight requests and records metrics on drop.
///
/// When a request completes normally, [`GrpcMetricsFuture::poll`] records the
/// response status and marks the guard as completed. If the future is dropped
/// before completion (e.g. client disconnect), the guard records a `"canceled"`
/// status instead, matching the REST metrics behavior.
struct InFlightGuard {
    metrics: Arc<GrpcServerMetrics>,
    method: String,
    start: Instant,
    completed: bool,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.metrics
            .inflight_requests
            .with_label_values(&[&self.method])
            .dec();

        let latency = self.start.elapsed().as_secs_f64();
        self.metrics
            .request_latency
            .with_label_values(&[&self.method])
            .observe(latency);

        if !self.completed {
            self.metrics
                .num_requests
                .with_label_values(&[self.method.as_str(), "canceled"])
                .inc();
        }
    }
}

pin_project! {
    /// Future returned by [`GrpcMetricsService`].
    ///
    /// - `Inner`: a real request forwarded to the inner service. Records the
    ///   gRPC status from the response headers on completion. If dropped before
    ///   completion (client disconnect), the [`InFlightGuard`] records a
    ///   `"canceled"` status.
    /// - `Rejected`: a SPAM request that was rejected immediately. Returns the
    ///   pre-built response on first poll.
    #[project = GrpcMetricsFutureProj]
    pub enum GrpcMetricsFuture<F, Res> {
        Inner {
            #[pin]
            inner: F,
            guard: InFlightGuard,
        },
        Rejected {
            response: Option<Res>,
        },
    }
}

impl<F, ResBody, E> Future for GrpcMetricsFuture<F, http::Response<ResBody>>
where
    F: Future<Output = Result<http::Response<ResBody>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            GrpcMetricsFutureProj::Inner { inner, guard } => match inner.poll(cx) {
                Poll::Ready(result) => {
                    let status = match &result {
                        Ok(response) => grpc_status_from_response(response),
                        Err(_) => "transport_error",
                    };

                    guard
                        .metrics
                        .num_requests
                        .with_label_values(&[guard.method.as_str(), status])
                        .inc();

                    guard.completed = true;

                    Poll::Ready(result)
                }
                Poll::Pending => Poll::Pending,
            },
            GrpcMetricsFutureProj::Rejected { response } => {
                Poll::Ready(Ok(response.take().expect("polled after completion")))
            }
        }
    }
}

/// Extract the gRPC status code from response headers.
///
/// Uses [`tonic::Status::from_header_map`] to parse the `grpc-status` header.
/// If not present, the response is assumed to be OK. For streaming RPCs, errors
/// sent via trailers are not visible here and will be reported as OK.
fn grpc_status_from_response<B>(response: &http::Response<B>) -> &'static str {
    let code = Status::from_header_map(response.headers()).map_or(Code::Ok, |s| s.code());
    grpc_code_to_str(code)
}

pub fn grpc_code_to_str(code: Code) -> &'static str {
    match code {
        Code::Ok => "Ok",
        Code::Cancelled => "Cancelled",
        Code::Unknown => "Unknown",
        Code::InvalidArgument => "InvalidArgument",
        Code::DeadlineExceeded => "DeadlineExceeded",
        Code::NotFound => "NotFound",
        Code::AlreadyExists => "AlreadyExists",
        Code::PermissionDenied => "PermissionDenied",
        Code::ResourceExhausted => "ResourceExhausted",
        Code::FailedPrecondition => "FailedPrecondition",
        Code::Aborted => "Aborted",
        Code::OutOfRange => "OutOfRange",
        Code::Unimplemented => "Unimplemented",
        Code::Internal => "Internal",
        Code::Unavailable => "Unavailable",
        Code::DataLoss => "DataLoss",
        Code::Unauthenticated => "Unauthenticated",
    }
}
