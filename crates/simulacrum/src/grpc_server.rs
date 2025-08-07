// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Built-in gRPC streaming server for Simulacrum

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Result;
use iota_config::local_ip_utils;
use iota_grpc_api::{
    CheckpointGrpcService, GrpcReader, GrpcStateReader,
    checkpoint::checkpoint_service_server::CheckpointServiceServer,
};
use iota_grpc_types::{
    CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary,
    CheckpointData as GrpcCheckpointData,
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};
use tokio::sync::broadcast;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use crate::{Simulacrum, store::SimulatorStore};

/// Configuration for the gRPC server
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
    /// Address to bind the gRPC server to
    pub address: SocketAddr,
    /// Buffer size for checkpoint summary broadcasts
    pub checkpoint_summary_buffer_size: usize,
    /// Buffer size for checkpoint data broadcasts
    pub checkpoint_data_buffer_size: usize,
    /// Whether to automatically broadcast new checkpoints
    pub auto_broadcast: bool,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            address: local_ip_utils::new_local_tcp_socket_for_testing(), /* Use proper test
                                                                          * utilities */
            checkpoint_summary_buffer_size: 1000,
            checkpoint_data_buffer_size: 1000,
            auto_broadcast: true,
        }
    }
}

impl GrpcServerConfig {
    /// Create a new configuration with a specific port on localhost
    pub fn with_port(port: u16) -> Self {
        Self {
            address: format!("{}:{}", local_ip_utils::localhost_for_testing(), port)
                .parse()
                .unwrap(),
            ..Self::default()
        }
    }

    /// Create a new configuration using an OS-assigned port
    /// This uses the proper test utilities to get an available port
    pub fn with_os_assigned_port() -> Self {
        Self {
            address: local_ip_utils::new_local_tcp_socket_for_testing(),
            ..Self::default()
        }
    }

    /// Create a new configuration for testing with guaranteed unique address
    /// This is the recommended method for tests to avoid port conflicts
    pub fn for_testing() -> Self {
        Self::default() // Default already uses test utilities
    }
}

/// Handle to control a running gRPC server
pub struct GrpcServerHandle {
    /// Handle to the server task
    pub server_handle: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
    /// Sender for checkpoint summaries
    pub checkpoint_summary_tx: broadcast::Sender<Arc<GrpcCertifiedCheckpointSummary>>,
    /// Sender for checkpoint data
    pub checkpoint_data_tx: broadcast::Sender<Arc<GrpcCheckpointData>>,
    /// Actual server address (with resolved port)
    pub address: SocketAddr,
    /// Local address bound by the server (includes actual port when using 0)
    pub local_addr: Option<SocketAddr>,
}

impl GrpcServerHandle {
    /// Broadcast a new checkpoint summary
    pub fn broadcast_checkpoint_summary(&self, summary: CertifiedCheckpointSummary) -> Result<()> {
        let grpc_summary = Arc::new(GrpcCertifiedCheckpointSummary::from(summary));
        self.checkpoint_summary_tx.send(grpc_summary)?;
        Ok(())
    }

    /// Broadcast a new checkpoint data
    pub fn broadcast_checkpoint_data(&self, data: CheckpointData) -> Result<()> {
        let grpc_data = Arc::new(GrpcCheckpointData::from(data));
        self.checkpoint_data_tx.send(grpc_data)?;
        Ok(())
    }

    /// Shutdown the gRPC server
    pub fn shutdown(self) {
        self.server_handle.abort();
    }

    /// Get the server address (actual bound address)
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Get the local address bound by the server (useful when using port 0)
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }
}

/// GrpcStateReader implementation for Simulacrum
pub struct SimulacrumGrpcReader<Store: SimulatorStore> {
    simulacrum: Arc<Simulacrum<rand::rngs::OsRng, Store>>,
}

impl<Store: SimulatorStore> SimulacrumGrpcReader<Store> {
    pub fn new(simulacrum: Arc<Simulacrum<rand::rngs::OsRng, Store>>) -> Self {
        Self { simulacrum }
    }
}

