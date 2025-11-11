// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC client for IOTA node operations.

mod ledger;
mod node_client;

pub use ledger::LedgerClient;
pub use node_client::NodeClient;
