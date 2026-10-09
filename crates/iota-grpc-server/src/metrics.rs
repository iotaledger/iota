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

use iota_metrics::peak::{PeakGauge, PeakGaugeVec};
use pin_project_lite::pin_project;
use prometheus_filtered::{
    Histogram, HistogramVec, IntCounterVec, IntGauge, IntGaugeVec, MetricLevel, Registry,
    register_histogram_vec_with_registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry, register_int_gauge_vec_with_registry,
    register_int_gauge_with_registry,
};
use strum::IntoEnumIterator;
use tonic::{Code, Status};
use tower::{Layer, Service};

mod response_item_sizes;
mod response_message_sizes;

pub(crate) use response_item_sizes::{CheckpointSizeTracker, ResponseItemKind};

pub const SPAM_LABEL: &str = "SPAM";

pub const LATENCY_SEC_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 20., 30., 60., 90.,
];

/// Sizes of a gRPC message in bytes, at the message size limits of the API: 1
/// MiB is the smallest `max_message_size_bytes` a request may ask for, 4 MiB the
/// default, 30 MiB the largest checkpoint of the protocol and 128 MiB the
/// largest message. 64 KiB, 8, 16 and 64 MiB fill in between.
const MESSAGE_SIZE_BUCKETS: &[f64] = &[
    65_536.0,
    1_048_576.0,
    4_194_304.0,
    8_388_608.0,
    16_777_216.0,
    31_457_280.0,
    67_108_864.0,
    134_217_728.0,
];

/// Sizes of a response message in bytes: [`MESSAGE_SIZE_BUCKETS`], and 1, 16
/// and 256 KiB for the small responses.
const RESPONSE_MESSAGE_SIZE_BUCKETS: &[f64] = &[
    1_024.0,
    16_384.0,
    65_536.0,
    262_144.0,
    1_048_576.0,
    4_194_304.0,
    8_388_608.0,
    16_777_216.0,
    31_457_280.0,
    67_108_864.0,
    134_217_728.0,
];

/// Metrics for the public-facing gRPC server.
///
/// Tracks in-flight requests, total request counts (by method and gRPC status),
/// request latency per RPC method, the sizes of response messages and items,
/// and the max message size that requests ask for.
#[derive(Clone)]
pub struct GrpcServerMetrics {
    inflight_requests: IntGaugeVec,
    num_requests: IntCounterVec,
    request_latency: HistogramVec,
    inflight_checkpoint_stream_subscribers: IntGauge,
    /// The peak of each kind of item, indexed by `ResponseItemKind as usize`.
    response_item_bytes_peaks: Vec<PeakGauge>,
    requested_max_message_bytes: Histogram,
    response_message_bytes: HistogramVec,
}

impl GrpcServerMetrics {
    pub fn new(registry: &Registry) -> Self {
        let response_item_bytes_peak = PeakGaugeVec::register(
            "node_grpc_response_item_bytes_peak",
            "The largest item built for a response over the last 2 minutes, by kind. The size \
             is observed after the field mask is applied, for each item of each response. It \
             includes an item that is too large for a message and so is rejected, and the \
             item at the end of a page of a List call that goes to the next page. The kind \
             checkpoint is the sum of all messages of one checkpoint of GetCheckpoint or \
             StreamCheckpoints",
            "kind",
            module_path!(),
            registry,
            MetricLevel::Info,
        );
        let response_item_bytes_peaks = ResponseItemKind::iter()
            .map(|kind| response_item_bytes_peak.with_label_values(&[kind.into()]))
            .collect();
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
            response_item_bytes_peaks,
            requested_max_message_bytes: register_histogram_with_registry!(
                "node_grpc_requested_max_message_bytes",
                "The max_message_size_bytes a request sets, as sent, before validation. A request \
                 that leaves it out is not observed",
                MESSAGE_SIZE_BUCKETS.to_vec(),
                registry;
                MetricLevel::Info
            )
            .expect("the gRPC server metrics register without collision"),
            response_message_bytes: register_histogram_vec_with_registry!(
                "node_grpc_response_message_bytes",
                "Size of each message of a response per method, in bytes: one message for a \
                 method that returns one, each message of a stream. A compressed message and a \
                 response that the tonic encoder rejects for its size are not observed",
                &["method"],
                RESPONSE_MESSAGE_SIZE_BUCKETS.to_vec(),
                registry;
                MetricLevel::Info
            )
            .expect("the gRPC server metrics register without collision"),
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
///
/// It puts the request metrics in the request extensions, for the handlers,
/// and observes the size of each message of each response.
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

impl<S, ReqBody> Service<http::Request<ReqBody>> for GrpcMetricsService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<tonic::body::Body>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
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

        let req = RequestMetrics::attach(req, &metrics);
        let response_message_bytes = metrics.response_message_bytes.with_label_values(&[&method]);

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
            response_message_bytes,
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
    ///   `response_message_bytes` observes the size of each message of the
    ///   response.
    /// - `Rejected`: a SPAM request that was rejected immediately. Returns the
    ///   pre-built response on first poll.
    #[project = GrpcMetricsFutureProj]
    pub enum GrpcMetricsFuture<F, Res> {
        Inner {
            #[pin]
            inner: F,
            guard: InFlightGuard,
            response_message_bytes: Histogram,
        },
        Rejected {
            response: Option<Res>,
        },
    }
}

impl<F, E> Future for GrpcMetricsFuture<F, http::Response<tonic::body::Body>>
where
    F: Future<Output = Result<http::Response<tonic::body::Body>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            GrpcMetricsFutureProj::Inner {
                inner,
                guard,
                response_message_bytes,
            } => match inner.poll(cx) {
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

                    Poll::Ready(result.map(|response| {
                        response_message_sizes::record_sizes(
                            response,
                            response_message_bytes.clone(),
                        )
                    }))
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

/// The metrics a handler can reach: the ones of its request, or none.
#[derive(Clone, Default)]
pub(crate) struct RequestMetrics(Option<Arc<GrpcServerMetrics>>);

impl RequestMetrics {
    /// The metrics that the metrics layer put in the request extensions.
    pub(crate) fn from_extensions(extensions: &http::Extensions) -> Self {
        extensions.get::<Self>().cloned().unwrap_or_default()
    }

    /// Puts the metrics of a request in the extensions of `request`.
    fn attach<B>(
        mut request: http::Request<B>,
        metrics: &Arc<GrpcServerMetrics>,
    ) -> http::Request<B> {
        request.extensions_mut().insert(Self(Some(metrics.clone())));
        request
    }

    /// Records the size of an item built for a response.
    pub(crate) fn record_response_item(&self, kind: ResponseItemKind, size: usize) {
        if let Some(metrics) = &self.0 {
            metrics.response_item_bytes_peaks[kind as usize].observe(size as u64);
        }
    }

    /// Whether the request has metrics.
    pub(crate) fn is_enabled(&self) -> bool {
        self.0.is_some()
    }

    /// Records the `max_message_size_bytes` a request sets.
    pub(crate) fn record_requested_max_message_size(&self, requested: Option<u32>) {
        if let (Some(metrics), Some(requested)) = (&self.0, requested) {
            metrics
                .requested_max_message_bytes
                .observe(f64::from(requested));
        }
    }
}
