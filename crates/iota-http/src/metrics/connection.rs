// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use parking_lot::Mutex;

use crate::metrics::{
    Inner,
    requests::IdleEnd,
    tls::{self, HandshakeGuard},
};

/// The HTTP version of the first request of a connection.
#[derive(Clone, Copy, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub(super) enum Protocol {
    H1,
    H2,
}

impl Protocol {
    fn of(version: http::Version) -> Self {
        if version == http::Version::HTTP_2 {
            Self::H2
        } else {
            Self::H1
        }
    }
}

/// Whether a connection carried a request.
#[derive(Clone, Copy, strum::IntoStaticStr)]
pub(super) enum Used {
    #[strum(serialize = "false")]
    No,
    #[strum(serialize = "true")]
    Yes,
}

/// One accepted connection. Dropping it records the close of the connection.
///
/// The requests of the connection share its state: see
/// [`handle`](Self::handle).
pub(crate) struct ConnectionGuard(Arc<ConnectionState>);

impl ConnectionGuard {
    pub(super) fn new(listener: Arc<Inner>) -> Self {
        Self(Arc::new(ConnectionState {
            listener,
            accepted_at: Instant::now(),
            requests: Mutex::new(ConnectionRequests::default()),
        }))
    }

    /// A handle that the requests of the connection use. It does not keep the
    /// connection open, and its drop records nothing.
    pub(crate) fn handle(&self) -> ConnectionHandle {
        ConnectionHandle(self.0.clone())
    }

    /// Records the start of a TLS handshake of this connection; the handshake
    /// itself is run by the caller. Returns `None` when the listener has no
    /// TLS metrics.
    pub(crate) fn record_handshake_start(&self) -> Option<HandshakeGuard> {
        let tls = self.0.listener.tls.get()?;
        Some(tls::record_handshake_start(tls))
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let state = &*self.0;
        let now = Instant::now();
        let (idle, requests_carried) = {
            let requests = state.requests.lock();
            (requests.idle_time(now), requests.requests_carried)
        };
        let age = now.saturating_duration_since(state.accepted_at);

        if let Some((idle, protocol)) = idle {
            state
                .listener
                .requests
                .idle(IdleEnd::Close, protocol)
                .observe(idle.as_secs_f64());
        }
        let used = if requests_carried > 0 {
            Used::Yes
        } else {
            Used::No
        };
        let metrics = &state.listener.metrics;
        metrics.lifetime(used).observe(age.as_secs_f64());
        metrics.inbound_connections.dec();
    }
}

/// The state of a connection that its requests share. Cloning is cheap.
#[derive(Clone)]
pub(crate) struct ConnectionHandle(Arc<ConnectionState>);

impl ConnectionHandle {
    /// Marks the connection as upgraded (HTTP upgrade or CONNECT tunnel). Its
    /// time until the close is no idle gap.
    pub(super) fn mark_upgraded(&self) {
        self.0.requests.lock().upgraded = true;
    }

    /// Records a request start. The request ends when the guard is dropped.
    pub(super) fn start_request(&self, version: http::Version) -> RequestGuard {
        let state = &*self.0;
        let requests = &state.listener.requests;

        let now = Instant::now();
        let age = now.saturating_duration_since(state.accepted_at);
        let start = state.requests.lock().start_request(version, now);

        if start.first {
            requests
                .first_request(start.protocol)
                .observe(age.as_secs_f64());
        }
        if let Some(gap) = start.idle_gap {
            requests
                .idle(IdleEnd::Request, start.protocol)
                .observe(gap.as_secs_f64());
        }
        requests.inflight().observe(start.in_flight as f64);
        requests.record_peak_inflight(start.in_flight, now);

        RequestGuard {
            connection: self.clone(),
            started_at: now,
            ready_at: None,
        }
    }
}

struct ConnectionState {
    listener: Arc<Inner>,
    accepted_at: Instant,
    requests: Mutex<ConnectionRequests>,
}

