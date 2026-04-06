// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use pin_project_lite::pin_project;
use prometheus::{
    HistogramVec, IntCounterVec, IntGaugeVec, Registry, register_histogram_vec_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_vec_with_registry,
};
use tonic::{Code, Status};
use tower::{Layer, Service};

const LATENCY_SEC_BUCKETS: &[f64] = &[
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
}

impl GrpcServerMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            inflight_requests: register_int_gauge_vec_with_registry!(
                "grpc_server_inflight_requests",
                "Total in-flight gRPC requests per method",
                &["method"],
                registry,
            )
            .unwrap(),
            num_requests: register_int_counter_vec_with_registry!(
                "grpc_server_requests",
                "Total gRPC requests per method and status code",
                &["method", "status"],
                registry,
            )
            .unwrap(),
            request_latency: register_histogram_vec_with_registry!(
                "grpc_server_request_latency",
                "Latency of gRPC requests per method in seconds",
                &["method"],
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
            .unwrap(),
        }
    }
}

/// Tower [`Layer`] that adds gRPC request metrics to a service.
#[derive(Clone)]
pub struct GrpcMetricsLayer {
    metrics: Arc<GrpcServerMetrics>,
}

impl GrpcMetricsLayer {
    pub fn new(metrics: Arc<GrpcServerMetrics>) -> Self {
        Self { metrics }
    }
}

impl<S> Layer<S> for GrpcMetricsLayer {
    type Service = GrpcMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        GrpcMetricsService {
            inner,
            metrics: self.metrics.clone(),
        }
    }
}

/// Tower [`Service`] wrapper that records gRPC request metrics.
#[derive(Clone)]
pub struct GrpcMetricsService<S> {
    inner: S,
    metrics: Arc<GrpcServerMetrics>,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for GrpcMetricsService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = GrpcMetricsFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let method = req.uri().path().to_owned();
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

        GrpcMetricsFuture {
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
    /// Future that records metrics when the inner response completes.
    ///
    /// On normal completion, records the gRPC status from the response headers.
    /// If dropped before completion (client disconnect), the [`InFlightGuard`]
    /// records a `"canceled"` status.
    pub struct GrpcMetricsFuture<F> {
        #[pin]
        inner: F,
        guard: InFlightGuard,
    }
}

impl<F, ResBody, E> Future for GrpcMetricsFuture<F>
where
    F: Future<Output = Result<http::Response<ResBody>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.inner.poll(cx) {
            Poll::Ready(result) => {
                let status = match &result {
                    Ok(response) => grpc_status_from_response(response),
                    Err(_) => "transport_error",
                };

                this.guard
                    .metrics
                    .num_requests
                    .with_label_values(&[this.guard.method.as_str(), status])
                    .inc();

                this.guard.completed = true;

                Poll::Ready(result)
            }
            Poll::Pending => Poll::Pending,
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

fn grpc_code_to_str(code: Code) -> &'static str {
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
