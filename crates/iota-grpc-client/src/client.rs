// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use iota_grpc_types::v0::ledger_service::ledger_service_client::LedgerServiceClient;
use tap::Pipe;
use tonic::{codec::CompressionEncoding, transport::channel::ClientTlsConfig};

use crate::interceptors::HeadersInterceptor;

type Result<T, E = tonic::Status> = std::result::Result<T, E>;
type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

type InterceptedChannel =
    tonic::service::interceptor::InterceptedService<tonic::transport::Channel, HeadersInterceptor>;

/// gRPC client factory for IOTA gRPC operations.
#[derive(Clone)]
pub struct Client {
    /// Target URI of the gRPC server
    uri: http::Uri,
    /// Shared gRPC channel for all service clients
    channel: tonic::transport::Channel,
    /// Headers interceptor for adding custom headers to requests
    headers: HeadersInterceptor,
    /// Maximum decoding message size for responses
    max_decoding_message_size: Option<usize>,

    /// Cached ledger client (singleton)
    ledger_client: Arc<OnceLock<LedgerServiceClient<InterceptedChannel>>>,
}

impl Client {
    /// Connect to a gRPC server and create a new Client instance.
    #[allow(clippy::result_large_err)]
    pub async fn connect<T>(uri: T) -> Result<Self>
    where
        T: TryInto<http::Uri>,
        T::Error: Into<BoxError>,
    {
        let uri = uri
            .try_into()
            .map_err(Into::into)
            .map_err(tonic::Status::from_error)?;

        let mut endpoint = tonic::transport::Endpoint::from(uri.clone());
        if uri.scheme() == Some(&http::uri::Scheme::HTTPS) {
            endpoint = endpoint
                .tls_config(ClientTlsConfig::new().with_enabled_roots())
                .map_err(Into::into)
                .map_err(tonic::Status::from_error)?;
        }

        let channel = endpoint
            .connect_timeout(Duration::from_secs(5))
            .http2_keep_alive_interval(Duration::from_secs(5))
            .connect_lazy();

        Ok(Self {
            uri,
            channel,
            headers: Default::default(),
            max_decoding_message_size: None,
            ledger_client: Arc::new(OnceLock::new()),
        })
    }

    pub fn uri(&self) -> &http::Uri {
        &self.uri
    }

    /// Get a reference to the underlying channel.
    ///
    /// This can be useful for creating additional service clients that aren't
    /// yet integrated into Client.
    pub fn channel(&self) -> &tonic::transport::Channel {
        &self.channel
    }

    pub fn headers(&self) -> &HeadersInterceptor {
        &self.headers
    }

    pub fn max_decoding_message_size(&self) -> Option<usize> {
        self.max_decoding_message_size
    }

    pub fn with_headers(mut self, headers: HeadersInterceptor) -> Self {
        self.headers = headers;
        self
    }

    pub fn with_max_decoding_message_size(mut self, limit: usize) -> Self {
        self.max_decoding_message_size = Some(limit);
        self
    }

    // ========================================
    // Service Client Factories
    // ========================================

    /// Get a ledger service client.
    ///
    /// Returns `Some(LedgerClient)` if the server supports ledger-related
    /// operations, `None` otherwise. The client is created only once and
    /// cached for subsequent calls.
    pub fn ledger_service_client(&self) -> Option<LedgerServiceClient<InterceptedChannel>> {
        // For now, always return Some since ledger service is always available
        // In the future, this could check server capabilities first
        Some(
            self.ledger_client
                .get_or_init(|| {
                    LedgerServiceClient::with_interceptor(
                        self.channel.clone(),
                        self.headers.clone(),
                    )
                    .accept_compressed(CompressionEncoding::Zstd)
                    .pipe(|client| {
                        if let Some(limit) = self.max_decoding_message_size {
                            client.max_decoding_message_size(limit)
                        } else {
                            client
                        }
                    })
                })
                .clone(),
        )
    }
}
