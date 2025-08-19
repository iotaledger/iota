// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Generated protobuf code
pub mod checkpoint {
    tonic::include_proto!("iota.grpc");
}

pub mod events {
    tonic::include_proto!("iota.grpc.events");
}

// Modules
pub mod bcs_event;
pub mod checkpoint_service;
pub mod client;
pub mod config;
pub mod event_service;
pub mod server;
pub mod types;

// Re-export commonly used types and traits
pub use checkpoint_service::CheckpointGrpcService;
pub use client::{CheckpointClient, CheckpointContent, EventClient, NodeClient};
pub use config::Config;
pub use event_service::EventGrpcService;
pub use server::{GrpcServerHandle, start_grpc_server};
pub use types::{
    CheckpointDataBroadcaster, CheckpointSummaryBroadcaster, GrpcCheckpointDataBroadcaster,
    GrpcCheckpointSummaryBroadcaster, GrpcEventBroadcaster, GrpcReader, GrpcStateReader,
    RestStateReaderAdapter,
};
