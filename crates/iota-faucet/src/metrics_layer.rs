// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures::Future;
use http::{Request, StatusCode};
use prometheus::{HistogramTimer, Registry};
use tower::{BoxError, Layer, Service, ServiceExt, load_shed::error::Overloaded};
use tracing::{error, info, warn};

use crate::metrics::RequestMetrics;

/// Tower Layer for tracking metrics in Prometheus related to number,
/// success-rate and latency of requests running through service.
#[derive(Clone)]
pub struct RequestMetricsLayer {
    metrics: Arc<RequestMetrics>,
}

#[derive(Clone)]
pub struct RequestMetricsService<Inner> {
    inner: Inner,
    metrics: Arc<RequestMetrics>,
}

pub struct RequestMetricsFuture<Res> {
    future: Pin<Box<dyn Future<Output = Result<Res, BoxError>> + Send>>,
}

struct MetricsGuard {
    timer: Option<HistogramTimer>,
    metrics: Arc<RequestMetrics>,
    path: String,
    user_agent: String,
}

impl RequestMetricsLayer {
    pub fn new(registry: &Registry) -> Self {
        Self {
            metrics: Arc::new(RequestMetrics::new(registry)),
        }
    }
}

impl<Inner> Layer<Inner> for RequestMetricsLayer {
    type Service = RequestMetricsService<Inner>;
    fn layer(&self, inner: Inner) -> Self::Service {
        RequestMetricsService {
            inner,
            metrics: self.metrics.clone(),
        }
    }
}

impl<Inner, Body> Service<Request<Body>> for RequestMetricsService<Inner>
where
    Inner: Service<Request<Body>, Response = http::Response<Body>, Error = BoxError>
        + Clone
        + Send
        + 'static,
    Inner::Future: Send,
    Body: Send + 'static,
{
    type Response = Inner::Response;
    type Error = BoxError;
    type Future = RequestMetricsFuture<Self::Response>;

    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(ctx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let path = req.uri().path().to_string();
        let user_agent = req
            .headers()
            .get(http::header::USER_AGENT)
            .and_then(|val| val.to_str().ok());
        let metrics = MetricsGuard::new(self.metrics.clone(), &path, user_agent);
        let inner = self.inner.clone();

        let future = Box::pin(async move {
            let resp = inner.oneshot(req).await;
            match &resp {
                Ok(resp) if !resp.status().is_success() => {
                    metrics.failed(None, Some(resp.status()))
                }
                Ok(_) => metrics.succeeded(),
                Err(err) => {
                    if err.is::<Overloaded>() {
                        metrics.shed();
                    } else {
                        metrics.failed(Some(err), None);
                    }
                }
            }

            resp
        });

        RequestMetricsFuture { future }
    }
}

impl<Res> Future for RequestMetricsFuture<Res> {
    type Output = Result<Res, BoxError>;
    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        Future::poll(self.future.as_mut(), ctx)
    }
}

impl MetricsGuard {
    fn new(metrics: Arc<RequestMetrics>, path: &str, user_agent: Option<&str>) -> Self {
        let user_agent = user_agent.unwrap_or("unknown");

        metrics
            .total_requests_received
            .with_label_values(&[path, user_agent])
            .inc();
        metrics
            .current_requests_in_flight
            .with_label_values(&[path, user_agent])
            .inc();

        MetricsGuard {
            timer: Some(
                metrics
                    .process_latency
                    .with_label_values(&[path, user_agent])
                    .start_timer(),
            ),
            metrics,
            path: path.to_string(),
            user_agent: user_agent.to_string(),
        }
    }

    fn succeeded(mut self) {
        if let Some(timer) = self.timer.take() {
            let elapsed = timer.stop_and_record();
            self.metrics
                .total_requests_succeeded
                .with_label_values(&[&self.path, &self.user_agent])
                .inc();
            info!("Request succeeded for path {} in {elapsed:.2}s", self.path);
        }
    }

    fn failed(mut self, error: Option<&BoxError>, status: Option<StatusCode>) {
        if let Some(timer) = self.timer.take() {
            let elapsed = timer.stop_and_record();
            self.metrics
                .total_requests_failed
                .with_label_values(&[&self.path, &self.user_agent])
                .inc();

            if let Some(err) = error {
                error!(
                    "request failed for path {} in {elapsed:.2}s, error {err:?}",
                    self.path
                );
            } else if let Some(status) = status {
                error!(
                    "Request failed for path {} in {elapsed:.2}s with status: {status}",
                    self.path
                );
            } else {
                warn!("Request failed for path {} in {elapsed:.2}s", self.path);
            }
        }
    }

    fn shed(mut self) {
        if let Some(timer) = self.timer.take() {
            let elapsed = timer.stop_and_record();
            self.metrics
                .total_requests_shed
                .with_label_values(&[&self.path, &self.user_agent])
                .inc();
            info!("Request shed for path {} in {elapsed:.2}s", self.path);
        }
    }
}

impl Drop for MetricsGuard {
    fn drop(&mut self) {
        self.metrics
            .current_requests_in_flight
            .with_label_values(&[&self.path, &self.user_agent])
            .dec();

        // Request was still in flight when the guard was dropped, implying the client
        // disconnected.
        if let Some(timer) = self.timer.take() {
            let elapsed = timer.stop_and_record();
            self.metrics
                .total_requests_disconnected
                .with_label_values(&[&self.path, &self.user_agent])
                .inc();
            info!(
                "Request disconnected for path {} in {elapsed:.2}s",
                self.path
            );
        }
    }
}
