// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Standalone DAG visualizer server binary.
//!
//! Connects to a validator's gRPC endpoint, persists DAG data per epoch
//! in RocksDB, and serves a REST/WebSocket API for the browser frontend.
mod config;
mod grpc_client;
mod http_server;
mod snapshot;
mod storage;
pub mod types;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use clap::Parser;
use tokio::sync::broadcast;
use tracing::info;

use crate::{config::Config, storage::StorageManager, types::DagVisualizerEvent};

#[tokio::main]
async fn main() {
    let config = Config::parse();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| config.log_level.parse().unwrap_or_default()),
        )
        .init();

    info!("Starting DAG visualizer server");
    info!("  Validator gRPC: {}", config.validator_grpc_address);
    info!("  Webserver address: {}", config.webserver_address);
    info!("  Data directory: {}", config.data_dir);
    info!("  Max epochs: {}", config.max_epochs);

    let storage = Arc::new(StorageManager::new(
        PathBuf::from(&config.data_dir),
        config.max_epochs,
    ));

    let (event_tx, _) = broadcast::channel::<DagVisualizerEvent>(config.broadcast_capacity);

    let listen_addr: SocketAddr = config
        .webserver_address
        .parse()
        .expect("Invalid webserver address");

    // Spawn the gRPC client (reconnects automatically)
    let grpc_storage = storage.clone();
    let grpc_event_tx = event_tx.clone();
    let validator_addr = config.validator_grpc_address.clone();
    tokio::spawn(async move {
        grpc_client::run_grpc_client(validator_addr, grpc_storage, grpc_event_tx).await;
    });

    // Run the HTTP/WS server (blocks)
    http_server::start_http_server(listen_addr, storage, event_tx).await;
}
