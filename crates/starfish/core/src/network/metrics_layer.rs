// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tower layer adapters that allow specifying callbacks for request and
//! response handling can be implemented for different networking stacks.

use std::sync::Arc;

use prometheus_filtered::HistogramTimer;

use super::metrics::NetworkRouteMetrics;

/// Length of the prefix gRPC puts in front of every message: a compression
/// flag byte followed by the big-endian `u32` payload length.
const GRPC_MESSAGE_PREFIX_LEN: usize = 5;

pub(crate) trait SizedRequest {
    /// Size of the request head in bytes. Request bodies are not observable at
    /// this layer.
    fn size(&self) -> usize;
    /// Metric label for this request, from a fixed set of values.
    fn route(&self) -> &'static str;
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

        self.metrics.requests.with_label_values(&[route]).inc();
        self.metrics
            .inflight_requests
            .with_label_values(&[route])
            .inc();
        let request_size = request.size();
        if request_size > 0 {
            self.metrics
                .request_size
                .with_label_values(&[route])
                .observe(request_size as f64);
        }
        if request_size > self.excessive_message_size {
            self.metrics
                .excessive_size_requests
                .with_label_values(&[route])
                .inc();
        }

        let timer = self
            .metrics
            .request_latency
            .with_label_values(&[route])
            .start_timer();

        MetricsResponseCallback {
            metrics: self.metrics.clone(),
            timer,
            route,
            excessive_message_size: self.excessive_message_size,
            messages: GrpcMessageParser::default(),
        }
    }
}

pub(crate) struct MetricsResponseCallback {
    metrics: Arc<NetworkRouteMetrics>,
    // The timer is held on to and "observed" once dropped
    #[expect(unused)]
    timer: HistogramTimer,
    route: &'static str,
    excessive_message_size: usize,
    /// Splits the response body into gRPC messages.
    messages: GrpcMessageParser,
}

impl MetricsResponseCallback {
    /// Records a failed response status. Sizes are recorded per message as the
    /// body streams through `on_chunk`.
    pub(crate) fn on_response(&mut self, error_type: Option<&str>) {
        if let Some(err) = error_type {
            self.metrics
                .errors
                .with_label_values(&[self.route, err])
                .inc();
        }
    }

    pub(crate) fn on_error<E>(&mut self, _error: &E) {
        self.metrics
            .errors
            .with_label_values(&[self.route, "unknown"])
            .inc();
    }

    /// Records the wire size of every message that completes inside `chunk`.
    pub(crate) fn on_chunk(&mut self, chunk: &[u8]) {
        let metrics = &self.metrics;
        let route = self.route;
        let excessive_message_size = self.excessive_message_size;
        self.messages.feed(chunk, |message_size| {
            metrics
                .response_size
                .with_label_values(&[route])
                .observe(message_size as f64);
            if message_size > excessive_message_size {
                metrics
                    .excessive_size_responses
                    .with_label_values(&[route])
                    .inc();
            }
        });
    }

    /// Counts a body that ended inside a message as a `truncated` error.
    pub(crate) fn on_end_of_stream(&mut self) {
        if self.messages.finish() {
            self.metrics
                .errors
                .with_label_values(&[self.route, "truncated"])
                .inc();
        }
    }
}

impl Drop for MetricsResponseCallback {
    fn drop(&mut self) {
        self.metrics
            .inflight_requests
            .with_label_values(&[self.route])
            .dec();
    }
}

/// Splits a gRPC body into its length-prefixed messages and reports the wire
/// size of each: the prefix plus the payload, which is compressed when the
/// channel compresses.
///
/// Body chunks carry no message boundaries of their own: a chunk can hold
/// several messages, part of one, or even part of a prefix.
enum GrpcMessageParser {
    /// Reading the prefix; `len` bytes of it have arrived.
    Prefix {
        bytes: [u8; GRPC_MESSAGE_PREFIX_LEN],
        len: usize,
    },
    /// Reading the payload of a `size`-byte message; `remaining` payload
    /// bytes are still to come.
    Payload { remaining: usize, size: usize },
}

impl Default for GrpcMessageParser {
    fn default() -> Self {
        Self::Prefix {
            bytes: [0; GRPC_MESSAGE_PREFIX_LEN],
            len: 0,
        }
    }
}

