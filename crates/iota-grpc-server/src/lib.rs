// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Modules
pub mod checkpoint_service;
pub mod event_service;
pub mod read_service;
pub mod server;
pub mod transaction_service;
pub mod types;
pub mod write_service;

// Re-export commonly used types and traits
pub use checkpoint_service::CheckpointGrpcService;
pub use event_service::EventGrpcService;
pub use read_service::ReadGrpcService;
pub use server::{GrpcServerHandle, start_grpc_server};
pub use transaction_service::TransactionGrpcService;
pub use types::{
    CheckpointDataBroadcaster, CheckpointSummaryBroadcaster, EventSubscriber,
    GrpcCheckpointDataBroadcaster, GrpcCheckpointSummaryBroadcaster, GrpcReader,
};
pub use write_service::WriteGrpcService;
