// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes helper wrappers for building and starting a REST API
//! server

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{Router, response::IntoResponse, routing::get};
use tokio_util::sync::CancellationToken;

use crate::{
    RestApiConfig,
    errors::ApiError,
    routes::{health, kv_store},
    services::KvStoreService,
};

/// A wrapper which builds the components needed for the REST API server and
/// provides a simple way to start it
pub struct Server {
    router: Router,
    rest_api_address: SocketAddr,
    token: CancellationToken,
}

impl Server {
    /// Create a new Server instance
    ///
    /// Based on the config, it instantiates the needed services and
    /// constructs the [Router]
    pub async fn new(config: RestApiConfig, token: CancellationToken) -> Result<Self> {
        let kv_store_service = KvStoreService::new(config.kv_store_config).await?;

        let shared_state = Arc::new(kv_store_service);

        let router = Router::new()
            .route("/health", get(health::health))
            .route("/:digest/:item_type", get(kv_store::data_as_bytes))
            .with_state(shared_state)
            .fallback(fallback);

        Ok(Self {
            router,
            token,
            rest_api_address: config.rest_api_address,
        })
    }

    /// Start the server, this method is blocking
    pub async fn serve(self) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(self.rest_api_address)
            .await
            .expect("failed to bind to socket");

        tracing::info!("listening on: {}", self.rest_api_address);

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

/// A fallback API response if the user does not match the available routes
/// supported by the REST API server
async fn fallback() -> impl IntoResponse {
    ApiError::Forbidden
}
