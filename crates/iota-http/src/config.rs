// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, sync::Arc, time::Duration};

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
const DEFAULT_MAX_PENDING_CONNECTIONS: usize = 4096;
/// hyper's own default for `SETTINGS_MAX_CONCURRENT_STREAMS`
/// (`hyper::proto::h2::server`, 1.8). This crate passes its own setting to
/// hyper, so hyper's default applies only through this constant.
const HYPER_DEFAULT_MAX_CONCURRENT_STREAMS: u32 = 200;
/// hyper's own default for the stream window (`hyper::proto::h2::server`,
/// 1.8).
const HYPER_DEFAULT_STREAM_WINDOW: u32 = 1024 * 1024;
/// The largest HTTP/2 flow-control window (RFC 9113, section 6.9.1).
const MAX_WINDOW_SIZE: u32 = (1 << 31) - 1;

#[derive(Debug, Clone)]
pub struct Config {
    init_stream_window_size: Option<u32>,
    init_connection_window_size: Option<u32>,
    max_concurrent_streams: Option<u32>,
    pub(crate) tcp_keepalive: Option<Duration>,
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
    pub(crate) handshake_timeout: Option<Duration>,
    pub(crate) max_pending_connections: Option<usize>,
    pub(crate) max_connections_per_peer: Option<usize>,
    pub(crate) on_peer_connection_event: Option<OnPeerConnectionEvent>,
    pub(crate) concurrency_limit_per_connection: Option<usize>,
    pub(crate) metrics: Option<ListenerMetrics>,
}

/// The HTTP/2 settings that `concurrency_limit_per_connection` derives.
struct Http2Settings {
    max_concurrent_streams: Option<u32>,
    init_connection_window_size: Option<u32>,
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
            on_peer_connection_event: None,
            concurrency_limit_per_connection: None,
            metrics: None,
        }
    }
}

impl Config {
    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Default is 1 MiB, the default of hyper.
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
    /// Default is 1 MiB, the default of hyper.
    pub fn initial_connection_window_size(self, sz: impl Into<Option<u32>>) -> Self {
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
    pub fn max_concurrent_streams(self, max: impl Into<Option<u32>>) -> Self {
        Self {
            max_concurrent_streams: max.into(),
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
    /// Only connections that authenticate with a client certificate are
    /// counted, since a peer that presents none cannot be told apart from any
    /// other.
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

    /// Limits the responses in progress on each connection. A response is in
    /// progress from before its handler runs until h2 has dropped the last
    /// data chunk of it, so a response that the client reads slowly or not at
    /// all keeps its slot. Requests over the limit wait on the server, in
    /// arrival order. The limit applies to the requests that pass through a
    /// [`ResponseLimitLayer`](crate::ResponseLimitLayer) in the service; every
    /// request also gets the [`ResponseSlots`](crate::ResponseSlots) of its
    /// connection in its extensions, for a handler that takes slots itself.
    ///
    /// A waiting request does not read its request body, so with the limit
    /// set the listener announces its stream limit and opens the connection
    /// window to one stream window for each stream, unless a larger one is
    /// set. With a smaller connection window the request bodies of the
    /// waiting requests could take all of it, and a served request could not
    /// receive the rest of its own. Without
    /// [`max_concurrent_streams`](Self::max_concurrent_streams) the stream
    /// limit is hyper's default of 200; set it to make the announced limit
    /// part of the listener's own configuration.
    ///
    /// `Some(0)` counts the responses in the metrics without limiting them.
    /// Default is no limit and no counting (`None`).
    pub fn concurrency_limit_per_connection(self, limit: Option<usize>) -> Self {
        Self {
            concurrency_limit_per_connection: limit,
            ..self
        }
    }

    /// Measures the connections of the listener. The measuring changes no
    /// behaviour.
    ///
    /// Default is no measuring (`None`).
    pub fn metrics(self, metrics: Option<ListenerMetrics>) -> Self {
        Self { metrics, ..self }
    }

    /// The stream limit and the connection window that the listener announces
    /// with `concurrency_limit_per_connection` set, or why the settings do
    /// not allow the limit.
    fn settings_with_response_limit(&self) -> Result<Http2Settings, crate::BoxError> {
        let Some(limit) = self.concurrency_limit_per_connection else {
            return Ok(Http2Settings {
                max_concurrent_streams: self.max_concurrent_streams,
                init_connection_window_size: self.init_connection_window_size,
            });
        };
        if self.http2_adaptive_window == Some(true) {
            return Err(
                "'concurrency_limit_per_connection' needs fixed HTTP/2 windows, so it \
                 cannot be combined with an adaptive window"
                    .into(),
            );
        }
        let max_concurrent_streams = self
            .max_concurrent_streams
            .unwrap_or(HYPER_DEFAULT_MAX_CONCURRENT_STREAMS);
        // A response in progress uses a stream, so a higher limit could never
        // be reached.
        if limit > max_concurrent_streams as usize {
            return Err(format!(
                "'concurrency_limit_per_connection' is {limit}, but a connection has at most \
                 {max_concurrent_streams} streams; set it to at most that, or to 0 to turn the \
                 limit off"
            )
            .into());
        }
        let stream_window = self
            .init_stream_window_size
            .unwrap_or(HYPER_DEFAULT_STREAM_WINDOW);
        let window_for_all_streams = u64::from(max_concurrent_streams) * u64::from(stream_window);
        let window_for_all_streams = u32::try_from(window_for_all_streams)
            .ok()
            .filter(|window| *window <= MAX_WINDOW_SIZE)
            .ok_or_else(|| {
                format!(
                    "'concurrency_limit_per_connection' needs a connection window of \
                     {max_concurrent_streams} streams x {stream_window} bytes, which is over the \
                     largest HTTP/2 window of {MAX_WINDOW_SIZE} bytes; lower \
                     'max_concurrent_streams' or 'initial_stream_window_size'"
                )
            })?;
        let init_connection_window_size = match self.init_connection_window_size {
            Some(configured) if configured < window_for_all_streams => {
                return Err(format!(
                    "'initial_connection_window_size' is {configured}, but with \
                     'concurrency_limit_per_connection' it must hold the windows of all \
                     {max_concurrent_streams} streams, {window_for_all_streams} bytes"
                )
                .into());
            }
            Some(configured) => configured,
            None => window_for_all_streams,
        };
        Ok(Http2Settings {
            max_concurrent_streams: Some(max_concurrent_streams),
            init_connection_window_size: Some(init_connection_window_size),
        })
    }

    /// Rejects settings the accept loop cannot recover from, and a limit of
    /// responses per connection that the HTTP/2 settings do not allow.
    pub(crate) fn validate(&self) -> Result<(), crate::BoxError> {
        self.settings_with_response_limit()?;
        if self.max_connections_per_peer == Some(0) {
            return Err("'max_connections_per_peer' must be greater than zero, \
                        a peer allowed no connection can never be served"
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
        let Http2Settings {
            max_concurrent_streams,
            init_connection_window_size,
        } = self
            .settings_with_response_limit()
            .expect("the config was validated when the server started");

        builder
            .http2()
            .timer(hyper_util::rt::TokioTimer::new())
            .initial_connection_window_size(init_connection_window_size)
            .initial_stream_window_size(self.init_stream_window_size)
            .max_concurrent_streams(max_concurrent_streams)
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
