// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared gRPC server utilities

use std::{
    net::SocketAddr,
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
    time::Duration,
};

use anyhow::Result;
use iota_config::node::ListenerLimitsConfig;
use iota_grpc_types::v1::{
    ledger_service as grpc_ledger_service, move_package_service as grpc_move_package_service,
    service_methods, state_service as grpc_state_service,
    transaction_execution_service as grpc_tx_service,
};
use iota_network_stack::{grpc_timeout::GrpcTimeout, server::insert_tonic_connect_info};
use iota_traffic_controller::TrafficController;
use iota_types::{traffic_control::ClientIdSource, transaction_executor::TransactionExecutor};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tonic::service::Routes;

use crate::{
    GrpcCheckpointDataBroadcaster, GrpcReader, GrpcServerMetrics, LedgerGrpcService,
    MovePackageGrpcService, StateGrpcService, TransactionExecutionGrpcService,
    constants::{MIN_IOTA_SDK_GRPC_CLIENT_VERSION, X_MIN_IOTA_SDK_GRPC_CLIENT_VERSION},
    metrics::GrpcMetricsLayer,
    server_timing::ServerTimingLayer,
    traffic_control::TrafficControlLayer,
};

/// Handle to control a running gRPC server
pub struct GrpcServerHandle {
    /// Handle to the task that shuts the server down
    server_handle: tokio::task::JoinHandle<()>,
    /// Shutdown signal sender
    shutdown_token: CancellationToken,
    /// Broadcaster for checkpoint data
    pub checkpoint_data_broadcaster: GrpcCheckpointDataBroadcaster,
    /// Actual server address (with resolved port)
    pub address: SocketAddr,
}

impl GrpcServerHandle {
    /// Graceful shutdown of the gRPC server. Calls still running 1 s after the
    /// shutdown starts are closed.
    pub async fn shutdown(self) -> Result<()> {
        self.shutdown_token.cancel();
        self.server_handle
            .await
            .map_err(|join_error| anyhow::anyhow!("server task failed: {join_error}"))
    }

    /// Get the server address (actual bound address)
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Get a reference to the checkpoint data broadcaster
    pub fn checkpoint_data_broadcaster(&self) -> &GrpcCheckpointDataBroadcaster {
        &self.checkpoint_data_broadcaster
    }
}

