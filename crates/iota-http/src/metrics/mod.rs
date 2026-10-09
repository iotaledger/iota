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

pub(crate) use connection::ConnectionGuard;
pub(crate) use io::TrackedIo;
use scope::Scope;
pub(crate) use tls::HandshakeResult;

/// Connection lifetimes in seconds.
const CONNECTION_LIFETIME_SEC_BUCKETS: &[f64] =
    &[1.0, 5.0, 30.0, 60.0, 300.0, 1800.0, 14400.0, 86400.0];

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
    /// Registers the metrics as `{prefix}_<name>` in `registry`, all at
    /// `level`. The TLS metrics are registered later, by `enable_tls`.
    ///
    /// # Panics
    ///
    /// Panics if the same `prefix` is registered twice in one registry.
    pub fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        Self(Arc::new(Inner {
            prefix: prefix.to_owned(),
            registry: registry.clone(),
            level,
            metrics: Metrics::new(prefix, registry, level),
            tls: OnceLock::new(),
        }))
    }

    /// Registers the TLS metrics. The server calls it when it starts with TLS;
    /// a second call does nothing.
    pub(crate) fn enable_tls(&self) {
        self.0.tls.get_or_init(|| {
            Arc::new(tls::Metrics::new(
                &self.0.prefix,
                &self.0.registry,
                self.0.level,
            ))
        });
    }

    /// Counts a socket the listener has just accepted, before any TLS
    /// handshake.
    fn record_accept(&self, remote: SocketAddr) -> ConnectionGuard {
        let metrics = &self.0.metrics;
        let scope: &str = Scope::of(remote.ip()).into();
        metrics
            .inbound_connections_accepted
            .with_label_values(&[scope])
            .inc();
        metrics.inbound_connections.inc();
        ConnectionGuard::new(self.0.clone())
    }
}

struct Inner {
    prefix: String,
    registry: Registry,
    level: MetricLevel,
    metrics: Metrics,
    /// Registered when the listener starts with TLS.
    tls: OnceLock<Arc<tls::Metrics>>,
}

struct Metrics {
    inbound_connections_accepted: IntCounterVec,
    inbound_connections: IntGaugeWithPeakGauge,
    connection_lifetime_seconds: Histogram,
}

impl Metrics {
    fn new(prefix: &str, registry: &Registry, level: MetricLevel) -> Self {
        Self {
            inbound_connections_accepted: register_int_counter_vec_with_registry!(
                format!("{prefix}_inbound_connections_accepted"),
                "Sockets accepted, counted from the TCP accept, before any TLS handshake. \
                 The scope is where the remote address is: loopback, private or public",
                &["scope"],
                registry;
                level
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
                    "The most open connections over the last 2 minutes. A scrape reports at \
                     least the current number",
                ),
                module_path!(),
                registry,
                level,
            ),
            connection_lifetime_seconds: register_histogram_with_registry!(
                format!("{prefix}_connection_lifetime_seconds"),
                "Time from the TCP accept to the close, in seconds",
                CONNECTION_LIFETIME_SEC_BUCKETS.to_vec(),
                registry;
                level
            )
            .expect("the metrics of a listener register without collision"),
        }
    }
}

#[cfg(test)]
mod tests;