impl<Store: SimulatorStore + Send + Sync + 'static> GrpcStateReader
    for SimulacrumGrpcReader<Store>
{
    fn get_latest_checkpoint_sequence(&self) -> Option<u64> {
        self.simulacrum
            .store()
            .get_highest_checkpoint()
            .map(|checkpoint| *checkpoint.sequence_number())
    }

    fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary> {
        self.simulacrum
            .store()
            .get_checkpoint_by_sequence_number(seq)
            .map(CertifiedCheckpointSummary::from)
    }

    fn get_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let checkpoint = self
            .simulacrum
            .store()
            .get_checkpoint_by_sequence_number(seq)?;
        let contents = self
            .simulacrum
            .store()
            .get_checkpoint_contents(&checkpoint.content_digest)?;

        Some(CheckpointData {
            checkpoint_summary: CertifiedCheckpointSummary::from(checkpoint),
            checkpoint_contents: contents,
            transactions: vec![],
        })
    }

    fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        // Simple implementation for simulacrum - find the last checkpoint of the given
        // epoch
        let latest_seq = self.get_latest_checkpoint_sequence().unwrap_or(0);

        for seq in (0..=latest_seq).rev() {
            if let Some(summary) = self.get_checkpoint_summary(seq) {
                if summary.epoch() == epoch {
                    return Ok(Some(summary));
                }
            }
        }
        Ok(None)
    }
}