/// Start a gRPC server with checkpoint and event services
///
/// This function creates and starts a gRPC server that hosts checkpoint-related
/// and event streaming services. Currently includes the checkpoint streaming
/// and event streaming services, but can be extended to host additional
/// services in the future.
pub async fn start_grpc_server(
    grpc_reader: Arc<GrpcReader>,
    executor: Option<Arc<dyn TransactionExecutor>>,
    config: iota_config::node::GrpcApiConfig,
    listener_limits: &ListenerLimitsConfig,
    shutdown_token: CancellationToken,
    chain_id: iota_types::digests::ChainIdentifier,
    metrics: Option<GrpcServerMetrics>,
    traffic_controller: Option<Arc<TrafficController>>,
    client_id_source: Option<ClientIdSource>,
) -> Result<GrpcServerHandle> {
    // Create broadcast channels
    let (checkpoint_data_tx, _) = broadcast::channel(config.broadcast_buffer_size as usize);

    // Create broadcasters
    let inflight_subscribers = metrics
        .as_ref()
        .map(|m| m.inflight_checkpoint_stream_subscribers());
    let checkpoint_data_broadcaster = GrpcCheckpointDataBroadcaster::new(
        checkpoint_data_tx,
        config.max_concurrent_stream_subscribers.max(1) as usize,
        inflight_subscribers,
    );

    // Create the gRPC services - get the cancellation token directly from
    // server level
    let ledger_service = LedgerGrpcService::new(
        config.clone(),
        grpc_reader.clone(),
        checkpoint_data_broadcaster.clone(),
        shutdown_token.clone(),
        chain_id,
    );

    // Create TransactionExecutionService if executor is provided
    let tx_service = executor.map(|executor| {
        TransactionExecutionGrpcService::new(config.clone(), grpc_reader.clone(), executor)
    });

    // Create StateService and MovePackageService.
    // Unlike TransactionExecutionService (conditional on executor), these are
    // always registered: they are read-only and return a clear error
    // ("indexes are not available") at request time if indexes are absent,
    // which is more informative than an `Unimplemented` from an unregistered
    // service.
    let state_service = StateGrpcService::new(grpc_reader.clone());
    let move_package_service = MovePackageGrpcService::new(grpc_reader);

    // The listener is public, so its connection count is bounded only by this,
    // well inside the process fd limit it shares with RocksDB and the other
    // listeners.
    const MAX_CONNECTIONS: NonZeroUsize = NonZeroUsize::new(4096).unwrap();
    // A reverse proxy in front of the node shows as one address, so one
    // address may hold a quarter of the listener; it then takes four to fill
    // it.
    const MAX_CONNECTIONS_PER_CLIENT_IP: NonZeroUsize = NonZeroUsize::new(1024).unwrap();
    // Frees the connections of clients that stopped sending requests. A
    // checkpoint subscription counts as a request for as long as it streams.
    const MAX_CONNECTION_IDLE: Duration = Duration::from_secs(300);
    // The smallest value RFC 9113 recommends. Subscriptions are streams too,
    // and are also capped across all connections by
    // `max_concurrent_stream_subscribers`.
    const MAX_CONCURRENT_STREAMS: NonZeroU32 = NonZeroU32::new(100).unwrap();
    // Closes a connection whose peer has gone while a subscription is still
    // streaming to it, which the idle deadline does not.
    const HTTP2_KEEPALIVE: Duration = Duration::from_secs(60);

    // gRPC runs over HTTP/2 only, and no route of this API uses the extended
    // CONNECT protocol.
    let http_config = iota_http::Config::default()
        .accept_http1(false)
        .enable_connect_protocol(false)
        .handshake_timeout(
            listener_limits
                .handshake_timeout
                .resolve(iota_http::DEFAULT_HANDSHAKE_TIMEOUT),
        )
        .max_pending_connections(
            listener_limits
                .max_pending_connections
                .resolve(iota_http::DEFAULT_MAX_PENDING_CONNECTIONS),
        )
        .max_connections(listener_limits.max_connections.resolve(MAX_CONNECTIONS))
        .max_connections_per_client_ip(
            listener_limits
                .max_connections_per_client_ip
                .resolve(MAX_CONNECTIONS_PER_CLIENT_IP),
        )
        .ipv6_prefix_len(
            listener_limits
                .ipv6_prefix_len
                .unwrap_or(iota_http::DEFAULT_IPV6_PREFIX_LEN),
        )
        .max_connection_idle(
            listener_limits
                .max_connection_idle
                .resolve(MAX_CONNECTION_IDLE),
        )
        .max_concurrent_streams(
            listener_limits
                .max_concurrent_streams
                .resolve(MAX_CONCURRENT_STREAMS),
        )
        .http2_keepalive_interval(
            listener_limits
                .http2_keepalive_interval
                .resolve(HTTP2_KEEPALIVE),
        )
        .http1_header_read_timeout(
            listener_limits
                .http1_header_read_timeout
                .resolve(iota_http::DEFAULT_HTTP1_HEADER_READ_TIMEOUT),
        )
        .metrics(metrics.as_ref().map(GrpcServerMetrics::listener_metrics));
    let mut http_builder = iota_http::Builder::new().config(http_config);

    if let Some(tls_config) = config.tls_config() {
        http_builder = http_builder
            .tls_single_cert(tls_config.cert(), tls_config.key())
            .map_err(|e| {
                anyhow::anyhow!(
                    "failed to load the TLS cert file '{}' and key file '{}': {e}",
                    tls_config.cert(),
                    tls_config.key()
                )
            })?;
        tracing::info!("gRPC server TLS enabled");
    }

    let max_message_size = config.max_message_size_bytes() as usize;
    let mut routes = Routes::new(
        grpc_ledger_service::ledger_service_server::LedgerServiceServer::new(ledger_service)
            .max_encoding_message_size(max_message_size),
    )
    .add_service(
        grpc_state_service::state_service_server::StateServiceServer::new(state_service)
            .max_encoding_message_size(max_message_size),
    )
    .add_service(
        grpc_move_package_service::move_package_service_server::MovePackageServiceServer::new(
            move_package_service,
        )
        .max_encoding_message_size(max_message_size),
    );
    if let Some(tx_service) = tx_service {
        routes = routes.add_service(
            grpc_tx_service::transaction_execution_service_server::TransactionExecutionServiceServer::new(tx_service)
                .max_encoding_message_size(max_message_size),
        );
    }

    // Order matters:
    // - the address copy before traffic control, which reads it;
    // - the minimum client version outside every layer that can answer a request itself, so those
    //   responses carry it too;
    // - metrics before traffic control, to count blocked requests;
    // - `GrpcTimeout` inside metrics and traffic control, so they see the status of a call that
    //   ends at its deadline;
    // - server-timing innermost, to time only the requests that traffic control lets through.
    let service = tower::ServiceBuilder::new()
        .map_request(insert_tonic_connect_info)
        .map_response(insert_min_client_version)
        .option_layer(
            metrics.map(|m| GrpcMetricsLayer::new(Arc::new(m), &service_methods::ALL_METHOD_PATHS)),
        )
        .option_layer(
            traffic_controller
                .map(|tc| TrafficControlLayer::new(tc, client_id_source.unwrap_or_default())),
        )
        .layer_fn(|service| GrpcTimeout::new(service, None))
        .layer(ServerTimingLayer)
        .service(routes.prepare());

    let http_handle = http_builder.serve(config.address, service).map_err(|e| {
        anyhow::anyhow!("failed to start the gRPC server on {}: {e}", config.address)
    })?;
    let actual_addr = *http_handle.local_addr();

    tracing::info!(
        "Starting gRPC server on {} (bound to {})",
        config.address,
        actual_addr
    );

    let server_handle = tokio::spawn({
        let shutdown_token = shutdown_token.clone();
        async move {
            shutdown_token.cancelled().await;
            http_handle.shutdown().await;
            tracing::info!("gRPC server shutdown completed");
        }
    });

    Ok(GrpcServerHandle {
        server_handle,
        shutdown_token,
        checkpoint_data_broadcaster,
        address: actual_addr,
    })
}

/// Adds the lowest supported `iota-sdk-grpc-client` version to a response.
fn insert_min_client_version<B>(mut response: http::Response<B>) -> http::Response<B> {
    response.headers_mut().insert(
        X_MIN_IOTA_SDK_GRPC_CLIENT_VERSION,
        http::HeaderValue::from_static(MIN_IOTA_SDK_GRPC_CLIENT_VERSION),
    );
    response
}