/// The requests of one connection. One lock keeps the idle gaps right when a
/// request starts on one thread and another ends on a second thread.
#[derive(Default)]
struct ConnectionRequests {
    in_flight: u64,
    requests_carried: u64,
    /// The HTTP version of the first request, once there is one.
    protocol: Option<Protocol>,
    /// When the connection last had no request in flight.
    idle_since: Option<Instant>,
    /// The HTTP/1 connection was upgraded (HTTP upgrade or CONNECT tunnel). What
    /// it carries after that is no request.
    upgraded: bool,
}

/// What the start of a request changed on its connection.
struct RequestStart {
    /// The request is the first of the connection.
    first: bool,
    /// The time the connection had no request in flight until this request.
    idle_gap: Option<Duration>,
    /// The requests in flight, with this one.
    in_flight: u64,
    protocol: Protocol,
}

impl ConnectionRequests {
    fn start_request(&mut self, version: http::Version, now: Instant) -> RequestStart {
        let first = self.requests_carried == 0;
        let protocol = *self.protocol.get_or_insert_with(|| Protocol::of(version));
        let idle_gap = self
            .idle_since
            .take()
            .map(|since| now.saturating_duration_since(since));
        self.in_flight += 1;
        self.requests_carried += 1;
        RequestStart {
            first,
            idle_gap,
            in_flight: self.in_flight,
            protocol,
        }
    }

    fn end_request(&mut self, now: Instant) {
        self.in_flight = self.in_flight.saturating_sub(1);
        if self.in_flight == 0 {
            self.idle_since = Some(now);
        }
    }

    /// Whether the connection is idle: it carried a request, none is in flight,
    /// and it is not upgraded.
    fn is_idle(&self) -> bool {
        self.in_flight == 0 && self.requests_carried > 0 && !self.upgraded
    }

    /// How long the connection has been idle at `now`, with the HTTP version of
    /// its first request.
    fn idle_time(&self, now: Instant) -> Option<(Duration, Protocol)> {
        if !self.is_idle() {
            return None;
        }
        let since = self.idle_since?;
        Some((now.saturating_duration_since(since), self.protocol?))
    }
}

/// A request in flight. It lives until the response future, the response body
/// and every response chunk are gone.
pub(super) struct RequestGuard {
    connection: ConnectionHandle,
    started_at: Instant,
    /// When the response future returned the response.
    ready_at: Option<Instant>,
}

impl RequestGuard {
    /// Marks the response as ready. Call it once, before the guard is shared.
    pub(super) fn mark_response_ready(&mut self) {
        self.ready_at = Some(Instant::now());
    }

    /// Marks the connection as upgraded by the response (HTTP upgrade or CONNECT
    /// tunnel).
    pub(super) fn mark_upgraded(&self) {
        self.connection.mark_upgraded();
    }

    /// Wraps `bytes` so that the request stays in flight until the transport
    /// drops them. The data is not copied.
    pub(super) fn wrap_chunk(self: &Arc<Self>, bytes: Bytes) -> Bytes {
        if bytes.is_empty() {
            return bytes;
        }
        Bytes::from_owner(TrackedChunk {
            bytes,
            _guard: self.clone(),
        })
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let state = &*self.connection.0;
        let now = Instant::now();
        state.requests.lock().end_request(now);
        let requests = &state.listener.requests;
        requests
            .completion()
            .observe(now.saturating_duration_since(self.started_at).as_secs_f64());
        if let Some(ready_at) = self.ready_at {
            requests
                .delivery()
                .observe(now.saturating_duration_since(ready_at).as_secs_f64());
        }
    }
}

/// A response chunk the transport holds. Its drop is the moment the transport
/// lets go of the data.
struct TrackedChunk {
    bytes: Bytes,
    _guard: Arc<RequestGuard>,
}

impl AsRef<[u8]> for TrackedChunk {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
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
        assert_eq!(
            reader
                .histogram_totals("connection_lifetime_seconds", &[("used", "true")])
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
        let connection = metrics.record_accept("127.0.0.1:1000".parse().unwrap());
        let request = connection.handle().start_request(http::Version::HTTP_11);
        assert_eq!(reader.value("inbound_connections", &[]), 1.0);

        drop(connection);
        assert_closed(&reader);

        drop(request);
        assert_closed(&reader);
    }
}