impl<Store: SimulatorStore + Send + Sync + 'static> Simulacrum<rand::rngs::OsRng, Store> {
    /// Start a gRPC server for this simulacrum instance
    ///
    /// This provides a convenient way to expose the simulacrum's blockchain
    /// data via gRPC streaming APIs, useful for testing and development.
    ///
    /// # Example
    /// ```ignore
    /// use simulacrum::{Simulacrum, GrpcServerConfig};
    ///
    /// let mut simulacrum = Simulacrum::new();
    ///
    /// // Create some checkpoints
    /// simulacrum.advance_clock(std::time::Duration::from_secs(1));
    /// simulacrum.create_checkpoint();
    ///
    /// // Start gRPC server
    /// let config = GrpcServerConfig {
    ///     address: "127.0.0.1:9001".parse().unwrap(),
    ///     ..Default::default()
    /// };
    ///
    /// let server_handle = simulacrum.start_grpc_server(config).await?;
    /// println!("gRPC server running on {}", server_handle.address());
    ///
    /// // Server runs in background, can be shut down with:
    /// // server_handle.shutdown();
    /// ```
    pub async fn start_grpc_server(
        self: Arc<Self>,
        config: GrpcServerConfig,
    ) -> Result<GrpcServerHandle> {
        // Create broadcast channels
        let (checkpoint_summary_tx, _) = broadcast::channel(config.checkpoint_summary_buffer_size);
        let (checkpoint_data_tx, _) = broadcast::channel(config.checkpoint_data_buffer_size);

        // Create the gRPC reader
        let simulacrum_reader = SimulacrumGrpcReader::new(self.clone());
        let grpc_reader = Arc::new(GrpcReader::new(Arc::new(simulacrum_reader)));

        // Create the gRPC service
        let service = CheckpointGrpcService::new(
            grpc_reader,
            checkpoint_summary_tx.clone(),
            checkpoint_data_tx.clone(),
        );

        // Create the server with proper address binding
        let server_builder = Server::builder().add_service(CheckpointServiceServer::new(service));

        // Bind to the address to get the actual local address (especially important for
        // port 0)
        let listener = tokio::net::TcpListener::bind(config.address).await?;
        let local_addr = listener.local_addr().ok();
        let actual_addr = local_addr.unwrap_or(config.address);

        tracing::info!(
            "Starting Simulacrum gRPC server on {} (bound to {})",
            config.address,
            actual_addr
        );

        // Spawn the server task
        let server_handle = tokio::spawn(async move {
            server_builder
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
        });

        // If auto_broadcast is enabled, start broadcasting task
        if config.auto_broadcast {
            let broadcast_simulacrum = self.clone();
            let broadcast_summary_tx = checkpoint_summary_tx.clone();
            let broadcast_data_tx = checkpoint_data_tx.clone();

            tokio::spawn(async move {
                let mut last_sequence = 0u64;
                let mut interval = tokio::time::interval(Duration::from_millis(100));

                loop {
                    interval.tick().await;

                    // Check for new checkpoints
                    if let Some(latest_seq) = broadcast_simulacrum
                        .store()
                        .get_highest_checkpoint()
                        .map(|cp| *cp.sequence_number())
                    {
                        // Broadcast any new checkpoints since last check
                        for seq in (last_sequence + 1)..=latest_seq {
                            if let Some(summary) = broadcast_simulacrum
                                .store()
                                .get_checkpoint_by_sequence_number(seq)
                            {
                                let grpc_summary = Arc::new(GrpcCertifiedCheckpointSummary::from(
                                    CertifiedCheckpointSummary::from(summary.clone()),
                                ));
                                let _ = broadcast_summary_tx.send(grpc_summary);

                                // Also broadcast data if available
                                if let Some(contents) = broadcast_simulacrum
                                    .store()
                                    .get_checkpoint_contents(&summary.content_digest)
                                {
                                    let checkpoint_data = CheckpointData {
                                        checkpoint_summary: CertifiedCheckpointSummary::from(
                                            summary,
                                        ),
                                        checkpoint_contents: contents,
                                        transactions: vec![],
                                    };
                                    let grpc_data =
                                        Arc::new(GrpcCheckpointData::from(checkpoint_data));
                                    let _ = broadcast_data_tx.send(grpc_data);
                                }
                            }
                        }
                        last_sequence = latest_seq;
                    }
                }
            });
        }

        Ok(GrpcServerHandle {
            server_handle,
            checkpoint_summary_tx,
            checkpoint_data_tx,
            address: actual_addr,
            local_addr,
        })
    }

    /// Start a gRPC server with default configuration
    ///
    /// This is a convenience method that starts a gRPC server using test
    /// utilities to get an available port, with default buffer sizes and
    /// auto-broadcasting enabled.
    pub async fn start_grpc_server_default(self: Arc<Self>) -> Result<GrpcServerHandle> {
        self.start_grpc_server(GrpcServerConfig::default()).await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iota_types::base_types::IotaAddress;
    use tokio::time::timeout;

    use super::*;
    use crate::Simulacrum;

    #[tokio::test]
    async fn test_grpc_server_startup() {
        let mut simulacrum = Simulacrum::new();

        // Create some checkpoints
        simulacrum.advance_clock(Duration::from_secs(1));
        simulacrum.create_checkpoint();

        let simulacrum = Arc::new(simulacrum);

        // Start gRPC server with test configuration
        let config = GrpcServerConfig {
            auto_broadcast: false, // Disable for test
            ..GrpcServerConfig::for_testing()
        };

        let server_handle = simulacrum.start_grpc_server(config).await.unwrap();

        // Verify server handle was created with proper address resolution
        assert!(server_handle.address().ip().is_loopback());
        assert!(server_handle.address().port() > 0);

        // local_addr should also be available
        if let Some(local_addr) = server_handle.local_addr() {
            assert!(local_addr.port() > 0);
            assert!(local_addr.ip().is_loopback());
        }

        // Shutdown
        server_handle.shutdown();
    }

    #[tokio::test]
    async fn test_simulacrum_grpc_reader() {
        let mut simulacrum = Simulacrum::new();

        // Create some activity
        let recipient = IotaAddress::random_for_testing_only();
        let (tx, _) = simulacrum.transfer_txn(recipient);
        simulacrum.execute_transaction(tx).unwrap();
        simulacrum.create_checkpoint();

        let simulacrum = Arc::new(simulacrum);
        let reader = SimulacrumGrpcReader::new(simulacrum);

        // Test basic functionality
        assert!(reader.get_latest_checkpoint_sequence().is_some());
        assert!(reader.get_checkpoint_summary(0).is_some());
        assert!(reader.get_checkpoint_data(0).is_some());
        assert!(reader.get_epoch_last_checkpoint(0).unwrap().is_some());
    }

    #[tokio::test]
    async fn test_auto_broadcast() {
        let mut simulacrum = Simulacrum::new();
        simulacrum.advance_clock(Duration::from_secs(1));
        simulacrum.create_checkpoint();

        let simulacrum = Arc::new(simulacrum);

        // Start gRPC server with auto-broadcast using test configuration
        let config = GrpcServerConfig {
            auto_broadcast: true,
            ..GrpcServerConfig::for_testing()
        };

        let server_handle = simulacrum.start_grpc_server(config).await.unwrap();
        let mut summary_rx = server_handle.checkpoint_summary_tx.subscribe();

        // Give auto-broadcast a moment to start
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should receive existing checkpoint
        let result = timeout(Duration::from_secs(1), summary_rx.recv()).await;
        assert!(result.is_ok());

        server_handle.shutdown();
    }

    #[tokio::test]
    async fn test_available_port_assignment() {
        let mut simulacrum = Simulacrum::new();
        simulacrum.advance_clock(Duration::from_secs(1));
        simulacrum.create_checkpoint();

        let simulacrum = Arc::new(simulacrum);

        // Use default config which should use available port utilities
        let config = GrpcServerConfig::default();
        // The port should already be assigned by the test utilities
        assert!(config.address.port() > 0);
        assert!(config.address.ip().is_loopback());

        let server_handle = simulacrum.start_grpc_server(config).await.unwrap();

        // Server should be running on the pre-assigned available port
        assert!(server_handle.address().port() > 0);
        assert!(server_handle.address().ip().is_loopback());

        // local_addr should also be available
        if let Some(local_addr) = server_handle.local_addr() {
            assert!(local_addr.port() > 0);
            assert!(local_addr.ip().is_loopback());
        }

        println!(
            "Test utilities assigned port: {}",
            server_handle.address().port()
        );

        server_handle.shutdown();
    }

    #[tokio::test]
    async fn test_config_convenience_methods() {
        // Test specific port configuration
        let config_9001 = GrpcServerConfig::with_port(9001);
        assert_eq!(config_9001.address.port(), 9001);
        assert!(config_9001.address.ip().is_loopback());

        // Test OS-assigned port configuration using test utilities
        let config_os = GrpcServerConfig::with_os_assigned_port();
        assert!(config_os.address.port() > 0); // Should be pre-assigned by test utilities
        assert!(config_os.address.ip().is_loopback());

        // Test for_testing method
        let config_testing = GrpcServerConfig::for_testing();
        assert!(config_testing.address.port() > 0);
        assert!(config_testing.address.ip().is_loopback());

        // Default should use test utilities
        let config_default = GrpcServerConfig::default();
        assert!(config_default.address.port() > 0);
        assert!(config_default.address.ip().is_loopback());

        // All test configs should have different ports (due to proper utilities)
        let ports = vec![
            config_os.address.port(),
            config_testing.address.port(),
            config_default.address.port(),
        ];
        // Since test utilities give us available ports, they should all be different
        // (though this isn't guaranteed, it's highly likely)
        println!("Generated ports: {:?}", ports);
    }

    #[tokio::test]
    async fn test_multiple_servers_no_conflicts() {
        // Test that multiple simulacrum gRPC servers can run simultaneously
        // without port conflicts thanks to proper test utilities
        let mut handles = Vec::new();

        for i in 0..3 {
            let mut simulacrum = Simulacrum::new();
            simulacrum.advance_clock(Duration::from_secs(1));
            simulacrum.create_checkpoint();
            let simulacrum = Arc::new(simulacrum);

            let config = GrpcServerConfig::for_testing();
            let server_handle = simulacrum.start_grpc_server(config).await.unwrap();

            println!(
                "Server {} running on port {}",
                i,
                server_handle.address().port()
            );
            handles.push(server_handle);
        }

        // All servers should be running on different ports
        let ports: Vec<u16> = handles.iter().map(|h| h.address().port()).collect();
        let unique_ports: std::collections::HashSet<u16> = ports.iter().cloned().collect();

        // All ports should be unique
        assert_eq!(
            ports.len(),
            unique_ports.len(),
            "All servers should have unique ports"
        );

        // All ports should be in valid range
        for port in &ports {
            assert!(*port > 1024, "Port should be > 1024");
            // u16 max is 65535, so no need to check upper bound
        }

        println!("All servers running on unique ports: {:?}", ports);

        // Shutdown all servers
        for handle in handles {
            handle.shutdown();
        }
    }
}
