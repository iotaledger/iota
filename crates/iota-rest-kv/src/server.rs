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
    types::{RestServerAppState, SharedRestServerAppState},
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

        Ok(Self {
            router: build_router(shared_state),
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

/// Builds the [`Router`] with all routes exposed by the REST API server.
fn build_router(state: SharedRestServerAppState) -> Router {
    Router::new()
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
        .with_state(state)
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

/// Tests for the request-validation logic of the REST API handlers.
///
/// Every request here is rejected (or answered) before any BigTable call is
/// made, so the tests run against a client pointed at a closed port: the
/// BigTable channel connects lazily and is never used.
#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use iota_sdk_types::{Address, TransactionDigest};
    use iota_storage::http_key_value_store::{
        TaggedKey, encode_digest, encode_object_key, encoded_tagged_key,
    };
    use iota_types::storage::ObjectKey;
    use tower::ServiceExt;

    use super::*;
    use crate::{errors::ErrorResponse, routes::health::HealthResponse};

    const MULTIGET_MAX_ITEMS: usize = 5;

    /// Builds the server router for testing.
    fn test_router() -> Router {
        build_router(Arc::new(RestServerAppState {
            kv_store_client: Arc::new(KvStoreClient::new_for_tests()),
            multiget_max_items: NonZeroUsize::new(MULTIGET_MAX_ITEMS).unwrap(),
        }))
    }

    /// Sends a request to the router and returns the status code and body.
    async fn send(router: Router, request: Request<Body>) -> (StatusCode, String) {
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    /// Sends a GET request to the router and returns the status code and body.
    async fn get(uri: &str) -> (StatusCode, String) {
        send(
            test_router(),
            Request::builder().uri(uri).body(Body::empty()).unwrap(),
        )
        .await
    }

    /// Sends a POST request with JSON body to the router and returns the status
    /// code and body.
    async fn post_json(uri: &str, body: serde_json::Value) -> (StatusCode, String) {
        send(
            test_router(),
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
    }

    /// Represents expected cases for each item type.
    fn item_type_cases() -> [(ItemType, String, &'static str); 7] {
        let digest_key = encode_digest(&TransactionDigest::random());
        let tagged_key = encoded_tagged_key(&TaggedKey::CheckpointSequenceNumber(1));
        let object_key = encode_object_key(&ObjectKey::ZERO);

        [
            (
                ItemType::Transaction,
                digest_key.clone(),
                "invalid digest byte length",
            ),
            (
                ItemType::TransactionEffects,
                digest_key.clone(),
                "invalid digest byte length",
            ),
            (
                ItemType::TransactionToCheckpoint,
                digest_key.clone(),
                "invalid digest byte length",
            ),
            (
                ItemType::EventTransactionDigest,
                digest_key.clone(),
                "invalid digest byte length",
            ),
            (
                ItemType::Object,
                object_key,
                "failed to deserialize object key",
            ),
            (
                ItemType::CheckpointContents,
                tagged_key,
                "failed to deserialize checkpoint sequence number",
            ),
            (
                ItemType::CheckpointSummary,
                digest_key,
                "failed to deserialize checkpoint sequence number",
            ),
        ]
    }

    /// Asserts the rejection cases shared by every `/{item_type}/{key}`
    /// route.
    ///
    /// - Keys that are not valid base64url.
    /// - Keys whose decoded bytes cannot be parsed as the key the item type
    ///   expects.
    /// - `before_version` misuse.
    async fn assert_item_type_route_rejections(
        item_type: ItemType,
        valid_key: &str,
        key_decode_error: &str,
    ) {
        // invalid base64url.
        let (status, body) = get(&format!("/{item_type}/!!")).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{item_type}: malformed base64 key"
        );
        assert!(
            body.contains("invalid base64 url string"),
            "{item_type}: unexpected body: {body}"
        );

        // valid base64url, but "AAAA" decodes to 3 bytes, which cannot be
        // parsed as the key the item type expects (a 32-byte digest, a BCS
        // `ObjectKey`, or a BCS `TaggedKey`).
        let (status, body) = get(&format!("/{item_type}/AAAA")).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{item_type}: key decode failure"
        );
        assert!(
            body.contains(key_decode_error),
            "{item_type}: unexpected body: {body}"
        );

        // `before_version` is only supported by the object item type.
        if item_type != ItemType::Object {
            let (status, body) =
                get(&format!("/{item_type}/{valid_key}?before_version=true")).await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{item_type}: before_version"
            );
            assert!(
                body.contains("`before_version` query parameter is only valid for `ob` item types"),
                "{item_type}: unexpected body: {body}"
            );
        }

        // non-boolean query parameter value.
        let (status, _) = get(&format!("/{item_type}/{valid_key}?before_version=notabool")).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{item_type}: invalid query parameter value"
        );
    }

    /// Asserts the rejection cases shared by every `POST /{item_type}`
    /// multiget route.
    ///
    /// - An empty key list.
    /// - More keys than the configured maximum.
    /// - Keys that are not valid base64url.
    /// - Keys whose decoded bytes cannot be parsed as the key the item type
    ///   expects.
    /// - `before_version` misuse.
    async fn assert_multiget_route_rejections(
        item_type: ItemType,
        valid_key: &str,
        key_decode_error: &str,
    ) {
        // empty key list.
        let (status, body) =
            post_json(&format!("/{item_type}"), serde_json::json!({ "keys": [] })).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{item_type}: empty keys");
        assert!(
            body.contains("no keys provided"),
            "{item_type}: unexpected body: {body}"
        );

        // more keys than the configured maximum.
        let keys = vec![valid_key; MULTIGET_MAX_ITEMS + 1];
        let (status, body) = post_json(
            &format!("/{item_type}"),
            serde_json::json!({ "keys": keys }),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{item_type}: too many keys"
        );
        assert!(
            body.contains("too many keys"),
            "{item_type}: unexpected body: {body}"
        );

        // a key that is not valid base64url among well-formed ones.
        let (status, body) = post_json(
            &format!("/{item_type}"),
            serde_json::json!({ "keys": [valid_key, "!!"] }),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{item_type}: malformed base64 key"
        );
        assert!(
            body.contains("invalid key '!!'"),
            "{item_type}: unexpected body: {body}"
        );

        // valid base64url, but "AAAA" decodes to 3 bytes, which cannot be
        // parsed as the key the item type expects.
        let (status, body) = post_json(
            &format!("/{item_type}"),
            serde_json::json!({ "keys": ["AAAA"] }),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{item_type}: key decode failure"
        );
        assert!(
            body.contains("invalid key 'AAAA'") && body.contains(key_decode_error),
            "{item_type}: unexpected body: {body}"
        );

        // `before_version` is only supported by the object item type.
        if item_type != ItemType::Object {
            let (status, body) = post_json(
                &format!("/{item_type}?before_version=true"),
                serde_json::json!({ "keys": [valid_key] }),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{item_type}: before_version"
            );
            assert!(
                body.contains("`before_version` query parameter is only valid for `ob` item types"),
                "{item_type}: unexpected body: {body}"
            );
        }
    }

    #[tokio::test]
    async fn health_endpoint_reports_ok() {
        let (status, body) = get("/health").await;
        assert_eq!(status, StatusCode::OK);
        let health: HealthResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(health.status, "OK");
    }

    #[tokio::test]
    async fn unknown_route_returns_not_found() {
        let (status, _) = get("/unknown/route/segments").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_item_type_is_rejected() {
        let (status, body) = get(&format!(
            "/zz/{}",
            encode_digest(&TransactionDigest::random())
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body.contains("invalid path parameter"),
            "unexpected body: {body}"
        );
    }

    #[tokio::test]
    async fn item_type_routes_reject_invalid_requests() {
        for (item_type, valid_key, key_decode_error) in item_type_cases() {
            assert_item_type_route_rejections(item_type, &valid_key, key_decode_error).await;
        }
    }

    #[tokio::test]
    async fn before_version_for_min_version_returns_not_found() {
        // The scan range below the minimum version is empty, so the handler
        // answers 404 without querying the store.
        let uri = format!(
            "/ob/{}?before_version=true",
            encode_object_key(&ObjectKey::ZERO)
        );
        let (status, body) = get(&uri).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.is_empty(), "expected empty body, got: {body}");
    }

    #[tokio::test]
    async fn multiget_routes_reject_invalid_requests() {
        for (item_type, valid_key, key_decode_error) in item_type_cases() {
            assert_multiget_route_rejections(item_type, &valid_key, key_decode_error).await;
        }
    }

    #[tokio::test]
    async fn transactions_by_address_rejects_invalid_requests() {
        let address_key = encode_digest(&Address::random());

        // address is not valid base64url.
        let (status, body) = get("/txa/!!").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body.contains("address is not valid base64-url"),
            "unexpected body: {body}"
        );

        // "AAAA" decodes to 3 bytes, not a 32-byte address.
        let (status, body) = get("/txa/AAAA").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("invalid address"), "unexpected body: {body}");

        // limit above the configured maximum.
        let uri = format!("/txa/{address_key}?limit={}", MULTIGET_MAX_ITEMS + 1);
        let (status, body) = get(&uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("limit too large"), "unexpected body: {body}");

        // query parameter values that fail to deserialize.
        for query in ["limit=0", "cursor=notanumber", "oldest_first=notabool"] {
            let uri = format!("/txa/{address_key}?{query}");
            let (status, _) = get(&uri).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "GET {uri}");
        }
    }

    #[tokio::test]
    async fn multiget_rejects_transaction_digests_by_address_item_type() {
        let key = encode_digest(&Address::random());
        let (status, body) = post_json("/txa", serde_json::json!({ "keys": [key] })).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        let res = serde_json::from_str::<ErrorResponse>(&body).unwrap();
        assert_eq!(res.error_code, "400");
        assert!(
            res.error_message.contains("unsupported key"),
            "unexpected body: {body}"
        );
    }

    #[tokio::test]
    async fn store_error_maps_to_internal_server_error() {
        // A well-formed request that reaches the store fails against the test
        // client's unreachable BigTableDB endpoint.
        let uri = format!("/tx/{}", encode_digest(&TransactionDigest::random()));
        let (status, body) = get(&uri).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        let res = serde_json::from_str::<ErrorResponse>(&body).unwrap();
        assert_eq!(res.error_code, "500");
        assert_eq!(res.error_message, "internal server error");
    }

    #[tokio::test]
    async fn wrong_method_returns_method_not_allowed() {
        // `/{item_type}` only accepts POST.
        let (status, _) = get("/tx").await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }
}
