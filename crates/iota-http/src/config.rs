// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt,
    num::{NonZeroU32, NonZeroUsize},
    ops::RangeInclusive,
    sync::Arc,
    time::Duration,
};

use crate::metrics::ListenerMetrics;

const DEFAULT_HTTP2_KEEPALIVE_TIMEOUT_SECS: u64 = 20;
/// hyper's own default for the header read deadline; hyper only enforces it
/// when a timer is configured, which this crate always does.
const DEFAULT_HTTP1_HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Covers a round trip plus a few TCP retransmissions on a lossy link; an
/// unloaded TLS 1.3 handshake completes in one round trip.
const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
/// Every connection to a TLS listener passes through the handshake phase, so
/// together with the handshake deadline this bounds how many silent peers it
/// takes to make honest connections wait in the kernel backlog. Concurrent
/// handshakes only build up when peers are slow or silent, so this leaves ample
/// room for a legitimate reconnect burst.
const DEFAULT_MAX_PENDING_CONNECTIONS: NonZeroUsize = NonZeroUsize::new(4096).unwrap();
/// The largest HTTP/2 flow-control window (RFC 9113, section 6.9.1); h2 panics
/// on a larger connection window.
const HTTP2_MAX_WINDOW_SIZE: u32 = (1 << 31) - 1;
/// The frame sizes HTTP/2 allows (RFC 9113, section 6.5.2); h2 panics outside
/// them.
const HTTP2_MAX_FRAME_SIZE_RANGE: RangeInclusive<u32> = (1 << 14)..=(1 << 24) - 1;

#[derive(Debug, Clone)]
pub struct Config {
    init_stream_window_size: Option<NonZeroU32>,
    init_connection_window_size: Option<NonZeroU32>,
    max_concurrent_streams: Option<NonZeroU32>,
    pub(crate) tcp_keepalive: Option<Duration>,
    http2_keepalive_interval: Option<Duration>,
    http2_keepalive_timeout: Option<Duration>,
    http2_adaptive_window: Option<bool>,
    http2_max_pending_accept_reset_streams: Option<NonZeroUsize>,
    http2_max_header_list_size: Option<NonZeroU32>,
    max_frame_size: Option<NonZeroU32>,
    pub(crate) accept_http1: bool,
    http1_header_read_timeout: Option<Duration>,
    enable_connect_protocol: bool,
    pub(crate) max_connection_age: Option<Duration>,
    pub(crate) handshake_timeout: Option<Duration>,
    pub(crate) max_pending_connections: Option<NonZeroUsize>,
    pub(crate) max_connections_per_peer: Option<NonZeroUsize>,
    pub(crate) max_connections: Option<NonZeroUsize>,
    pub(crate) max_connection_idle: Option<Duration>,
    pub(crate) on_peer_connection_event: Option<OnPeerConnectionEvent>,
    pub(crate) metrics: Option<ListenerMetrics>,
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
            handshake_timeout: Some(DEFAULT_HANDSHAKE_TIMEOUT),
            max_pending_connections: Some(DEFAULT_MAX_PENDING_CONNECTIONS),
            max_connections_per_peer: None,
            max_connections: None,
            max_connection_idle: None,
            on_peer_connection_event: None,
            metrics: None,
        }
    }
}

