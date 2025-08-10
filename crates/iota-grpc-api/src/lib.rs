// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Generated protobuf code
pub mod checkpoint {
    tonic::include_proto!("iota.grpc");
}

// Modules
pub mod checkpoint_service;
pub mod client;
pub mod config;
pub mod server;
pub mod types;

// Re-export commonly used types and traits
pub use checkpoint_service::CheckpointGrpcService;
pub use client::{CheckpointClient, CheckpointContent, NodeClient};
pub use config::Config;
pub use server::{GrpcServerHandle, start_grpc_server};
pub use types::{
    CheckpointDataBroadcaster, CheckpointSummaryBroadcaster, GrpcCheckpointDataBroadcaster,
    GrpcCheckpointSummaryBroadcaster, GrpcReader, GrpcStateReader, RestStateReaderAdapter,
};
