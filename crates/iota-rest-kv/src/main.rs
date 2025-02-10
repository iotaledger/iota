// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use server::Server;
use services::KvStoreConfig;
use tokio::signal::unix::SignalKind;
use tokio_util::sync::CancellationToken;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod errors;
mod extractors;
mod routes;
mod server;
mod services;
mod types;

/// The main CLI application
#[derive(Parser, Clone, Debug)]
#[clap(
    name = "KV Store REST API",
    about = "Serves through a REST API the KV Store data from ingestion pipeline"
)]
struct Cli {
    #[clap(long, default_value = "INFO", env = "LOG_LEVEL")]
    log_level: Level,
    /// The yaml config file path
    #[clap(short, long)]
    config: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct RestApiConfig {
    #[serde(flatten)]
    pub kv_store_config: KvStoreConfig,
    pub rest_api_address: std::net::SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    init_tracing(cli.log_level);

    let raw_config = fs::read_to_string(cli.config).expect("failed to read config file");
    let config = serde_yaml::from_str::<RestApiConfig>(&raw_config)?;

    let token = CancellationToken::new();

    shutdown_signal_listener(token.clone());

    let server = Server::new(config, token).await?;
    server.serve().await
}

/// Initialize the tracing with custom subscribers
fn init_tracing(log_level: Level) {
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

/// Set up a `CTRL+C` & `SIGTERM` handler for graceful shutdown and spawn a
/// tokio task
fn shutdown_signal_listener(token: CancellationToken) {
    tokio::spawn(async move {
        let mut signal_stream = tokio::signal::unix::signal(SignalKind::terminate())
            .expect("Cannot listen to SIGTERM signal");

        tokio::select! {
            _ = tokio::signal::ctrl_c() => tracing::info!("CTRL+C signal received, shutting down"),
            _ = signal_stream.recv() => tracing::info!("SIGTERM signal received, shutting down")
        };

        token.cancel();
    });
}
