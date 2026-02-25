// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[macro_use]
mod macros;

// Re-export field_mask! from iota-grpc-types so existing `crate::field_mask!`
// call sites in this crate continue to work without modification.
pub use iota_grpc_types::field_mask;

// Modules
pub mod constants;
mod error;
pub mod event_filter;
pub mod ledger_service;
pub mod merge;
pub mod response;
pub mod server;
pub mod transaction_execution_service;
pub mod transaction_filter;
pub mod types;
pub mod utils;

// Re-export commonly used types and traits
pub use ledger_service::LedgerGrpcService;
pub use response::append_info_headers;
pub use server::{GrpcServerHandle, start_grpc_server};
pub use transaction_execution_service::TransactionExecutionGrpcService;
pub use types::{
    GrpcCheckpointDataBroadcaster, GrpcReader, GrpcStateReader, RestStateReaderAdapter,
};
