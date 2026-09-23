// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, sync::Arc, time::Duration};

const DEFAULT_HTTP2_KEEPALIVE_TIMEOUT_SECS: u64 = 20;
/// hyper's own default for the header read deadline; hyper only enforces it
/// when a timer is configured, which this crate does whenever it accepts
/// HTTP/1 at all.
const DEFAULT_HTTP1_HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Covers a round trip plus a few TCP retransmissions on a lossy link; an
/// unloaded TLS 1.3 handshake completes in one round trip.
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Every connection to a TLS listener passes through the handshake phase, so
/// together with the handshake deadline this bounds how many silent peers it
/// takes to make honest connections wait in the kernel backlog. Concurrent
/// handshakes only build up when peers are slow or silent, so this leaves ample
/// room for a legitimate reconnect burst.
const DEFAULT_MAX_PENDING_CONNECTIONS: usize = 4096;

#[derive(Debug, Clone)]
pub struct Config {
    init_stream_window_size: Option<u32>,
    init_connection_window_size: Option<u32>,
    max_concurrent_streams: Option<u32>,
    pub(crate) tcp_keepalive: Option<Duration>,
    pub(crate) tcp_nodelay: bool,
    http2_keepalive_interval: Option<Duration>,
    http2_keepalive_timeout: Option<Duration>,
    http2_adaptive_window: Option<bool>,
    http2_max_pending_accept_reset_streams: Option<usize>,
    http2_max_header_list_size: Option<u32>,
    max_frame_size: Option<u32>,
    pub(crate) accept_http1: bool,
    http1_header_read_timeout: Option<Duration>,
    enable_connect_protocol: bool,
    pub(crate) max_connection_age: Option<Duration>,
    pub(crate) max_connection_idle: Option<Duration>,
    pub(crate) handshake_timeout: Option<Duration>,
    pub(crate) max_pending_connections: Option<usize>,
    pub(crate) max_connections: Option<usize>,
    pub(crate) max_connections_per_peer: Option<usize>,
    pub(crate) on_peer_connection_event: Option<OnPeerConnectionEvent>,
    pub(crate) on_connection_event: Option<OnConnectionEvent>,
}

/// A change to the connections an authenticated peer holds, with the number
/// it holds afterwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerConnectionEvent {
    /// A connection was accepted and counted.
    Established { held: usize },
    /// A counted connection closed.
    Closed { held: usize },
    /// A further connection was closed because the peer already holds the
    /// limit.
    RefusedAtLimit { held: usize },
}

/// A change to the connections a listener holds, with the count it holds
/// afterwards.
///
/// Each variant carries the number the server itself is working from, rather
/// than a delta, so a consumer that stores it cannot drift away from the
/// server's own view.
///
/// `pending` counts connections whose TLS handshake is in progress — the same
/// number `max_pending_connections` is compared against. `live` counts
/// established connections being served. A connection that completes its
/// handshake leaves the first and joins the second, so it reports both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// A connection was accepted and its handshake started. Only listeners
    /// configured with TLS have this phase.
    HandshakeStarted { pending: usize },
    /// A handshake completed; `Established` follows for the same connection.
    HandshakeCompleted { pending: usize },
    /// A handshake ended without a connection: it timed out, failed, or its
    /// task panicked.
    HandshakeFailed { pending: usize },
    /// A connection is now being served.
    Established { live: usize },
    /// A served connection closed.
    Closed { live: usize },
    /// A connection was closed before being served because it was over a
    /// limit.
    Refused { live: usize },
}

type ConnectionCallback = Arc<dyn Fn(ConnectionEvent) + Send + Sync>;

/// Called on each change to the connections the listener holds.
#[derive(Clone)]
pub(crate) struct OnConnectionEvent(ConnectionCallback);

impl OnConnectionEvent {
    pub(crate) fn call(&self, event: ConnectionEvent) {
        (self.0)(event)
    }
}

impl fmt::Debug for OnConnectionEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OnConnectionEvent")
    }
}

type PeerConnectionCallback = Arc<dyn Fn(&[u8], PeerConnectionEvent) + Send + Sync>;

/// Called with the peer's public key on each of its connection events.
#[derive(Clone)]
pub(crate) struct OnPeerConnectionEvent(PeerConnectionCallback);

impl OnPeerConnectionEvent {
    pub(crate) fn call(&self, peer_public_key: &[u8], event: PeerConnectionEvent) {
        (self.0)(peer_public_key, event)
    }
}

