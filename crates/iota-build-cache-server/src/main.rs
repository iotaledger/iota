// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;

use anyhow::Result;
use clap::Parser;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

mod cache;
mod server;
mod types;

use server::BuildCacheServer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Address to bind the server to
    #[arg(short, long, default_value = "0.0.0.0:8080")]
    address: SocketAddr,

    /// Directory to store cached binaries
    #[arg(short, long, default_value = "./build_cache")]
    cache_dir: String,

    /// Repository URL to clone and build from
    #[arg(short, long, default_value = "https://github.com/iotaledger/iota.git")]
    repository_url: String,

    /// Working directory for git operations and builds
    #[arg(short, long, default_value = "./git_workspace")]
    workspace_dir: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args = Args::parse();

    info!("Starting IOTA Build Cache Server on {}", args.address);
    info!("Cache directory: {}", args.cache_dir);
    info!("Repository URL: {}", args.repository_url);
    info!("Workspace directory: {}", args.workspace_dir);

    let server = BuildCacheServer::new(args.cache_dir, args.workspace_dir, args.repository_url)?;

    if let Err(e) = server.run(args.address).await {
        error!("Server error: {e}");
        return Err(e);
    }

    Ok(())
}
