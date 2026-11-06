// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{num::NonZeroU32, time::Duration};

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

/// Settings for clients, and for the gRPC layers of a server. A server's
/// connection and HTTP/2 settings are an [`iota_http::Config`], passed to
/// [`ServerBuilder::http_config`].
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Config {
    /// Only affects clients. How many requests a connection may have in
    /// flight at once.
    pub concurrency_limit_per_connection: Option<usize>,

    /// Set a timeout for all request handlers.
    pub request_timeout: Option<Duration>,

    /// Only affects clients. Set a timeout for establishing an outbound
    /// connection.
    pub connect_timeout: Option<Duration>,

    /// Only affects clients. Sets the SETTINGS_INITIAL_WINDOW_SIZE option for
    /// HTTP2 stream-level flow control.
    ///
    /// Default is 65,535
    pub http2_initial_stream_window_size: Option<NonZeroU32>,

    /// Only affects clients. Sets the max connection-level flow control for
    /// HTTP2
    ///
    /// Default is 65,535
    pub http2_initial_connection_window_size: Option<NonZeroU32>,

    /// Only affects clients. Set whether TCP keepalive messages are enabled on
    /// outbound connections.
    ///
    /// If None is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP
    /// keepalive probes.
    ///
    /// Default is no keepalive (None)
    pub tcp_keepalive: Option<Duration>,

    /// Only affects clients. Set whether HTTP2 Ping frames are enabled on
    /// outbound connections.
    ///
    /// If None is specified, HTTP2 keepalive is disabled, otherwise the
    /// duration specified will be the time interval between HTTP2 Ping
    /// frames. The timeout for receiving an acknowledgement
    /// of the keepalive ping can be set with http2_keepalive_timeout.
    ///
    /// Default is no HTTP2 keepalive (None)
    pub http2_keepalive_interval: Option<Duration>,

    /// Only affects clients. Sets a timeout for receiving an acknowledgement
    /// of the keepalive ping.
    ///
    /// If the ping is not acknowledged within the timeout, the connection will
    /// be closed. Does nothing if http2_keep_alive_interval is disabled.
    ///
    /// Default is 20 seconds.
    pub http2_keepalive_timeout: Option<Duration>,

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
}
