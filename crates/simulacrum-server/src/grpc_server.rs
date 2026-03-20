// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC streaming server for Simulacrum

use std::sync::Arc;

use anyhow::Result;
use iota_grpc_server::{GrpcReader, GrpcServerHandle, start_grpc_server};
use iota_types::{
    storage::RestStateReader, transaction_executor::TransactionExecutor as TransactionExecutorTrait,
};
use simulacrum::{
    Simulacrum, state_reader::SimulacrumGrpcReader, transaction_executor::TransactionExecutor,
};

/// Start a gRPC server for the given simulacrum instance
pub async fn start_simulacrum_grpc_server(
    simulacrum: Arc<Simulacrum>,
    config: iota_config::node::GrpcApiConfig,
    shutdown_token: tokio_util::sync::CancellationToken,
) -> Result<GrpcServerHandle> {
    let chain_id = simulacrum
        .get_chain_identifier()
        .expect("chain identifier should be set");

    // Create a transaction executor for simulacrum to enable transaction execution
    // and simulation via gRPC
    let executor =
        Some(Arc::new(TransactionExecutor::new(simulacrum.clone()))
            as Arc<dyn TransactionExecutorTrait>);

    let simulacrum_reader = Arc::new(SimulacrumGrpcReader::new(simulacrum.clone(), chain_id));
    let grpc_reader = Arc::new(GrpcReader::new(simulacrum_reader, None));

    start_grpc_server(
        grpc_reader,
        executor,
        config,
        shutdown_token,
        chain_id,
        None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iota_config::local_ip_utils;
    use simulacrum::Simulacrum;

    use super::*;

    #[tokio::test]
    async fn test_grpc_server_startup_with_mutex() {
        let sim = Simulacrum::new();

        // Create some checkpoints
        sim.advance_clock(Duration::from_secs(1));
        sim.create_checkpoint();

        let simulacrum = Arc::new(sim);

        // Start gRPC server with test configuration using test utilities
        let address = local_ip_utils::new_local_tcp_socket_for_testing();
        let config = iota_config::node::GrpcApiConfig {
            address,
            ..Default::default()
        };
        let shutdown_token = tokio_util::sync::CancellationToken::new();

        let server_handle =
            start_simulacrum_grpc_server(simulacrum, config, shutdown_token.clone())
                .await
                .unwrap();

        // Verify server handle was created with proper address resolution
        assert!(server_handle.address().ip().is_loopback());
        assert!(server_handle.address().port() > 0);

        // Shutdown
        shutdown_token.cancel();
        server_handle.shutdown().await.unwrap();
    }
}
