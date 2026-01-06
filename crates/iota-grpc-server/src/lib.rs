// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[macro_use]
mod macros;

// Modules
pub mod constants;
mod error;
pub mod ledger_service;
pub mod response;
pub mod server;
pub mod transaction_execution_service;
pub mod types;
pub mod utils;

// Re-export commonly used types and traits
pub use ledger_service::LedgerGrpcService;
pub use response::append_info_headers;
pub use server::{GrpcServerHandle, start_grpc_server};
pub use transaction_execution_service::TransactionExecutionGrpcService;
pub use types::{
    CheckpointDataBroadcaster, CheckpointSummaryBroadcaster, EventSubscriber,
    GrpcCheckpointDataBroadcaster, GrpcCheckpointSummaryBroadcaster, GrpcReader, GrpcStateReader,
    RestStateReaderAdapter,
};
