// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use iota_grpc_types::v1::{
    ledger_service::ledger_service_client::LedgerServiceClient,
    move_package_service::move_package_service_client::MovePackageServiceClient,
    state_service::state_service_client::StateServiceClient,
    transaction_execution_service::transaction_execution_service_client::TransactionExecutionServiceClient,
};
use tonic::codec::CompressionEncoding;

use crate::{api::Result, interceptors::HeadersInterceptor};

type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub type InterceptedChannel =
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
            #[cfg(not(feature = "tls-ring"))]
            return Err(tonic::Status::failed_precondition(
                "the `tls-ring` feature must be enabled for HTTPS",
            )
            .into());

            #[cfg(not(any(feature = "tls-native-roots", feature = "tls-webpki-roots")))]
            return Err(tonic::Status::failed_precondition(
                "the `tls-native-roots` or `tls-webpki-roots` feature must be enabled for HTTPS",
            )
            .into());

            #[cfg(all(
                feature = "tls-ring",
                any(feature = "tls-native-roots", feature = "tls-webpki-roots")
            ))]
            {
                endpoint = endpoint
                    .tls_config(
                        tonic::transport::channel::ClientTlsConfig::new().with_enabled_roots(),
                    )
                    .map_err(Into::into)
                    .map_err(tonic::Status::from_error)?;
            }
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

    /// Get a ledger service client.
    pub fn ledger_service_client(&self) -> LedgerServiceClient<InterceptedChannel> {
        self.configure_client(LedgerServiceClient::with_interceptor(
            self.channel.clone(),
            self.headers.clone(),
        ))
    }

    /// Get a transaction execution service client.
    pub fn execution_service_client(
        &self,
    ) -> TransactionExecutionServiceClient<InterceptedChannel> {
        self.configure_client(TransactionExecutionServiceClient::with_interceptor(
            self.channel.clone(),
            self.headers.clone(),
        ))
    }

    /// Get a state service client.
    pub fn state_service_client(&self) -> StateServiceClient<InterceptedChannel> {
        self.configure_client(StateServiceClient::with_interceptor(
            self.channel.clone(),
            self.headers.clone(),
        ))
    }

    /// Get a move package service client.
    pub fn move_package_service_client(&self) -> MovePackageServiceClient<InterceptedChannel> {
        self.configure_client(MovePackageServiceClient::with_interceptor(
            self.channel.clone(),
            self.headers.clone(),
        ))
    }

    /// Apply common client configuration (compression, message size limits).
    fn configure_client<C: GrpcClientConfig>(&self, client: C) -> C {
        let client = client.accept_compressed(CompressionEncoding::Zstd);
        if let Some(limit) = self.max_decoding_message_size {
            client.max_decoding_message_size(limit)
        } else {
            client
        }
    }
}

/// Trait for common gRPC client configuration methods.
///
/// This trait abstracts over the common configuration methods shared by
/// tonic-generated service clients, allowing `configure_client` to work
/// generically.
trait GrpcClientConfig: Sized {
    fn accept_compressed(self, encoding: CompressionEncoding) -> Self;
    fn max_decoding_message_size(self, limit: usize) -> Self;
}

/// Implement `GrpcClientConfig` for tonic-generated service clients.
macro_rules! impl_grpc_client_config {
    ($($client:ty),* $(,)?) => {
        $(
            impl GrpcClientConfig for $client {
                fn accept_compressed(self, encoding: CompressionEncoding) -> Self {
                    self.accept_compressed(encoding)
                }
                fn max_decoding_message_size(self, limit: usize) -> Self {
                    self.max_decoding_message_size(limit)
                }
            }
        )*
    };
}

impl_grpc_client_config!(
    LedgerServiceClient<InterceptedChannel>,
    TransactionExecutionServiceClient<InterceptedChannel>,
    StateServiceClient<InterceptedChannel>,
    MovePackageServiceClient<InterceptedChannel>,
);
