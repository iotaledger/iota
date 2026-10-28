// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use parking_lot::Mutex;

use crate::metrics::{
    Inner,
    clients::ClientKey,
    requests::IdleEnd,
    tls::{self, HandshakeGuard},
};

/// One accepted connection. Dropping it records the close of the connection.
///
/// The requests of the connection share its state: see
/// [`metrics`](Self::metrics).
#[must_use = "dropping the guard records the close of the connection"]
pub(crate) struct ConnectionGuard(Arc<ConnectionState>);

impl ConnectionGuard {
    pub(super) fn new(listener: Arc<Inner>, client: ClientKey) -> Self {
        Self(Arc::new(ConnectionState {
            listener,
            client,
            accepted_at: Instant::now(),
            requests: Mutex::new(ConnectionRequests::default()),
        }))
    }

    /// The metrics of the connection, for its requests. They do not keep the
    /// connection open, and their drop records nothing.
    pub(crate) fn metrics(&self) -> ConnectionMetrics {
        ConnectionMetrics(self.0.clone())
    }

    /// Tracks a TLS handshake of this connection until the guard is dropped;
    /// the handshake itself is run by the caller. Returns `None` when the
    /// listener has no TLS metrics.
    #[must_use = "dropping the guard records the end of the handshake"]
    pub(crate) fn track_handshake(&self) -> Option<HandshakeGuard> {
        let tls_metrics = self.0.listener.tls.get()?;
        Some(tls::track_handshake(tls_metrics))
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let state = &*self.0;
        let now = Instant::now();
        let idle_time = state.requests.lock().idle_time(now);
        let lifetime = now.saturating_duration_since(state.accepted_at);

        if let Some(idle_time) = idle_time {
            state
                .listener
                .requests
                .connection_idle_seconds(IdleEnd::Close)
                .observe(idle_time.as_secs_f64());
        }
        let metrics = &state.listener.metrics;
        metrics
            .connection_lifetime_seconds
            .observe(lifetime.as_secs_f64());
        metrics.inbound_connections.dec();
        state.listener.clients.remove_connection(state.client);
    }
}

/// The metrics of a connection that its requests share. Cloning is cheap.
#[derive(Clone)]
pub(crate) struct ConnectionMetrics(Arc<ConnectionState>);

impl ConnectionMetrics {
    /// Tracks a request until the guard is dropped.
    pub(super) fn track_request(&self) -> RequestGuard {
        let state = &*self.0;
        let request_metrics = &state.listener.requests;

        let now = Instant::now();
        let connection_at_start = state.requests.lock().record_start(now);

        if let Some(idle_time) = connection_at_start.idle_time {
            request_metrics
                .connection_idle_seconds(IdleEnd::Request)
                .observe(idle_time.as_secs_f64());
        }
        request_metrics
            .connection_inflight_requests()
            .observe(connection_at_start.in_flight as f64);
        request_metrics.record_peak_inflight(connection_at_start.in_flight, now);

        RequestGuard {
            connection: self.clone(),
            started_at: now,
            ready_at: None,
        }
    }
}

struct ConnectionState {
    listener: Arc<Inner>,
    client: ClientKey,
    accepted_at: Instant,
    requests: Mutex<ConnectionRequests>,
}

/// The requests of one connection. One lock keeps the idle gaps right when a
/// request starts on one thread and another ends on a second thread.
#[derive(Default)]
struct ConnectionRequests {
    in_flight: u64,
    /// When the last request in flight ended. `None` while a request is in
    /// flight, and before the first request ends.
    idle_since: Option<Instant>,
}

/// The state of a connection when a request starts on it.
struct ConnectionAtRequestStart {
    /// The time the connection had no request in flight until this request.
    idle_time: Option<Duration>,
    /// The requests in flight, with this one.
    in_flight: u64,
}

impl ConnectionRequests {
    fn record_start(&mut self, now: Instant) -> ConnectionAtRequestStart {
        let idle_time = self.idle_time(now);
        self.idle_since = None;
        self.in_flight += 1;
        ConnectionAtRequestStart {
            idle_time,
            in_flight: self.in_flight,
        }
    }

    fn record_end(&mut self, now: Instant) {
        self.in_flight = self.in_flight.saturating_sub(1);
        if self.in_flight == 0 {
            self.idle_since = Some(now);
        }
    }

    /// How long the connection has had no request in flight at `now`, after
    /// its first request.
    fn idle_time(&self, now: Instant) -> Option<Duration> {
        self.idle_since
            .map(|idle_since| now.saturating_duration_since(idle_since))
    }
}

/// A request in flight. It lives until the response future and the response
/// body are done.
#[must_use = "dropping the guard records the end of the request"]
pub(super) struct RequestGuard {
    connection: ConnectionMetrics,
    started_at: Instant,
    /// When the response future returned the response.
    ready_at: Option<Instant>,
}

impl RequestGuard {
    /// Records that the response is ready.
    pub(super) fn record_response_ready(&mut self) {
        self.ready_at = Some(Instant::now());
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let state = &*self.connection.0;
        let now = Instant::now();
        state.requests.lock().record_end(now);
        let request_metrics = &state.listener.requests;
        request_metrics
            .request_completion_latency()
            .observe(now.saturating_duration_since(self.started_at).as_secs_f64());
        if let Some(ready_at) = self.ready_at {
            request_metrics
                .response_delivery_latency()
                .observe(now.saturating_duration_since(ready_at).as_secs_f64());
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_metrics::test_utils::MetricsReader;

    use crate::metrics::tests::setup;

    /// The effects of the close of a connection that carried a request, which
    /// was in flight at the close.
    fn assert_closed(reader: &MetricsReader) {
        assert_eq!(reader.value("inbound_connections", &[]), 0.0);
        assert_eq!(reader.value("clients", &[("scope", "loopback")]), 0.0);
        assert_eq!(
            reader
                .histogram_totals("connection_lifetime_seconds", &[])
                .count,
            1
        );
        assert_eq!(
            reader
                .histogram_totals("connection_idle_seconds", &[("end", "close")])
                .count,
            0
        );
    }

    /// A request can outlive its connection: it holds the state of the
    /// connection after the guard is gone. The close is the drop of the guard,
    /// not the release of the state.
    #[test]
    fn the_close_is_recorded_when_the_guard_drops_while_a_request_holds_the_state() {
        let (metrics, reader) = setup();
        let connection = metrics.track_connection("127.0.0.1:1000".parse().unwrap());
        let request = connection.metrics().track_request();
        assert_eq!(reader.value("inbound_connections", &[]), 1.0);
        assert_eq!(reader.value("clients", &[("scope", "loopback")]), 1.0);

        drop(connection);
        assert_closed(&reader);

        drop(request);
        assert_closed(&reader);
    }
}
