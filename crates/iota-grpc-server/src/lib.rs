// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Modules
mod error;
pub mod ledger_service;
pub mod server;
pub mod types;

// Re-export commonly used types and traits
pub use ledger_service::LedgerGrpcService;
pub use server::{GrpcServerHandle, start_grpc_server};
pub use types::{
    CheckpointDataBroadcaster, CheckpointSummaryBroadcaster, EventSubscriber,
    GrpcCheckpointDataBroadcaster, GrpcCheckpointSummaryBroadcaster, GrpcReader, GrpcStateReader,
    RestStateReaderAdapter,
};
