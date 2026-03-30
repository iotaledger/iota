// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared gRPC server utilities

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use iota_grpc_types::v1::{
    ledger_service as grpc_ledger_service, move_package_service as grpc_move_package_service,
    state_service as grpc_state_service, transaction_execution_service as grpc_tx_service,
};
use iota_types::transaction_executor::TransactionExecutor;
use tokio::sync::broadcast;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::{Identity, Server, ServerTlsConfig};

use crate::{
    GrpcCheckpointDataBroadcaster, GrpcReader, GrpcServerMetrics, LedgerGrpcService,
    MovePackageGrpcService, StateGrpcService, TransactionExecutionGrpcService,
    metrics::GrpcMetricsLayer,
};

/// Handle to control a running gRPC server
pub struct GrpcServerHandle {
    /// Handle to the server task
    pub server_handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    /// Shutdown signal sender
    shutdown_token: CancellationToken,
    /// Broadcaster for checkpoint data
    pub checkpoint_data_broadcaster: GrpcCheckpointDataBroadcaster,
    /// Actual server address (with resolved port)
    pub address: SocketAddr,
}

impl GrpcServerHandle {
    /// Graceful shutdown of the gRPC server
    pub async fn shutdown(self) -> Result<()> {
        self.shutdown_token.cancel();
        match self.server_handle.await {
            Ok(result) => result.map_err(Into::into),
            Err(join_error) => Err(anyhow::anyhow!("Server task failed: {join_error}")),
        }
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

/// Adds gRPC services to a server builder and spawns the server.
///
/// This macro avoids duplicating the service registration and spawning logic
/// across the with-metrics and without-metrics code paths, since
/// `Server::layer()` changes the builder's type parameter.
macro_rules! build_and_spawn {
    ($server_builder:expr, $ledger_service:expr, $tx_service:expr, $state_service:expr,
     $move_package_service:expr, $config:expr,
     $listener:expr, $actual_addr:expr, $shutdown_token:expr) => {{
        let mut router = $server_builder.add_service(
            grpc_ledger_service::ledger_service_server::LedgerServiceServer::new($ledger_service)
                .max_encoding_message_size($config.max_message_size_bytes() as usize),
        );

        if let Some(tx_service) = $tx_service {
            router = router.add_service(
                grpc_tx_service::transaction_execution_service_server::TransactionExecutionServiceServer::new(tx_service)
                .max_encoding_message_size($config.max_message_size_bytes() as usize),
            );
        }

        router = router.add_service(
            grpc_state_service::state_service_server::StateServiceServer::new($state_service)
                .max_encoding_message_size($config.max_message_size_bytes() as usize),
        );

        router = router.add_service(
            grpc_move_package_service::move_package_service_server::MovePackageServiceServer::new($move_package_service)
                .max_encoding_message_size($config.max_message_size_bytes() as usize),
        );

        let shutdown_token_for_server = $shutdown_token.clone();
        if $config.tls_config().is_some() {
            // TLS case: tonic needs to control the entire transport stack for TLS,
            // so we let it handle binding. We drop our pre-bound listener since
            // tonic will create its own with proper TLS configuration.
            drop($listener);

            tokio::spawn(async move {
                let result = router
                    .serve_with_shutdown($actual_addr, shutdown_token_for_server.cancelled())
                    .await;
                tracing::info!("gRPC server shutdown completed");
                result
            })
        } else {
            // Non-TLS case: use the existing listener
            tokio::spawn(async move {
                let result = router
                    .serve_with_incoming_shutdown(
                        TcpListenerStream::new($listener),
                        shutdown_token_for_server.cancelled(),
                    )
                    .await;
                tracing::info!("gRPC server shutdown completed");
                result
            })
        }
    }};
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
    shutdown_token: CancellationToken,
    chain_id: iota_types::digests::ChainIdentifier,
    metrics: Option<GrpcServerMetrics>,
) -> Result<GrpcServerHandle> {
    // Create broadcast channels
    let (checkpoint_data_tx, _) = broadcast::channel(config.broadcast_buffer_size as usize);

    // Create broadcasters
    let checkpoint_data_broadcaster = GrpcCheckpointDataBroadcaster::new(checkpoint_data_tx);

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
    let move_package_service = MovePackageGrpcService::new(grpc_reader.clone());

    // Bind to the address to get the actual local address (especially important for
    // port 0)
    let listener = tokio::net::TcpListener::bind(config.address).await?;
    let actual_addr = listener.local_addr().unwrap_or(config.address);

    tracing::info!(
        "Starting gRPC server on {} (bound to {})",
        config.address,
        actual_addr
    );

    // Build the server builder with TLS and optional metrics layer.
    // Server::layer() changes the builder's generic type parameter, so we use
    // a macro to avoid duplicating the service registration and spawn logic.
    let mut server_builder = Server::builder();

    // Configure TLS if enabled
    if let Some(tls_config) = config.tls_config() {
        let cert = std::fs::read_to_string(tls_config.cert()).map_err(|e| {
            anyhow::anyhow!(
                "failed to read TLS cert file '{}': {}",
                tls_config.cert(),
                e
            )
        })?;
        let key = std::fs::read_to_string(tls_config.key()).map_err(|e| {
            anyhow::anyhow!("failed to read TLS key file '{}': {}", tls_config.key(), e)
        })?;

        let identity = Identity::from_pem(cert, key);
        let tls = ServerTlsConfig::new().identity(identity);

        tracing::info!("gRPC server TLS enabled");

        server_builder = server_builder
            .tls_config(tls)
            .map_err(|e| anyhow::anyhow!("failed to configure TLS: {}", e))?;
    }

    // Add services and spawn the server, optionally wrapping with metrics layer
    let server_handle = if let Some(metrics) = metrics {
        let mut layered_builder = server_builder.layer(GrpcMetricsLayer::new(Arc::new(metrics)));
        build_and_spawn!(
            layered_builder,
            ledger_service,
            tx_service,
            state_service,
            move_package_service,
            config,
            listener,
            actual_addr,
            shutdown_token
        )
    } else {
        build_and_spawn!(
            server_builder,
            ledger_service,
            tx_service,
            state_service,
            move_package_service,
            config,
            listener,
            actual_addr,
            shutdown_token
        )
    };

    Ok(GrpcServerHandle {
        server_handle,
        shutdown_token,
        checkpoint_data_broadcaster,
        address: actual_addr,
    })
}
