// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::OnceLock;

use tonic::transport::Channel;

use super::checkpoint::CheckpointClient;

/// gRPC client factory for IOTA node operations.
pub struct NodeClient {
    /// Shared gRPC channel for all service clients
    channel: Channel,
    /// Cached checkpoint client (singleton)
    checkpoint_client: OnceLock<CheckpointClient>,
}

impl NodeClient {
    /// Connect to a gRPC server and create a new NodeClient instance.
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let channel = Channel::from_shared(url.to_string())?.connect().await?;

        Ok(Self {
            channel,
            checkpoint_client: OnceLock::new(),
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

    /// Get a checkpoint service client.
    ///
    /// Returns `Some(CheckpointClient)` if the node supports checkpoint
    /// operations, `None` otherwise. The client is created only once and
    /// cached for subsequent calls.
    pub fn checkpoint_client(&self) -> Option<CheckpointClient> {
        // For now, always return Some since checkpoint service is always available
        // In the future, this could check node capabilities first
        Some(
            self.checkpoint_client
                .get_or_init(|| CheckpointClient::new(self.channel.clone()))
                .clone(),
        )
    }
}
