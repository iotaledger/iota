// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Configuration for the gRPC API service
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// The address to bind the gRPC server to
    #[serde(default = "default_grpc_api_address")]
    pub address: std::net::SocketAddr,

    /// Buffer size for broadcast channels used for checkpoint streaming
    #[serde(default = "default_checkpoint_broadcast_buffer_size")]
    pub checkpoint_broadcast_buffer_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: default_grpc_api_address(),
            checkpoint_broadcast_buffer_size: default_checkpoint_broadcast_buffer_size(),
        }
    }
}

fn default_grpc_api_address() -> std::net::SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    std::net::SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 50051)
}

fn default_checkpoint_broadcast_buffer_size() -> usize {
    100
}

// Generated protobuf code
pub mod checkpoint {
    tonic::include_proto!("iota.grpc");
}

// Modules
pub mod checkpoint_service;
pub mod client;
pub mod types;

// Re-export commonly used types and traits
pub use checkpoint_service::CheckpointGrpcService;
pub use client::{CheckpointClient, CheckpointContent, NodeClient};
pub use types::{
    CheckpointDataBroadcaster, CheckpointSummaryBroadcaster, GrpcCheckpointDataBroadcaster,
    GrpcCheckpointSummaryBroadcaster,
};
