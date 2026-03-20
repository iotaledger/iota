// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;

/// Standalone DAG visualizer server.
///
/// Connects to a validator's gRPC endpoint, persists DAG data per epoch
/// in RocksDB, and serves a REST/WebSocket API for the browser frontend.
#[derive(Parser, Debug, Clone)]
#[command(name = "dag-visualizer-server")]
pub struct Config {
    /// Validator gRPC address (e.g. http://127.0.0.1:9185)
    #[arg(long, default_value = "http://127.0.0.1:9185")]
    pub validator_grpc_address: String,

    /// Webserver listen address (e.g. 0.0.0.0:9186)
    #[arg(long, default_value = "127.0.0.1:9186")]
    pub webserver_address: String,

    /// Broadcast channel capacity for real-time events.
    /// Larger values buffer more events before lagging; smaller values use less
    /// memory.
    #[arg(long, default_value_t = 4096)]
    pub broadcast_capacity: usize,

    /// Directory for RocksDB epoch storage
    #[arg(long, default_value = "./dag-visualizer-data/")]
    pub data_dir: String,

    /// Maximum number of epoch databases to keep
    #[arg(long, default_value_t = 2)]
    pub max_epochs: usize,

    /// Log level
    #[arg(long, default_value = "info")]
    pub log_level: String,
}
