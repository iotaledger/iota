// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::OnceLock;

use tonic::transport::Channel;

use crate::LedgerClient;

/// gRPC client factory for IOTA node operations.
pub struct NodeClient {
    /// Shared gRPC channel for all service clients
    channel: Channel,
    /// Cached ledger client (singleton)
    ledger_client: OnceLock<LedgerClient>,
}

impl NodeClient {
    /// Connect to a gRPC server and create a new NodeClient instance.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let channel = Channel::from_shared(url.to_string())?.connect().await?;

        Ok(Self {
            channel,
            ledger_client: OnceLock::new(),
        })
    }

    /// Get a reference to the underlying channel.
    ///
    /// This can be useful for creating additional service clients that aren't
    /// yet integrated into NodeClient.
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    // ========================================
    // Service Client Factories
    // ========================================

    /// Get a ledger service client.
    ///
    /// Returns `Some(LedgerClient)` if the node supports ledger-related
    /// operations, `None` otherwise. The client is created only once and
    /// cached for subsequent calls.
    pub fn ledger_service_client(&self) -> Option<super::ledger::LedgerClient> {
        // For now, always return Some since ledger service is always available
        // In the future, this could check node capabilities first
        Some(
            self.ledger_client
                .get_or_init(|| LedgerClient::new(self.channel.clone()))
                .clone(),
        )
    }
}
