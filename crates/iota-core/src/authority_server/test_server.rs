// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{io, sync::Arc};

use anyhow::Result;
use fastcrypto::traits::KeyPair;
use iota_config::local_ip_utils::new_local_tcp_address_for_testing;
use iota_network::api::ValidatorServer;
use iota_network_stack::server::IOTA_TLS_SERVER_NAME;
use iota_types::multiaddr::Multiaddr;
use tracing::info;

use crate::{
    authority::AuthorityState,
    authority_server::{ValidatorService, ValidatorServiceMetrics},
    checkpoints::CheckpointStore,
    consensus_adapter::{
        ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics,
    },
    starfish_adapter::LazyStarfishClient,
};

/// A handle to the authority server.
pub struct AuthorityServerHandle {
    server_handle: iota_network_stack::server::Server,
}

impl AuthorityServerHandle {
    /// Waits for the server to complete.
    pub async fn join(self) -> Result<(), io::Error> {
        self.server_handle.handle().wait_for_shutdown().await;
        Ok(())
    }

    /// Kills the server.
    pub async fn kill(self) -> Result<(), io::Error> {
        self.server_handle.handle().shutdown().await;
        Ok(())
    }

    /// Returns the address of the server.
    pub fn address(&self) -> &Multiaddr {
        self.server_handle.local_addr()
    }
}

/// An authority server that is used for testing.
pub struct AuthorityServer {
    address: Multiaddr,
    pub state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    pub metrics: Arc<ValidatorServiceMetrics>,
}

impl AuthorityServer {
    /// Creates a new `AuthorityServer` for testing with a consensus adapter.
    pub fn new_for_test_with_consensus_adapter(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
    ) -> Self {
        let address = new_local_tcp_address_for_testing();
        let metrics = Arc::new(ValidatorServiceMetrics::new_for_tests());

        Self {
            address,
            state,
            consensus_adapter,
            metrics,
        }
    }

    /// Creates a new `AuthorityServer` for testing.
    pub fn new_for_test(state: Arc<AuthorityState>) -> Self {
        let consensus_adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(LazyStarfishClient::new()),
            CheckpointStore::new_for_tests(),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
        ));
        Self::new_for_test_with_consensus_adapter(state, consensus_adapter)
    }

    /// Spawns the server.
    pub async fn spawn_for_test(self) -> Result<AuthorityServerHandle, io::Error> {
        let address = self.address.clone();
        self.spawn_with_bind_address_for_test(address).await
    }

    /// Spawns the server with a bind address.
    pub async fn spawn_with_bind_address_for_test(
        self,
        address: Multiaddr,
    ) -> Result<AuthorityServerHandle, io::Error> {
        let tls_config = iota_tls::create_rustls_server_config(
            self.state.config.network_key_pair().copy().private(),
            IOTA_TLS_SERVER_NAME.to_string(),
        );
        let server = iota_network_stack::config::Config::new()
            .server_builder()
            .add_service(ValidatorServer::new(ValidatorService::new_for_tests(
                self.state,
                self.consensus_adapter,
                self.metrics,
            )))
            .bind(&address, Some(tls_config))
            .await
            .unwrap();
        let local_addr = server.local_addr().to_owned();
        info!("Listening to traffic on {local_addr}");
        let handle = AuthorityServerHandle {
            server_handle: server,
        };
        Ok(handle)
    }
}
