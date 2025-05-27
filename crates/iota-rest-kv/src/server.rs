// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes helper wrappers for building and starting a REST API
//! server.

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{Router, response::IntoResponse, routing::get};
use tokio_util::sync::CancellationToken;

use crate::{
    RestApiConfig,
    errors::ApiError,
    kv_store_client::KvStoreClient,
    routes::{health, kv_store},
};

/// A wrapper which builds the components needed for the REST API server and
/// provides a simple way to start it.
pub struct Server {
    router: Router,
    server_address: SocketAddr,
    token: CancellationToken,
}

impl Server {
    /// Create a new Server instance.
    ///
    /// Based on the config, it instantiates the [`KvStoreClient`] and
    /// constructs the [`Router`].
    pub async fn new(config: RestApiConfig, token: CancellationToken) -> Result<Self> {
        let kv_store_client = KvStoreClient::new(config.kv_store_config).await?;

        let shared_state = Arc::new(kv_store_client);

        let router = Router::new()
            .route("/health", get(health::health))
            .route("/{item_type}/{key}", get(kv_store::data_as_bytes))
            .with_state(shared_state)
            .fallback(fallback);

        Ok(Self {
            router,
            token,
            server_address: config.server_address,
        })
    }

    /// Start the server, this method is blocking.
    pub async fn serve(self) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(self.server_address)
            .await
            .expect("failed to bind to socket");

        tracing::info!("listening on: {}", self.server_address);

        axum::serve(listener, self.router)
            .with_graceful_shutdown(async move {
                self.token.cancelled().await;
                tracing::info!("shutdown signal received.");
            })
            .await
            .inspect_err(|e| tracing::error!("server encountered an error: {e}"))
            .map_err(Into::into)
    }
}

/// Handles requests to routes that are not defined in the API.
///
/// This fallback handler is called when the requested URL path does not match
/// any of the defined routes. It returns a `404 Not Found` error, indicating
/// that the requested resource could not be found. This can happen if the user
/// enters an incorrect URL or if the requested resource (identified by a
/// [`Key`](iota_storage::http_key_value_store::Key)) cannot be extracted from
/// the request.
async fn fallback() -> impl IntoResponse {
    ApiError::NotFound
}
