// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, num::NonZeroUsize, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use server::Server;
use tokio_util::sync::CancellationToken;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// This module contains the DynamoDb and S3 implementation of the KV store
/// client.
#[allow(dead_code)]
mod aws;
/// This module contains the Bigtable implementation of the KV store client.
mod bigtable;
mod errors;
mod extractors;
mod routes;
mod server;
mod types;

use bigtable::KvStoreConfig;

/// The main CLI application.
#[derive(Parser, Clone, Debug)]
#[clap(
    name = "KV Store REST API",
    about = "A HTTP server exposing key-value data of the IOTA network through a REST API."
)]
struct Cli {
    #[clap(long, default_value = "INFO", env = "LOG_LEVEL")]
    log_level: Level,
    /// The yaml config file path.
    #[clap(short, long)]
    config: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct RestApiConfig {
    #[serde(flatten)]
    pub kv_store_config: KvStoreConfig,
    pub server_address: std::net::SocketAddr,
    #[serde(default = "default_multiget_max_items")]
    pub multiget_max_items: NonZeroUsize,
}

fn default_multiget_max_items() -> NonZeroUsize {
    NonZeroUsize::new(100).expect("value should be greater than 0")
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let cli = Cli::parse();

    init_tracing(cli.log_level);

    let raw_config = fs::read_to_string(cli.config).expect("failed to read config file");
    let config = serde_yaml::from_str::<RestApiConfig>(&raw_config)?;

    let token = CancellationToken::new();

    shutdown_signal_listener(token.clone());

    let server = Server::new(config, token).await?;
    server.serve().await
}

/// Initialize the tracing with custom subscribers.
fn init_tracing(log_level: Level) {
    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

/// Set up a `CTRL+C` & `SIGTERM` handler for graceful shutdown and spawn a
/// tokio task.
fn shutdown_signal_listener(token: CancellationToken) {
    tokio::spawn(async move {
        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("cannot listen to SIGTERM signal")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = tokio::signal::ctrl_c() => tracing::info!("shutting down, CTRL+C signal received"),
            _ = terminate => tracing::info!("shutting down, SIGTERM signal received")
        };

        token.cancel();
    });
}
