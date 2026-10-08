// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The TLS handshakes of a listener.

use std::time::Instant;

use iota_metrics::peak::PeakGauge;
use prometheus_filtered::{
    HistogramVec, MetricLevel, Registry, register_histogram_vec_with_registry,
};

/// Handshake durations in seconds.
const HANDSHAKE_SECONDS_BUCKETS: &[f64] = &[0.1, 1.0, 5.0];

/// The result of a TLS handshake.
#[derive(Clone, Copy, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum HandshakeResult {
    Completed,
    Failed,
    TimedOut,
}

pub(super) struct Metrics {
    handshake_latency: HistogramVec,
    pending_handshakes_peak: PeakGauge,
}

impl Metrics {
    pub(super) fn new(prefix: &str, registry: &Registry) -> Self {
        Self {
            handshake_latency: register_histogram_vec_with_registry!(
                format!("{prefix}_handshake_latency"),
                "Time of a TLS handshake in seconds, from the start of the TLS accept. The \
                 result is completed, failed or timed_out",
                &["result"],
                HANDSHAKE_SECONDS_BUCKETS.to_vec(),
                registry;
                MetricLevel::Info
            )
            .expect("the metrics of a listener register without collision"),
            pending_handshakes_peak: PeakGauge::register(
                &format!("{prefix}_pending_handshakes_peak"),
                "The most TLS handshakes pending at the same time over the last 2 minutes. \
                 At the configured maximum of pending connections the accept loop stops \
                 accepting",
                module_path!(),
                registry,
                MetricLevel::Info,
            ),
        }
    }

    /// Records the connections in the set of pending handshakes.
    pub(super) fn record_pending_handshakes(&self, pending: usize) {
        self.pending_handshakes_peak.observe(pending as u64);
    }

    /// Starts to time a TLS handshake.
    pub(super) fn start_handshake(&self) -> HandshakeGuard {
        HandshakeGuard {
            handshake_latency: self.handshake_latency.clone(),
            started_at: Instant::now(),
        }
    }
}

/// A TLS handshake in progress.
pub(crate) struct HandshakeGuard {
    handshake_latency: HistogramVec,
    started_at: Instant,
}

impl HandshakeGuard {
    pub(crate) fn record_result(&self, result: HandshakeResult) {
        let result: &str = result.into();
        self.handshake_latency.with_label_values(&[result]).observe(
            Instant::now()
                .saturating_duration_since(self.started_at)
                .as_secs_f64(),
        );
    }
}
