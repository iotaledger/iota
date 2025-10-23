// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::OnceLock;

use tonic::transport::Channel;

use super::{
    checkpoint::CheckpointClient, event::EventClient, read::ReadClient,
    transaction::TransactionClient, write::WriteClient,
};

/// gRPC client factory for IOTA node operations.
pub struct NodeClient {
    /// Shared gRPC channel for all service clients
    channel: Channel,
    /// Cached checkpoint client (singleton)
    checkpoint_client: OnceLock<CheckpointClient>,
    /// Cached event client (singleton)
    event_client: OnceLock<EventClient>,
    /// Cached transaction client (singleton)
    transaction_client: OnceLock<TransactionClient>,
    /// Cached read client (singleton)
    read_client: OnceLock<ReadClient>,
    /// Cached write client (singleton)
    write_client: OnceLock<WriteClient>,
}

impl NodeClient {
    /// Connect to a gRPC server and create a new NodeClient instance.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let channel = Channel::from_shared(url.to_string())?.connect().await?;

        Ok(Self {
            channel,
            checkpoint_client: OnceLock::new(),
            event_client: OnceLock::new(),
            transaction_client: OnceLock::new(),
            read_client: OnceLock::new(),
            write_client: OnceLock::new(),
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

    /// Get an event service client.
    ///
    /// Returns `Some(EventClient)` if the node supports event streaming
    /// operations, `None` otherwise. The client is created only once and
    /// cached for subsequent calls.
    pub fn event_client(&self) -> Option<EventClient> {
        // For now, always return Some since event service is always available
        // In the future, this could check node capabilities first
        Some(
            self.event_client
                .get_or_init(|| EventClient::new(self.channel.clone()))
                .clone(),
        )
    }

    /// Get a transaction service client.
    ///
    /// Returns `Some(TransactionClient)` if the node supports transaction
    /// streaming operations, `None` otherwise. The client is created only
    /// once and cached for subsequent calls.
    pub fn transaction_client(&self) -> Option<TransactionClient> {
        // For now, always return Some since transaction service is always available
        // In the future, this could check node capabilities first
        Some(
            self.transaction_client
                .get_or_init(|| TransactionClient::new(self.channel.clone()))
                .clone(),
        )
    }

    /// Get a read service client.
    ///
    /// Returns `Some(ReadClient)` if the node supports read operations,
    /// `None` otherwise. The client is created only once and cached for
    /// subsequent calls.
    pub fn read_client(&self) -> Option<ReadClient> {
        // For now, always return Some since read service is always available
        // In the future, this could check node capabilities first
        Some(
            self.read_client
                .get_or_init(|| ReadClient::new(self.channel.clone()))
                .clone(),
        )
    }

    /// Get a write service client.
    ///
    /// Returns `Some(WriteClient)` if the node supports write operations,
    /// `None` otherwise. The client is created only once and cached for
    /// subsequent calls.
    pub fn write_client(&self) -> Option<WriteClient> {
        // For now, always return Some since write service is always available
        // In the future, this could check node capabilities first
        Some(
            self.write_client
                .get_or_init(|| WriteClient::new(self.channel.clone()))
                .clone(),
        )
    }
}
