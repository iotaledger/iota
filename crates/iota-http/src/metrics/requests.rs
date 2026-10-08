// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The requests of a listener: how many are in flight, how long they take, and
//! how the connections carry them.

use std::time::Instant;

use iota_metrics::peak::PeakGauge;
use prometheus_filtered::{
    Histogram, HistogramVec, MetricLevel, Registry, register_histogram_vec_with_registry,
    register_histogram_with_registry,
};

use crate::metrics::connection::Protocol;

/// The time without a request in flight, in seconds: the gaps between requests
/// and the time to the first request.
const NO_REQUEST_SECONDS_BUCKETS: &[f64] = &[1.0, 5.0, 30.0, 300.0, 3600.0];
/// The requests in flight on a connection.
const CONNECTION_INFLIGHT_BUCKETS: &[f64] = &[1.0, 16.0, 64.0, 100.0, 200.0, 1000.0];
const REQUEST_SECONDS_BUCKETS: &[f64] = &[0.1, 1.0, 60.0, 300.0, 600.0, 3600.0];
const DELIVERY_SECONDS_BUCKETS: &[f64] = &[0.1, 1.0, 5.0, 30.0, 300.0];

/// What ends a gap with no request in flight.
#[derive(Clone, Copy, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub(super) enum IdleEnd {
    Request,
    Close,
}

pub(super) struct Metrics {
    first_request_seconds: HistogramVec,
    idle_seconds: HistogramVec,
    inflight_requests: Histogram,
    connection_inflight_requests_peak: PeakGauge,
    request_completion_latency: Histogram,
    response_delivery_latency: Histogram,
}

impl Metrics {
    pub(super) fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        Self {
            first_request_seconds: register_histogram_vec_with_registry!(
                format!("{prefix}_connection_first_request_seconds"),
                "Time from the TCP accept to the first request of the connection, by the HTTP \
                 version of the request",
                &["protocol"],
                NO_REQUEST_SECONDS_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            idle_seconds: register_histogram_vec_with_registry!(
                format!("{prefix}_connection_idle_seconds"),
                "Gaps with no request in flight, after the first request, until the next \
                 request (end is request) or the close (end is close). The protocol is the HTTP \
                 version of the first request of the connection",
                &["end", "protocol"],
                NO_REQUEST_SECONDS_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            inflight_requests: register_histogram_with_registry!(
                format!("{prefix}_connection_inflight_requests"),
                "At each request start: the requests in flight on the connection, with the new \
                 one",
                CONNECTION_INFLIGHT_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            connection_inflight_requests_peak: PeakGauge::register(
                &format!("{prefix}_connection_inflight_requests_peak"),
                "The most requests in flight on one connection when a request started, over the \
                 last 2 minutes",
                module_path!(),
                registry,
                level,
            ),
            request_completion_latency: register_histogram_with_registry!(
                format!("{prefix}_request_completion_latency"),
                "Time in seconds from the start of a request until it ends: the response future \
                 and the response body are done and the transport has written the last chunk of \
                 the response to the socket or copied it into its buffer. For a streaming \
                 response, the time covers the whole stream",
                REQUEST_SECONDS_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            response_delivery_latency: register_histogram_with_registry!(
                format!("{prefix}_response_delivery_latency"),
                "Time in seconds from the moment the response future returns the response, with \
                 its headers, until the request ends as in request_completion_latency. A \
                 response that fits in the HTTP/2 window of the client and in the buffer of the \
                 socket ends without a read by the client. A request whose response future \
                 fails or is dropped before the response is ready is not observed. For a \
                 streaming response, the time covers the whole stream",
                DELIVERY_SECONDS_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
        }
    }

    pub(super) fn first_request(&self, protocol: Protocol) -> Histogram {
        let protocol: &str = protocol.into();
        self.first_request_seconds.with_label_values(&[protocol])
    }

    pub(super) fn idle(&self, end: IdleEnd, protocol: Protocol) -> Histogram {
        let (end, protocol): (&str, &str) = (end.into(), protocol.into());
        self.idle_seconds.with_label_values(&[end, protocol])
    }

    pub(super) fn inflight(&self) -> &Histogram {
        &self.inflight_requests
    }

    pub(super) fn record_peak_inflight(&self, in_flight: u64, now: Instant) {
        self.connection_inflight_requests_peak
            .observe_at(in_flight, now);
    }

    pub(super) fn completion(&self) -> &Histogram {
        &self.request_completion_latency
    }

    pub(super) fn delivery(&self) -> &Histogram {
        &self.response_delivery_latency
    }
}
