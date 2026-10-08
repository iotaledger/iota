// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Measures the connections of one listener.
//!
//! The listener wraps each socket it accepts in a `TrackedIo` and uses it for
//! the whole life of the connection.

use std::{
    fmt,
    net::SocketAddr,
    sync::{Arc, OnceLock},
};

use iota_metrics::peak::IntGaugeWithPeakGauge;
use prometheus_filtered::{
    Histogram, IntCounterVec, MetricLevel, Registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry,
};

mod connection;
mod io;
mod scope;
mod tls;

pub(crate) use connection::TrackedConnection;
pub(crate) use io::TrackedIo;
use scope::Scope;
pub(crate) use tls::HandshakeResult;

/// Connection lifetimes in seconds.
const LIFETIME_SECONDS_BUCKETS: &[f64] = &[1.0, 5.0, 30.0, 60.0, 300.0, 1800.0, 14400.0, 86400.0];

/// The metrics of one listener, and the state they need.
#[derive(Clone)]
pub struct ListenerMetrics(Arc<Inner>);

impl fmt::Debug for ListenerMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListenerMetrics")
            .field("listener", &self.0.prefix)
            .finish_non_exhaustive()
    }
}

impl ListenerMetrics {
    /// Registers the metrics as `{prefix}_<name>` in `registry`, at level
    /// `Info`, except the counter of the accepted connections, which gets
    /// `connection_counts_level`. The TLS metrics are registered later, by
    /// `enable_tls`.
    ///
    /// # Panics
    ///
    /// Panics if the same `prefix` is registered twice in one registry.
    pub fn new(prefix: &str, registry: &Registry, connection_counts_level: MetricLevel) -> Self {
        Self(Arc::new(Inner {
            prefix: prefix.to_owned(),
            registry: registry.clone(),
            metrics: Metrics::new(prefix, registry, connection_counts_level),
            tls: OnceLock::new(),
        }))
    }

    /// Registers the TLS metrics. The server calls it when it starts with TLS;
    /// a second call does nothing.
    pub(crate) fn enable_tls(&self) {
        self.0
            .tls
            .get_or_init(|| tls::Metrics::new(&self.0.prefix, &self.0.registry));
    }

    /// Observes the connections in the set of pending handshakes.
    pub(crate) fn observe_pending_handshakes(&self, pending: usize) {
        if let Some(tls) = self.0.tls.get() {
            tls.observe_pending_handshakes(pending);
        }
    }

    /// Counts a socket the listener has just accepted, before any TLS
    /// handshake.
    fn connection_accepted(&self, remote: SocketAddr) -> TrackedConnection {
        let metrics = &self.0.metrics;
        let scope: &str = Scope::of(remote.ip()).into();
        metrics
            .inbound_connections_accepted
            .with_label_values(&[scope])
            .inc();
        metrics.inbound_connections.inc();
        TrackedConnection::new(self.0.clone())
    }
}

struct Inner {
    prefix: String,
    registry: Registry,
    metrics: Metrics,
    /// Registered when the listener starts with TLS.
    tls: OnceLock<tls::Metrics>,
}

struct Metrics {
    inbound_connections_accepted: IntCounterVec,
    inbound_connections: IntGaugeWithPeakGauge,
    connection_lifetime_seconds: Histogram,
}

impl Metrics {
    fn new(prefix: &str, registry: &Registry, connection_counts_level: MetricLevel) -> Self {
        Self {
            inbound_connections_accepted: register_int_counter_vec_with_registry!(
                format!("{prefix}_inbound_connections_accepted"),
                "Sockets accepted, counted from the TCP accept, before any TLS handshake. \
                 The scope is where the remote address is: loopback, private or public",
                &["scope"],
                registry;
                connection_counts_level
            )
            .expect("the metrics of a listener register without collision"),
            inbound_connections: IntGaugeWithPeakGauge::register(
                (
                    &format!("{prefix}_inbound_connections"),
                    "The number of connections open now. A connection counts from its TCP accept \
                     to its close, TLS handshake included",
                ),
                (
                    &format!("{prefix}_inbound_connections_peak"),
                    "The most open connections over the last 2 minutes. Observed at each \
                     accept, and a scrape reports at least the current number",
                ),
                module_path!(),
                registry,
                MetricLevel::Info,
            ),
            connection_lifetime_seconds: register_histogram_with_registry!(
                format!("{prefix}_connection_lifetime_seconds"),
                "Time from the TCP accept to the close, in seconds",
                LIFETIME_SECONDS_BUCKETS.to_vec(),
                registry;
                MetricLevel::Info
            )
            .expect("the metrics of a listener register without collision"),
        }
    }
}

#[cfg(test)]
mod tests;
