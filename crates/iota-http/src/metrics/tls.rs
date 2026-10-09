// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The TLS handshakes of a listener.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use iota_metrics::peak::PeakGauge;
use prometheus_filtered::{
    HistogramVec, MetricLevel, Registry, register_histogram_vec_with_registry,
};

/// Handshake durations in seconds.
const HANDSHAKE_LATENCY_SEC_BUCKETS: &[f64] = &[0.1, 1.0, 5.0];

/// The result of a TLS handshake.
#[derive(Clone, Copy, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum HandshakeResult {
    Completed,
    Failed,
    TimedOut,
    /// The handshake was cancelled, as at the shutdown of the server.
    Dropped,
}

pub(super) struct Metrics {
    handshake_latency: HistogramVec,
    pending_handshakes_peak: PeakGauge,
    /// The handshakes in progress: one per live `HandshakeGuard`.
    pending_handshakes: AtomicU64,
}

impl Metrics {
    pub(super) fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        Self {
            handshake_latency: register_histogram_vec_with_registry!(
                format!("{prefix}_handshake_latency"),
                "Time of a TLS handshake in seconds, from the start of the TLS accept. The \
                 result is completed, failed, timed_out or dropped",
                &["result"],
                HANDSHAKE_LATENCY_SEC_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
            pending_handshakes_peak: PeakGauge::register(
                &format!("{prefix}_pending_handshakes_peak"),
                "The most TLS handshakes pending at the same time over the last 2 minutes. \
                 At the configured maximum of pending connections the accept loop stops \
                 accepting",
                module_path!(),
                registry,
                level,
            ),
            pending_handshakes: AtomicU64::new(0),
        }
    }
}

/// Records the start of a TLS handshake: times it, and counts it as pending
/// until the guard is dropped.
pub(super) fn record_handshake_start(metrics: &Arc<Metrics>) -> HandshakeGuard {
    let pending = metrics.pending_handshakes.fetch_add(1, Ordering::Relaxed) + 1;
    metrics.pending_handshakes_peak.observe(pending);
    HandshakeGuard {
        metrics: metrics.clone(),
        started_at: Instant::now(),
        result: HandshakeResult::Dropped,
    }
}

/// A TLS handshake in progress. Dropping it records the handshake, as
/// `dropped` if no result was recorded.
pub(crate) struct HandshakeGuard {
    metrics: Arc<Metrics>,
    started_at: Instant,
    result: HandshakeResult,
}

impl HandshakeGuard {
    pub(crate) fn record_result(mut self, result: HandshakeResult) {
        self.result = result;
    }
}

impl Drop for HandshakeGuard {
    fn drop(&mut self) {
        self.metrics
            .pending_handshakes
            .fetch_sub(1, Ordering::Relaxed);
        let result: &str = self.result.into();
        self.metrics
            .handshake_latency
            .with_label_values(&[result])
            .observe(
                Instant::now()
                    .saturating_duration_since(self.started_at)
                    .as_secs_f64(),
            );
    }
}
