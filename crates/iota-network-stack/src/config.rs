// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use eyre::Result;
use serde::{Deserialize, Serialize};
use tokio_rustls::rustls::ClientConfig;
use tonic::transport::Channel;

use crate::{
    Multiaddr,
    client::{connect_lazy_with_config, connect_with_config},
    metrics::{DefaultMetricsCallbackProvider, MetricsCallbackProvider},
    server::ServerBuilder,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    /// Set the concurrency limit applied to on requests inbound per connection.
    pub concurrency_limit_per_connection: Option<usize>,

    /// Set a timeout for all request handlers.
    pub request_timeout: Option<Duration>,

    /// Set a timeout for establishing an outbound connection.
    pub connect_timeout: Option<Duration>,

    /// Sets the SETTINGS_INITIAL_WINDOW_SIZE option for HTTP2 stream-level flow
    /// control. Default is 65,535
    pub http2_initial_stream_window_size: Option<u32>,

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Default is 65,535
    pub http2_initial_connection_window_size: Option<u32>,

    /// Sets the SETTINGS_MAX_CONCURRENT_STREAMS option for HTTP2 connections.
    ///
    /// Default is no limit (None).
    pub http2_max_concurrent_streams: Option<u32>,

    /// Set whether TCP keepalive messages are enabled on accepted connections.
    ///
    /// If None is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP
    /// keepalive probes.
    ///
    /// Default is no keepalive (None)
    pub tcp_keepalive: Option<Duration>,

    /// Set the value of TCP_NODELAY option for accepted connections. Enabled by
    /// default.
    pub tcp_nodelay: Option<bool>,

    /// Set whether HTTP2 Ping frames are enabled on accepted connections.
    ///
    /// If None is specified, HTTP2 keepalive is disabled, otherwise the
    /// duration specified will be the time interval between HTTP2 Ping
    /// frames. The timeout for receiving an acknowledgement
    /// of the keepalive ping can be set with http2_keepalive_timeout.
    ///
    /// Default is no HTTP2 keepalive (None)
    pub http2_keepalive_interval: Option<Duration>,

    /// Sets a timeout for receiving an acknowledgement of the keepalive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if http2_keep_alive_interval is disabled.
    ///
    /// Default is 20 seconds.
    pub http2_keepalive_timeout: Option<Duration>,

    /// Only affects servers. How long a connection may serve no request before
    /// it is closed. Keepalive closes a connection whose peer has gone; this
    /// closes one whose peer is present and doing nothing.
    ///
    /// Must exceed the longest gap between requests a legitimate peer leaves.
    ///
    /// Default is no limit (`None`).
    pub max_connection_idle: Option<Duration>,

    /// Only affects servers. How long a connection may exist at all, however
    /// busy.
    ///
    /// Default is no limit (`None`).
    pub max_connection_age: Option<Duration>,

    /// Only affects servers. How many connections this listener may serve at
    /// once; the bound on its file descriptors.
    ///
    /// Default is no limit (`None`).
    pub max_connections: Option<usize>,

    /// Only affects servers. How many connections one peer may hold, counted
    /// by client certificate where there is one and by address prefix
    /// otherwise. This stops one peer taking the whole of `max_connections`;
    /// it is not itself a bound, since the number of peers is not.
    ///
    /// Default is no limit (`None`).
    pub max_connections_per_peer: Option<usize>,

    // Only affects servers
    pub load_shed: Option<bool>,

    /// Only affects clients
    pub rate_limit: Option<(u64, Duration)>,

    // Only affects servers
    pub global_concurrency_limit: Option<usize>,
}

impl Config {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn server_builder(&self) -> ServerBuilder {
        ServerBuilder::from_config(self, DefaultMetricsCallbackProvider::default())
    }

    pub fn server_builder_with_metrics<M>(&self, metrics_provider: M) -> ServerBuilder<M>
    where
        M: MetricsCallbackProvider,
    {
        ServerBuilder::from_config(self, metrics_provider)
    }

    pub async fn connect(&self, addr: &Multiaddr, tls_config: ClientConfig) -> Result<Channel> {
        connect_with_config(addr, tls_config, self).await
    }

    pub fn connect_lazy(&self, addr: &Multiaddr, tls_config: ClientConfig) -> Result<Channel> {
        connect_lazy_with_config(addr, tls_config, self)
    }

    pub(crate) fn http_config(&self) -> iota_http::Config {
        iota_http::Config::default()
            .initial_stream_window_size(self.http2_initial_stream_window_size)
            .initial_connection_window_size(self.http2_initial_connection_window_size)
            .max_concurrent_streams(self.http2_max_concurrent_streams)
            .http2_keepalive_timeout(self.http2_keepalive_timeout)
            .http2_keepalive_interval(self.http2_keepalive_interval)
            .tcp_keepalive(self.tcp_keepalive)
            .tcp_nodelay(self.tcp_nodelay.unwrap_or_default())
            .max_connection_idle(self.max_connection_idle)
            .max_connections(self.max_connections)
            .max_connections_per_peer(self.max_connections_per_peer)
            .max_connection_age(self.max_connection_age)
    }
}
