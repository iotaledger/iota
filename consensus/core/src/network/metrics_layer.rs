// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tower layer adapters that allow specifying callbacks for request and
//! response handling can be implemented for different networking stacks.

use std::sync::Arc;

use prometheus::HistogramTimer;

use super::metrics::NetworkRouteMetrics;

pub(crate) trait SizedRequest {
    fn size(&self) -> usize;
    fn route(&self) -> String;
}

pub(crate) trait SizedResponse {
    fn size(&self) -> usize;
    fn error_type(&self) -> Option<String>;
}

#[derive(Clone)]
pub(crate) struct MetricsCallbackMaker {
    metrics: Arc<NetworkRouteMetrics>,
    /// Size in bytes above which a request or response message is considered
    /// excessively large
    excessive_message_size: usize,
}

impl MetricsCallbackMaker {
    pub(crate) fn new(metrics: Arc<NetworkRouteMetrics>, excessive_message_size: usize) -> Self {
        Self {
            metrics,
            excessive_message_size,
        }
    }

    // Update request metrics. And create a callback that should be called on
    // response.
    pub(crate) fn handle_request(&self, request: &dyn SizedRequest) -> MetricsResponseCallback {
        let route = request.route();

        self.metrics.requests.with_label_values(&[&route]).inc();
        self.metrics
            .inflight_requests
            .with_label_values(&[&route])
            .inc();
        let request_size = request.size();
        if request_size > 0 {
            self.metrics
                .request_size
                .with_label_values(&[&route])
                .observe(request_size as f64);
        }
        if request_size > self.excessive_message_size {
            self.metrics
                .excessive_size_requests
                .with_label_values(&[&route])
                .inc();
        }

        let timer = self
            .metrics
            .request_latency
            .with_label_values(&[&route])
            .start_timer();

        MetricsResponseCallback {
            metrics: self.metrics.clone(),
            timer,
            route,
            excessive_message_size: self.excessive_message_size,
            response_body_size: None,
        }
    }
}

pub(crate) struct MetricsResponseCallback {
    metrics: Arc<NetworkRouteMetrics>,
    // The timer is held on to and "observed" once dropped
    #[expect(unused)]
    timer: HistogramTimer,
    route: String,
    excessive_message_size: usize,
    /// If Some, response size has already been observed (exact size was known
    /// from headers). If None, response size should be tracked via body
    /// chunks.
    response_body_size: Option<usize>,
}

impl MetricsResponseCallback {
    /// Track response metrics from HTTP response parts.
    /// Handles both size tracking and error tracking.
    pub(crate) fn on_response(&mut self, response: &dyn SizedResponse, headers: &http::HeaderMap) {
        let mut response_size = response.size();

        // Try to get exact body size from Content-Length header
        // This is calculated outside of response.size() to properly handle
        // streaming/chunked responses later to avoid calculating the same bytes twice -
        // once from the header value and when parsing the response body chunks.
        let body_size = headers
            .get(http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());

        if let Some(body_size) = body_size {
            // Exact size is known, update response size
            response_size += body_size;

            // Mark as already observed
            self.response_body_size = Some(body_size);
        }
        if response_size > 0 {
            self.metrics
                .response_size
                .with_label_values(&[&self.route])
                .observe(response_size as f64);
        }
        if response_size > self.excessive_message_size {
            self.metrics
                .excessive_size_responses
                .with_label_values(&[&self.route])
                .inc();
        }

        if let Some(err) = response.error_type() {
            self.metrics
                .errors
                .with_label_values(&[&self.route, &err])
                .inc();
        }
    }

    pub(crate) fn on_error<E>(&mut self, _error: &E) {
        self.metrics
            .errors
            .with_label_values(&[self.route.as_str(), "unknown"])
            .inc();
    }

    pub(crate) fn on_chunk(&mut self, chunk_size: usize) {
        // Only track chunks if the exact size wasn't known from headers
        if self.response_body_size.is_none() && chunk_size > 0 {
            self.metrics
                .response_size
                .with_label_values(&[&self.route])
                .observe(chunk_size as f64);
        }
    }
}

impl Drop for MetricsResponseCallback {
    fn drop(&mut self) {
        self.metrics
            .inflight_requests
            .with_label_values(&[&self.route])
            .dec();
    }
}
