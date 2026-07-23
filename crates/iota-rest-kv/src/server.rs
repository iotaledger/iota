// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module includes helper wrappers for building and starting a REST API
//! server.
use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Result;
use axum::{
    Router,
    extract::{MatchedPath, Request},
    http::{StatusCode, header::HeaderName},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use iota_storage::http_key_value_store::ItemType;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tower_http::{
    classify::ServerErrorsFailureClass,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{Level, Span, field};

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

use crate::{
    RestApiConfig,
    bigtable::KvStoreClient,
    errors::ApiError,
    routes::{health, kv_store},
    types::RestServerAppState,
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

        let shared_state = Arc::new(RestServerAppState {
            kv_store_client: Arc::new(kv_store_client),
            multiget_max_items: config.multiget_max_items,
        });

        let router = Router::new()
            .route("/health", get(health::health))
            .route("/{item_type}", post(kv_store::multi_get_data))
            .route("/{item_type}/{key}", get(kv_store::data_as_bytes))
            // static and dynamic route segments are allowed to overlap. If they do, static segments
            // will be given higher priority.
            .route(
                &format!("/{}/{{address}}", ItemType::TransactionDigestsByAddress),
                get(kv_store::transaction_digests_by_address),
            )
            // register the fallback before the layers so that requests to
            // unmatched routes are traced as well
            .fallback(fallback)
            .layer(
                ServiceBuilder::new()
                    .layer(SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid))
                    .layer(
                        TraceLayer::new_for_http()
                            .make_span_with(make_request_span)
                            .on_response(log_response)
                            .on_failure(
                                |class: ServerErrorsFailureClass, latency: Duration, _: &Span| {
                                    tracing::error!(
                                        %class,
                                        latency_ms = latency.as_millis() as u64,
                                        "request failed"
                                    );
                                },
                            ),
                    )
                    .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER)),
            )
            .with_state(shared_state);

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

/// Creates a tracing span that wraps a single request.
///
/// - If `DEBUG` logging is enabled, a detailed span is created containing:
///   - `request_id`: Extracted from the request header (or empty if missing).
///   - `method`: The HTTP method of the request.
///   - `route`: The matched route template (falling back to the raw URI path).
///   - `uri`: The concrete request path data.
///   - `error`: Starts empty and is recorded by [`ApiError::into_response`]
///     when the request fails.
/// - Otherwise, a lighter span is created containing only `uri` and `error`.
fn make_request_span(request: &Request) -> Span {
    if tracing::span_enabled!(Level::DEBUG) {
        let request_id = request
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map_or_else(|| request.uri().path(), MatchedPath::as_str);

        tracing::debug_span!(
            "request",
            request_id,
            method = %request.method(),
            route,
            uri = %request.uri(),
            error = field::Empty,
        )
    } else {
        tracing::info_span!(
            "request",
            uri = %request.uri(),
            error = field::Empty,
        )
    }
}

/// Logs the completion of a request, choosing the level by status code range.
///
/// # Note
/// Server errors are skipped here, they are logged by
/// [`TraceLayer::on_failure`] together with the failure class.
fn log_response(response: &Response, latency: Duration, _: &Span) {
    let status = response.status();
    let latency_ms = latency.as_millis() as u64;

    if status.is_server_error() {
        return;
    }

    if status.is_client_error() && status != StatusCode::NOT_FOUND {
        tracing::warn!(%status, latency_ms, "request failed with client error");
    } else {
        tracing::info!(%status, latency_ms, "request completed");
    }
}
