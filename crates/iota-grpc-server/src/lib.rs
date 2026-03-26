// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[macro_use]
mod macros;

// Modules
pub mod constants;
mod error;
pub mod event_filter;
pub mod ledger_service;
pub mod merge;
pub mod metrics;
pub mod move_package_service;
pub mod response;
pub mod server;
pub mod state_service;
pub mod transaction_execution_service;
pub mod transaction_filter;
pub mod types;
pub mod utils;
// Internal helpers — not part of the public API.
pub(crate) mod validation;

// Re-export commonly used types and traits
pub use ledger_service::LedgerGrpcService;
pub use metrics::GrpcServerMetrics;
pub use move_package_service::MovePackageGrpcService;
pub use response::append_info_headers;
pub use server::{GrpcServerHandle, start_grpc_server};
pub use state_service::StateGrpcService;
pub use transaction_execution_service::TransactionExecutionGrpcService;
pub use types::{
    DynamicFieldIterItem, GrpcCheckpointDataBroadcaster, GrpcReader, GrpcStateReader,
    OwnedObjectIterItem, OwnedObjectV2Cursor, OwnedObjectV2IterItem, PackageVersionIterItem,
    RestStateReaderAdapter,
};