impl Config {
    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// At most 2^31 - 1, the largest window HTTP/2 allows.
    ///
    /// Default is 1 MiB, the default of hyper.
    ///
    /// [spec]: https://httpwg.org/specs/rfc9113.html#InitialWindowSize
    pub fn initial_stream_window_size(self, sz: impl Into<Option<NonZeroU32>>) -> Self {
        Self {
            init_stream_window_size: sz.into(),
            ..self
        }
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// At most 2^31 - 1, the largest window HTTP/2 allows.
    ///
    /// Default is 1 MiB, the default of hyper.
    pub fn initial_connection_window_size(self, sz: impl Into<Option<NonZeroU32>>) -> Self {
        Self {
            init_connection_window_size: sz.into(),
            ..self
        }
    }

    /// Sets the [`SETTINGS_MAX_CONCURRENT_STREAMS`][spec] option for HTTP2
    /// connections.
    ///
    /// Default is no limit (`None`).
    ///
    /// [spec]: https://httpwg.org/specs/rfc9113.html#n-stream-concurrency
    pub fn max_concurrent_streams(self, max: impl Into<Option<NonZeroU32>>) -> Self {
        Self {
            max_concurrent_streams: max.into(),
            ..self
        }
    }

    /// Sets the maximum time option in milliseconds that a connection may exist
    ///
    /// Default is no limit (`None`).
    pub fn max_connection_age(self, max_connection_age: impl Into<Option<Duration>>) -> Self {
        Self {
            max_connection_age: max_connection_age.into(),
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
    pub fn http2_keepalive_interval(
        self,
        http2_keepalive_interval: impl Into<Option<Duration>>,
    ) -> Self {
        Self {
            http2_keepalive_interval: http2_keepalive_interval.into(),
            ..self
        }
    }

    /// Sets a timeout for receiving an acknowledgement of the keepalive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if http2_keep_alive_interval is disabled.
    ///
    /// Default is 20 seconds.
    pub fn http2_keepalive_timeout(
        self,
        http2_keepalive_timeout: impl Into<Option<Duration>>,
    ) -> Self {
        Self {
            http2_keepalive_timeout: http2_keepalive_timeout.into(),
            ..self
        }
    }

    /// Sets whether to use an adaptive flow control. Defaults to false.
    /// Enabling this will override the limits set in
    /// http2_initial_stream_window_size and
    /// http2_initial_connection_window_size.
    pub fn http2_adaptive_window(self, enabled: impl Into<Option<bool>>) -> Self {
        Self {
            http2_adaptive_window: enabled.into(),
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
    pub fn http2_max_pending_accept_reset_streams(
        self,
        max: impl Into<Option<NonZeroUsize>>,
    ) -> Self {
        Self {
            http2_max_pending_accept_reset_streams: max.into(),
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
    pub fn tcp_keepalive(self, tcp_keepalive: impl Into<Option<Duration>>) -> Self {
        Self {
            tcp_keepalive: tcp_keepalive.into(),
            ..self
        }
    }

    /// Sets the max size of received header frames.
    ///
    /// This will default to whatever the default in hyper is. As of v1.4.1, it
    /// is 16 KiB.
    pub fn http2_max_header_list_size(self, max: impl Into<Option<NonZeroU32>>) -> Self {
        Self {
            http2_max_header_list_size: max.into(),
            ..self
        }
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// HTTP/2 allows 16,384 to 16,777,215; any other value is rejected when the
    /// server starts.
    ///
    /// If not set, will default from underlying transport.
    pub fn max_frame_size(self, frame_size: impl Into<Option<NonZeroU32>>) -> Self {
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

    /// Sets whether HTTP/2 connections offer the extended CONNECT protocol
    /// ([RFC 8441]), which lets a client tunnel another protocol, such as
    /// WebSocket, through an HTTP/2 stream.
    ///
    /// Default is `true`.
    ///
    /// [RFC 8441]: https://www.rfc-editor.org/rfc/rfc8441
    pub fn enable_connect_protocol(self, enable_connect_protocol: bool) -> Self {
        Config {
            enable_connect_protocol,
            ..self
        }
    }

    /// Sets how long an HTTP/1 connection may take to send a complete request
    /// header block before it is closed. Until the headers arrive no request
    /// exists that a request deadline could apply to, so this is the only bound
    /// on a peer that stalls mid-headers.
    ///
    /// Default is 30 seconds. `None` disables the deadline.
    pub fn http1_header_read_timeout(self, timeout: impl Into<Option<Duration>>) -> Self {
        Config {
            http1_header_read_timeout: timeout.into(),
            ..self
        }
    }

    /// Sets how long an accepted connection may take to complete its TLS
    /// handshake before it is closed. The peer is unauthenticated for the whole
    /// handshake, so without this a silent peer holds a task and a file
    /// descriptor indefinitely.
    ///
    /// Default is 5 seconds. `None` disables the deadline.
    pub fn handshake_timeout(self, handshake_timeout: impl Into<Option<Duration>>) -> Self {
        Self {
            handshake_timeout: handshake_timeout.into(),
            ..self
        }
    }

    /// Sets how many accepted connections may be handshaking at the same time.
    /// While the limit is reached the server stops accepting, leaving new
    /// connections in the kernel backlog instead of holding file descriptors
    /// for them.
    ///
    /// Default is 4096. `None` removes the limit.
    pub fn max_pending_connections(
        self,
        max_pending_connections: impl Into<Option<NonZeroUsize>>,
    ) -> Self {
        Self {
            max_pending_connections: max_pending_connections.into(),
            ..self
        }
    }

    /// Sets how many established connections a single peer may hold at once.
    /// Further connections from a peer already at the limit are closed as soon
    /// as they are accepted.
    ///
    /// Only connections that authenticate with a client certificate are
    /// counted, since a peer that presents none cannot be told apart from any
    /// other.
    ///
    /// Default is no limit (`None`).
    pub fn max_connections_per_peer(
        self,
        max_connections_per_peer: impl Into<Option<NonZeroUsize>>,
    ) -> Self {
        Self {
            max_connections_per_peer: max_connections_per_peer.into(),
            ..self
        }
    }

    /// Sets how many connections this listener may hold at once, counting
    /// those still in their TLS handshake. Further connections are closed as
    /// soon as they are accepted, before any handshake, so a full listener
    /// still answers a new peer rather than leaving it in the kernel backlog.
    ///
    /// A peer that never finishes its handshake holds a slot until
    /// [`handshake_timeout`](Self::handshake_timeout).
    ///
    /// Default is no limit (`None`).
    pub fn max_connections(self, max_connections: impl Into<Option<NonZeroUsize>>) -> Self {
        Self {
            max_connections: max_connections.into(),
            ..self
        }
    }

    /// Sets how long a connection may serve no request before it is closed.
    ///
    /// Keepalive closes a connection whose peer has gone; this closes one
    /// whose peer is present and doing nothing. Idle means no request, not no
    /// bytes: the server's own keepalive pings and the answers to them are
    /// bytes, so a peer that answers them and asks for nothing is idle. A
    /// connection that has not yet chosen a protocol is idle too.
    ///
    /// Must exceed the longest gap between requests a peer that is behaving
    /// leaves.
    ///
    /// Default is no deadline (`None`).
    pub fn max_connection_idle(self, max_connection_idle: impl Into<Option<Duration>>) -> Self {
        Self {
            max_connection_idle: max_connection_idle.into(),
            ..self
        }
    }

    /// Sets a callback invoked with the peer's public key each time one of its
    /// connections is established, closed or refused at the limit. Only
    /// connections counted under `max_connections_per_peer` are reported. It
    /// runs on the accept loop or a connection's task, so it must not block.
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

    /// Measures the connections of the listener. The measuring changes no
    /// behaviour.
    ///
    /// Default is no measuring (`None`).
    pub fn metrics(self, metrics: impl Into<Option<ListenerMetrics>>) -> Self {
        Self {
            metrics: metrics.into(),
            ..self
        }
    }

    /// Rejects settings the accept loop cannot recover from, and those that
    /// would leave it unable to serve anything.
    pub(crate) fn validate(&self) -> Result<(), crate::BoxError> {
        // A zero deadline is reached at once and a zero interval never pauses,
        // so either would close or flood every connection.
        for (name, duration) in [
            ("tcp_keepalive", self.tcp_keepalive),
            ("http2_keepalive_interval", self.http2_keepalive_interval),
            ("http2_keepalive_timeout", self.http2_keepalive_timeout),
            ("http1_header_read_timeout", self.http1_header_read_timeout),
            ("max_connection_age", self.max_connection_age),
            ("handshake_timeout", self.handshake_timeout),
            ("max_connection_idle", self.max_connection_idle),
        ] {
            if duration == Some(Duration::ZERO) {
                return Err(format!("'{name}' must be greater than zero").into());
            }
        }

        for (name, size) in [
            ("initial_stream_window_size", self.init_stream_window_size),
            (
                "initial_connection_window_size",
                self.init_connection_window_size,
            ),
        ] {
            if size.is_some_and(|size| size.get() > HTTP2_MAX_WINDOW_SIZE) {
                return Err(format!("'{name}' must be at most {HTTP2_MAX_WINDOW_SIZE}").into());
            }
        }

        if self
            .max_frame_size
            .is_some_and(|size| !HTTP2_MAX_FRAME_SIZE_RANGE.contains(&size.get()))
        {
            return Err(format!(
                "'max_frame_size' must be between {} and {}",
                HTTP2_MAX_FRAME_SIZE_RANGE.start(),
                HTTP2_MAX_FRAME_SIZE_RANGE.end()
            )
            .into());
        }

        // Reaching the limit stops accepting until a handshake finishes, so
        // without a deadline enough silent peers stall the server for good.
        if self.max_pending_connections.is_some() && self.handshake_timeout.is_none() {
            return Err(
                "'max_pending_connections' requires a 'handshake_timeout' to release its slots"
                    .into(),
            );
        }

        Ok(())
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

        builder
            .http2()
            .timer(hyper_util::rt::TokioTimer::new())
            .initial_connection_window_size(self.init_connection_window_size.map(NonZeroU32::get))
            .initial_stream_window_size(self.init_stream_window_size.map(NonZeroU32::get))
            .max_concurrent_streams(self.max_concurrent_streams.map(NonZeroU32::get))
            .keep_alive_interval(self.http2_keepalive_interval)
            .keep_alive_timeout(http2_keepalive_timeout)
            .adaptive_window(self.http2_adaptive_window.unwrap_or_default())
            .max_pending_accept_reset_streams(
                self.http2_max_pending_accept_reset_streams
                    .map(NonZeroUsize::get),
            )
            .max_frame_size(self.max_frame_size.map(NonZeroU32::get));

        if let Some(max_header_list_size) = self.http2_max_header_list_size {
            builder
                .http2()
                .max_header_list_size(max_header_list_size.get());
        }

        builder
    }
}