impl GrpcMessageParser {
    /// Consumes `chunk`, calling `on_message` with the wire size of every
    /// message that completes in it.
    fn feed(&mut self, mut chunk: &[u8], mut on_message: impl FnMut(usize)) {
        while !chunk.is_empty() {
            match self {
                Self::Prefix { bytes, len } => {
                    let take = (GRPC_MESSAGE_PREFIX_LEN - *len).min(chunk.len());
                    bytes[*len..*len + take].copy_from_slice(&chunk[..take]);
                    *len += take;
                    chunk = &chunk[take..];
                    if *len < GRPC_MESSAGE_PREFIX_LEN {
                        continue;
                    }
                    let payload_len =
                        u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
                    // The length is peer-supplied; saturate rather than trust it.
                    let size = GRPC_MESSAGE_PREFIX_LEN.saturating_add(payload_len);
                    if payload_len == 0 {
                        on_message(size);
                        *self = Self::default();
                    } else {
                        *self = Self::Payload {
                            remaining: payload_len,
                            size,
                        };
                    }
                }
                Self::Payload { remaining, size } => {
                    let take = (*remaining).min(chunk.len());
                    *remaining -= take;
                    chunk = &chunk[take..];
                    if *remaining == 0 {
                        on_message(*size);
                        *self = Self::default();
                    }
                }
            }
        }
    }

    /// Resets the parser, returning whether the body ended inside a message.
    fn finish(&mut self) -> bool {
        let mid_message = !matches!(self, Self::Prefix { len: 0, .. });
        *self = Self::default();
        mid_message
    }
}

#[cfg(test)]
mod tests {
    use prometheus_filtered::Registry;

    use super::*;

    const ROUTE: &str = "route";
    const EXCESSIVE_MESSAGE_SIZE: usize = 64;

    struct TestRequest;

    impl SizedRequest for TestRequest {
        fn size(&self) -> usize {
            0
        }

        fn route(&self) -> &'static str {
            ROUTE
        }
    }

    fn callback() -> (Arc<NetworkRouteMetrics>, MetricsResponseCallback) {
        let metrics = Arc::new(NetworkRouteMetrics::new("test", &Registry::new()));
        let callback = MetricsCallbackMaker::new(metrics.clone(), EXCESSIVE_MESSAGE_SIZE)
            .handle_request(&TestRequest);
        (metrics, callback)
    }

    /// A gRPC message with an uncompressed payload of `payload_len` bytes.
    fn message(payload_len: usize) -> Vec<u8> {
        let mut message = vec![0];
        message.extend((payload_len as u32).to_be_bytes());
        message.extend(std::iter::repeat_n(0xAB, payload_len));
        message
    }

    fn sizes(metrics: &NetworkRouteMetrics) -> (u64, f64) {
        let histogram = metrics.response_size.with_label_values(&[ROUTE]);
        (histogram.get_sample_count(), histogram.get_sample_sum())
    }

    fn excessive(metrics: &NetworkRouteMetrics) -> u64 {
        metrics
            .excessive_size_responses
            .with_label_values(&[ROUTE])
            .get()
    }

    fn truncated(metrics: &NetworkRouteMetrics) -> u64 {
        metrics
            .errors
            .with_label_values(&[ROUTE, "truncated"])
            .get()
    }

    #[test]
    fn message_split_across_chunks_is_one_sample() {
        let (metrics, mut callback) = callback();
        let message = message(100);

        // Prefix split at byte 3, payload split in two.
        callback.on_chunk(&message[..3]);
        callback.on_chunk(&message[3..40]);
        assert_eq!(sizes(&metrics), (0, 0.0));
        callback.on_chunk(&message[40..]);
        callback.on_end_of_stream();

        assert_eq!(sizes(&metrics), (1, 105.0));
        assert_eq!(excessive(&metrics), 1);
        assert_eq!(truncated(&metrics), 0);
    }

    #[test]
    fn several_messages_in_one_chunk_are_separate_samples() {
        let (metrics, mut callback) = callback();
        let mut chunk = message(10);
        chunk.extend(message(0));
        chunk.extend(message(20));

        callback.on_chunk(&chunk);
        callback.on_end_of_stream();

        assert_eq!(sizes(&metrics), (3, 45.0));
        assert_eq!(excessive(&metrics), 0);
        assert_eq!(truncated(&metrics), 0);
    }

    #[test]
    fn body_ending_inside_a_message_is_truncated_once() {
        let (metrics, mut callback) = callback();
        let message = message(100);

        callback.on_chunk(&message[..50]);
        callback.on_end_of_stream();
        // Trailers and the final poll both end the stream.
        callback.on_end_of_stream();

        assert_eq!(sizes(&metrics), (0, 0.0));
        assert_eq!(truncated(&metrics), 1);
    }
}
