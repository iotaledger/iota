// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Shared gRPC server utilities

use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use tokio::sync::broadcast;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

use crate::{
    CheckpointGrpcService, EventGrpcService, GrpcCheckpointDataBroadcaster,
    GrpcCheckpointSummaryBroadcaster, GrpcReader,
    checkpoint::checkpoint_service_server::CheckpointServiceServer,
    events::event_service_server::EventServiceServer,
};

/// Handle to control a running gRPC server
pub struct GrpcServerHandle {
    /// Handle to the server task
    pub server_handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    /// Shutdown signal sender
    shutdown_token: CancellationToken,
    /// Broadcaster for checkpoint summaries
    pub checkpoint_summary_broadcaster: GrpcCheckpointSummaryBroadcaster,
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

    /// Get a reference to the checkpoint summary broadcaster
    pub fn checkpoint_summary_broadcaster(&self) -> &GrpcCheckpointSummaryBroadcaster {
        &self.checkpoint_summary_broadcaster
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
    event_subscriber: Arc<dyn crate::EventSubscriber>,
    config: iota_config::node::GrpcApiConfig,
    shutdown_token: CancellationToken,
) -> Result<GrpcServerHandle> {
    // Create broadcast channels
    let (checkpoint_summary_tx, _) = broadcast::channel(config.checkpoint_broadcast_buffer_size);
    let (checkpoint_data_tx, _) = broadcast::channel(config.checkpoint_broadcast_buffer_size);

    // Create broadcasters
    let checkpoint_summary_broadcaster =
        GrpcCheckpointSummaryBroadcaster::new(checkpoint_summary_tx);
    let checkpoint_data_broadcaster = GrpcCheckpointDataBroadcaster::new(checkpoint_data_tx);

    // Create the gRPC services - both get the cancellation token directly from
    // server level
    let checkpoint_service = CheckpointGrpcService::new(
        grpc_reader.clone(),
        checkpoint_summary_broadcaster.clone(),
        checkpoint_data_broadcaster.clone(),
        shutdown_token.clone(),
    );
    let event_service = EventGrpcService::new(event_subscriber, shutdown_token.clone());

    // Create the server with proper address binding
    let server_builder = Server::builder()
        .add_service(CheckpointServiceServer::new(checkpoint_service))
        .add_service(EventServiceServer::new(event_service));

    // Bind to the address to get the actual local address (especially important for
    // port 0)
    let listener = tokio::net::TcpListener::bind(config.address).await?;
    let actual_addr = listener.local_addr().unwrap_or(config.address);

    tracing::info!(
        "Starting gRPC server on {} (bound to {})",
        config.address,
        actual_addr
    );

    // Spawn the server task with graceful shutdown
    let shutdown_token_for_server = shutdown_token.clone();
    let server_handle = tokio::spawn(async move {
        let result = server_builder
            .serve_with_incoming_shutdown(
                TcpListenerStream::new(listener),
                shutdown_token_for_server.cancelled(),
            )
            .await;

        tracing::info!("gRPC server shutdown completed");
        result
    });

    Ok(GrpcServerHandle {
        server_handle,
        shutdown_token,
        checkpoint_summary_broadcaster,
        checkpoint_data_broadcaster,
        address: actual_addr,
    })
}