impl fmt::Debug for OnPeerConnectionEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OnPeerConnectionEvent")
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            init_stream_window_size: None,
            init_connection_window_size: None,
            max_concurrent_streams: None,
            tcp_keepalive: None,
            tcp_nodelay: true,
            http2_keepalive_interval: None,
            http2_keepalive_timeout: None,
            http2_adaptive_window: None,
            http2_max_pending_accept_reset_streams: None,
            http2_max_header_list_size: None,
            max_frame_size: None,
            accept_http1: true,
            http1_header_read_timeout: Some(DEFAULT_HTTP1_HEADER_READ_TIMEOUT),
            enable_connect_protocol: true,
            max_connection_age: None,
            max_connection_idle: None,
            handshake_timeout: Some(DEFAULT_HANDSHAKE_TIMEOUT),
            max_pending_connections: Some(DEFAULT_MAX_PENDING_CONNECTIONS),
            max_connections: None,
            max_connections_per_peer: None,
            on_peer_connection_event: None,
            on_connection_event: None,
        }
    }
}

impl Config {
    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Default is 65,535
    ///
    /// [spec]: https://httpwg.org/specs/rfc9113.html#InitialWindowSize
    pub fn initial_stream_window_size(self, sz: impl Into<Option<u32>>) -> Self {
        Self {
            init_stream_window_size: sz.into(),
            ..self
        }
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Default is 65,535
    pub fn initial_connection_window_size(self, sz: impl Into<Option<u32>>) -> Self {
        Self {
            init_connection_window_size: sz.into(),
            ..self
        }
    }

    /// Sets the [`SETTINGS_MAX_CONCURRENT_STREAMS`][spec] option for HTTP2
    /// connections. This bounds the requests one connection may have in flight,
    /// so without it a single peer can occupy a whole service's admission
    /// slots.
    ///
    /// `None` leaves the transport's own default in place, currently 200.
    ///
    /// [spec]: https://httpwg.org/specs/rfc9113.html#n-stream-concurrency
    pub fn max_concurrent_streams(self, max: impl Into<Option<u32>>) -> Self {
        Self {
            max_concurrent_streams: max.into(),
            ..self
        }
    }

    /// Sets how long a connection may serve no request before it is closed.
    ///
    /// Idle means serving no request, not receiving no bytes: the server's own
    /// keepalive pings and the answers to them are bytes, so a peer that
    /// answers them and does nothing else stays idle by this measure. A
    /// connection that has not yet chosen a protocol is idle too, since it has
    /// started no request either.
    ///
    /// Keepalive closes a connection whose peer has *gone*; this closes one
    /// whose peer is present and doing nothing. Both are needed, and neither
    /// substitutes for the other.
    ///
    /// The deadline must exceed the longest gap between requests a legitimate
    /// peer leaves, or it will disconnect working clients.
    ///
    /// Default is no limit (`None`).
    pub fn max_connection_idle(self, max_connection_idle: Option<Duration>) -> Self {
        Self {
            max_connection_idle,
            ..self
        }
    }

    /// Sets the maximum time option in milliseconds that a connection may exist
    ///
    /// Default is no limit (`None`).
    pub fn max_connection_age(self, max_connection_age: Duration) -> Self {
        Self {
            max_connection_age: Some(max_connection_age),
            ..self
        }
    }

    /// Set whether HTTP2 Ping frames are enabled on accepted connections.
    ///
    /// If `None` is specified, HTTP2 keepalive is disabled, otherwise the
    /// duration specified will be the time interval between HTTP2 Ping
    /// frames. The timeout for receiving an acknowledgement of the
    /// keepalive ping can be set with [`Config::http2_keepalive_timeout`].
    ///
    /// Default is no HTTP2 keepalive (`None`)
    pub fn http2_keepalive_interval(self, http2_keepalive_interval: Option<Duration>) -> Self {
        Self {
            http2_keepalive_interval,
            ..self
        }
    }

    /// Sets a timeout for receiving an acknowledgement of the keepalive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if http2_keep_alive_interval is disabled.
    ///
    /// Default is 20 seconds.
    pub fn http2_keepalive_timeout(self, http2_keepalive_timeout: Option<Duration>) -> Self {
        Self {
            http2_keepalive_timeout,
            ..self
        }
    }

    /// Sets whether to use an adaptive flow control. Defaults to false.
    /// Enabling this will override the limits set in
    /// http2_initial_stream_window_size and
    /// http2_initial_connection_window_size.
    pub fn http2_adaptive_window(self, enabled: Option<bool>) -> Self {
        Self {
            http2_adaptive_window: enabled,
            ..self
        }
    }

    /// Configures the maximum number of pending reset streams allowed before a
    /// GOAWAY will be sent.
    ///
    /// This will default to whatever the default in h2 is. As of v0.3.17, it is
    /// 20.
    ///
    /// See <https://github.com/hyperium/hyper/issues/2877> for more information.
    pub fn http2_max_pending_accept_reset_streams(self, max: Option<usize>) -> Self {
        Self {
            http2_max_pending_accept_reset_streams: max,
            ..self
        }
    }

    /// Set whether TCP keepalive messages are enabled on accepted connections.
    ///
    /// If `None` is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP keepalive
    /// probes.
    ///
    /// Default is no keepalive (`None`)
    pub fn tcp_keepalive(self, tcp_keepalive: Option<Duration>) -> Self {
        Self {
            tcp_keepalive,
            ..self
        }
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections. Enabled
    /// by default.
    pub fn tcp_nodelay(self, enabled: bool) -> Self {
        Self {
            tcp_nodelay: enabled,
            ..self
        }
    }

    /// Sets the max size of received header frames.
    ///
    /// This will default to whatever the default in hyper is. As of v1.4.1, it
    /// is 16 KiB.
    pub fn http2_max_header_list_size(self, max: impl Into<Option<u32>>) -> Self {
        Self {
            http2_max_header_list_size: max.into(),
            ..self
        }
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Passing `None` will do nothing.
    ///
    /// If not set, will default from underlying transport.
    pub fn max_frame_size(self, frame_size: impl Into<Option<u32>>) -> Self {
        Self {
            max_frame_size: frame_size.into(),
            ..self
        }
    }

    /// Allow this accepting http1 requests.
    ///
    /// Default is `true`.
    pub fn accept_http1(self, accept_http1: bool) -> Self {
        Config {
            accept_http1,
            ..self
        }
    }

    /// Sets how long an HTTP/1 connection may take to send a complete request
    /// header block before it is closed. Until the headers arrive no request
    /// exists that a request deadline could apply to, so this is the only bound
    /// on a peer that stalls mid-headers.
    ///
    /// Default is 30 seconds. `None` disables the deadline.
    pub fn http1_header_read_timeout(self, timeout: Option<Duration>) -> Self {
        Config {
            http1_header_read_timeout: timeout,
            ..self
        }
    }

    /// Sets how long an accepted connection may take to complete its TLS
    /// handshake before it is closed. The peer is unauthenticated for the whole
    /// handshake, so without this a silent peer holds a task and a file
    /// descriptor indefinitely.
    ///
    /// Default is 5 seconds. `None` disables the deadline.
    pub fn handshake_timeout(self, handshake_timeout: Option<Duration>) -> Self {
        Self {
            handshake_timeout,
            ..self
        }
    }

    /// Sets how many accepted connections may be handshaking at the same time.
    /// While the limit is reached the server stops accepting, leaving new
    /// connections in the kernel backlog instead of holding file descriptors
    /// for them.
    ///
    /// Default is 4096. `None` removes the limit.
    pub fn max_pending_connections(self, max_pending_connections: Option<usize>) -> Self {
        Self {
            max_pending_connections,
            ..self
        }
    }

    /// Sets how many established connections a single peer may hold at once.
    /// Further connections from a peer already at the limit are closed as soon
    /// as they are accepted.
    ///
    /// Sets how many connections this listener may serve at once. Further
    /// connections are closed immediately after their handshake, before being
    /// served.
    ///
    /// This is the bound on file descriptors, and the only one: a per-peer
    /// limit permits one connection per peer per limit, and on a listener
    /// whose peers are not a known set that product is unbounded. The two are
    /// meant to be set together, this one to bound the listener and the other
    /// to stop one peer consuming all of it.
    ///
    /// The limit is enforced after the handshake rather than by refusing to
    /// accept, so that a full listener still answers new peers instead of
    /// leaving them in the kernel backlog with no way to tell a busy server
    /// from an unreachable one.
    ///
    /// Default is no limit (`None`).
    pub fn max_connections(self, max_connections: Option<usize>) -> Self {
        Self {
            max_connections,
            ..self
        }
    }

    /// Connections are counted under the peer's certificate public key, or,
    /// for a peer that presents no certificate, under the prefix its address
    /// belongs to.
    ///
    /// Default is no limit (`None`).
    pub fn max_connections_per_peer(self, max_connections_per_peer: Option<usize>) -> Self {
        Self {
            max_connections_per_peer,
            ..self
        }
    }

    /// Sets a callback invoked with the peer's public key each time one of its
    /// connections is established, closed or refused at the limit. Only
    /// connections counted under `max_connections_per_peer` are reported. It
    /// runs on the accept loop or a connection's task, so it must not block.
    /// Sets a callback invoked on each change to the connections this listener
    /// holds. It runs on the accept loop or a connection's task, so it must not
    /// block.
    ///
    /// A connection that is opened and then left silent is attributed nowhere
    /// else: it sends no request, so no request-level metric records it. This
    /// is the only place it is counted.
    pub fn on_connection_event(
        self,
        on_connection_event: impl Fn(ConnectionEvent) + Send + Sync + 'static,
    ) -> Self {
        Self {
            on_connection_event: Some(OnConnectionEvent(Arc::new(on_connection_event))),
            ..self
        }
    }

    pub fn on_peer_connection_event(
        self,
        on_peer_connection_event: impl Fn(&[u8], PeerConnectionEvent) + Send + Sync + 'static,
    ) -> Self {
        Self {
            on_peer_connection_event: Some(OnPeerConnectionEvent(Arc::new(
                on_peer_connection_event,
            ))),
            ..self
        }
    }

    /// Rejects settings the accept loop cannot recover from.
    pub(crate) fn validate(&self) -> Result<(), crate::BoxError> {
        if self.max_connections_per_peer == Some(0) {
            return Err("'max_connections_per_peer' must be greater than zero, \
                        a peer allowed no connection can never be served"
                .into());
        }

        if self.max_connections == Some(0) {
            return Err("'max_connections' must be greater than zero, \
                        a server that serves no connection is never useful"
                .into());
        }

        match self.max_pending_connections {
            Some(0) => Err("'max_pending_connections' must be greater than zero, \
                            a server that accepts no connection is never useful"
                .into()),
            // Reaching the limit stops accepting until a handshake finishes, so
            // without a deadline enough silent peers stall the server for good.
            Some(_) if self.handshake_timeout.is_none() => Err(
                "'max_pending_connections' requires a 'handshake_timeout' to release its \
                     slots"
                    .into(),
            ),
            _ => Ok(()),
        }
    }

    pub(crate) fn connection_builder(
        &self,
    ) -> hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor> {
        let mut builder =
            hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());

        if self.accept_http1 {
            builder
                .http1()
                .timer(hyper_util::rt::TokioTimer::new())
                .header_read_timeout(self.http1_header_read_timeout);
        } else {
            builder = builder.http2_only();
        }

        if self.enable_connect_protocol {
            builder.http2().enable_connect_protocol();
        }

        let http2_keepalive_timeout = self
            .http2_keepalive_timeout
            .unwrap_or_else(|| Duration::new(DEFAULT_HTTP2_KEEPALIVE_TIMEOUT_SECS, 0));

        // hyper assigns whatever it is given, so passing `None` would replace
        // its own protective default with no limit at all. `None` here means
        // "no opinion", which is hyper's default, not "unlimited" — the
        // adjacent `max_pending_accept_reset_streams` is guarded the same way.
        if let Some(max_concurrent_streams) = self.max_concurrent_streams {
            builder.http2().max_concurrent_streams(max_concurrent_streams);
        }

        builder
            .http2()
            .timer(hyper_util::rt::TokioTimer::new())
            .initial_connection_window_size(self.init_connection_window_size)
            .initial_stream_window_size(self.init_stream_window_size)
            .keep_alive_interval(self.http2_keepalive_interval)
            .keep_alive_timeout(http2_keepalive_timeout)
            .adaptive_window(self.http2_adaptive_window.unwrap_or_default())
            .max_pending_accept_reset_streams(self.http2_max_pending_accept_reset_streams)
            .max_frame_size(self.max_frame_size);

        if let Some(max_header_list_size) = self.http2_max_header_list_size {
            builder.http2().max_header_list_size(max_header_list_size);
        }

        builder
    }
}
