// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The requests of a listener: how many are in flight, how long they take, and
//! how the connections carry them.

use std::time::Instant;

use iota_metrics::peak::PeakGauge;
use prometheus_filtered::{
    Histogram, MetricLevel, Registry, register_histogram_vec_with_registry,
    register_histogram_with_registry,
};

/// The gaps with no request in flight, in seconds.
const IDLE_SECONDS_BUCKETS: &[f64] = &[1.0, 5.0, 30.0, 300.0, 3600.0];
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
    /// `connection_idle_seconds` with `end` = `request`.
    connection_idle_until_request: Histogram,
    /// `connection_idle_seconds` with `end` = `close`.
    connection_idle_until_close: Histogram,
    connection_inflight_requests: Histogram,
    connection_inflight_requests_peak: PeakGauge,
    request_completion_latency: Histogram,
    response_delivery_latency: Histogram,
}

impl Metrics {
    pub(super) fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        let connection_idle_seconds = register_histogram_vec_with_registry!(
            format!("{prefix}_connection_idle_seconds"),
            "Gaps with no request in flight, after the first request, until the next request \
             (end is request) or the close (end is close)",
            &["end"],
            IDLE_SECONDS_BUCKETS.to_vec(),
            registry;
            level
        )
        .expect("the metrics of a listener register without collision");
        let idle_until = |end: IdleEnd| {
            let end: &str = end.into();
            connection_idle_seconds.with_label_values(&[end])
        };
        Self {
            connection_idle_until_request: idle_until(IdleEnd::Request),
            connection_idle_until_close: idle_until(IdleEnd::Close),
            connection_inflight_requests: register_histogram_with_registry!(
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
                 is done and the transport has taken the last frame of the response body, or \
                 has dropped the body. For a streaming response, the time covers the whole \
                 stream",
                REQUEST_SECONDS_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            response_delivery_latency: register_histogram_with_registry!(
                format!("{prefix}_response_delivery_latency"),
                "Time in seconds from the moment the response future returns the response, with \
                 its headers, until the request ends as in request_completion_latency. The \
                 transport takes the next frame only when the data before it fits in the HTTP/2 \
                 window of the client or in the write buffer, so the time includes the waits \
                 for a slow client, but not for the last frame: a response of one data frame \
                 ends without a read by the client. A request whose response future fails or is \
                 dropped before the response is ready is not observed. For a streaming \
                 response, the time covers the whole stream",
                DELIVERY_SECONDS_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
        }
    }

    pub(super) fn connection_idle_seconds(&self, end: IdleEnd) -> &Histogram {
        match end {
            IdleEnd::Request => &self.connection_idle_until_request,
            IdleEnd::Close => &self.connection_idle_until_close,
        }
    }

    pub(super) fn connection_inflight_requests(&self) -> &Histogram {
        &self.connection_inflight_requests
    }

    pub(super) fn record_peak_inflight(&self, in_flight: u64, now: Instant) {
        self.connection_inflight_requests_peak
            .observe_at(in_flight, now);
    }

    pub(super) fn request_completion_latency(&self) -> &Histogram {
        &self.request_completion_latency
    }

    pub(super) fn response_delivery_latency(&self) -> &Histogram {
        &self.response_delivery_latency
    }
}
