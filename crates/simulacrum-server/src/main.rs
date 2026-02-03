// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Simulacrum Server Binary
//! This binary provides a gRPC and REST API server for a simulacrum instance.
//! It allows external clients to interact with a simulated IOTA blockchain.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Result;
use clap::Parser;
use iota_swarm_config::genesis_config::AccountConfig;
use iota_types::storage::RestStateReader;
use simulacrum::Simulacrum;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tower::ServiceBuilder;
use tracing::{error, info, level_filters::LevelFilter, warn};

mod faucet;
mod grpc_server;
mod rest_api;

use rest_api::AppState;
use tracing_subscriber::EnvFilter;

/// Command line arguments for the simulacrum server
#[derive(Parser, Debug)]
#[command(
    name = "simulacrum-server",
    about = "A gRPC and REST API server to simulate the IOTA blockchain"
)]
struct Args {
    /// gRPC server address
    #[arg(long, default_value = "127.0.0.1:9000")]
    grpc_address: SocketAddr,

    /// REST API server address
    #[arg(long, default_value = "127.0.0.1:8080")]
    rest_address: SocketAddr,

    /// Initial number of checkpoints to create on startup
    #[arg(long, default_value = "0")]
    initial_checkpoints: u64,

    /// Chain start timestamp in milliseconds
    #[arg(long)]
    chain_start_timestamp_ms: Option<u64>,

    /// Faucet request amount in nanos (default: 1_000_000_000 = 1 IOTA)
    #[arg(long, default_value = "1000000000")]
    faucet_request_amount: u64,

    /// Accounts to create in the format "address:amount,address:amount..."
    #[arg(long)]
    accounts: Option<String>,

    /// Path to store data ingestion files
    #[arg(long)]
    data_ingestion_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = ::tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("unable to initialize logging");

    let args = Args::parse();

    info!("Starting Simulacrum Server");
    info!("REST address: {}", args.rest_address);
    info!("gRPC address: {}", args.grpc_address);

    // Create simulacrum instance
    info!(
        "Creating simulacrum with chain start timestamp: {}",
        args.chain_start_timestamp_ms.unwrap_or_default()
    );

    // Parse accounts from command line argument
    let mut account_configs = if let Some(accounts_str) = args.accounts {
        let pairs: Vec<&str> = accounts_str.split(',').collect();
        info!("Creating {} accounts", pairs.len());

        pairs
            .iter()
            .map(|pair| {
                let parts: Vec<&str> = pair.split(':').collect();
                if parts.len() != 2 {
                    panic!("Invalid account format '{pair}', expected 'address:amount'");
                }

                let address = match parts[0].parse::<iota_types::base_types::IotaAddress>() {
                    Ok(addr) => addr,
                    Err(e) => {
                        panic!("Invalid address '{}': {e}", parts[0]);
                    }
                };

                let amount = match parts[1].parse::<u64>() {
                    Ok(amt) => amt,
                    Err(e) => {
                        panic!("Invalid amount '{}': {e}", parts[1]);
                    }
                };

                AccountConfig {
                    address: Some(address),
                    gas_amounts: vec![amount],
                }
            })
            .collect()
    } else {
        info!("No accounts specified");
        vec![]
    };

    // create an account for the faucet
    let faucet_account = AccountConfig {
        address: None,
        gas_amounts: vec![100_000_000_000_000],
    };
    account_configs.insert(0, faucet_account); // ensure faucet account is first

    let simulacrum = Simulacrum::new_with_protocol_version_and_accounts(
        rand::rngs::OsRng,
        args.chain_start_timestamp_ms.unwrap_or_default(),
        iota_protocol_config::ProtocolVersion::MAX,
        account_configs,
    );

    // Set data ingestion path if provided
    if let Some(path) = args.data_ingestion_path {
        info!("Setting data ingestion path to: {}", path);
        simulacrum.set_data_ingestion_path(path.into());
    }

    // Create initial checkpoints if requested
    if args.initial_checkpoints > 0 {
        info!("Creating {} initial checkpoints", args.initial_checkpoints);
        for i in 0..args.initial_checkpoints {
            simulacrum.advance_clock(Duration::from_secs(1));
            let checkpoint = simulacrum.create_checkpoint();
            info!(
                "Created initial checkpoint {}: sequence {}",
                i + 1,
                checkpoint.sequence_number()
            );
        }
    }

    let simulacrum = Arc::new(simulacrum);
    let app_state = AppState {
        simulacrum: simulacrum.clone(),
        faucet_request_amount: args.faucet_request_amount,
        chain_id: simulacrum
            .get_chain_identifier()
            .expect("chain identifier should be set")
            .to_string(),
    };

    // Start gRPC server
    let shutdown_token = CancellationToken::new();
    let grpc_handle = {
        info!("Starting gRPC server on {}", args.grpc_address);

        let grpc_config = iota_config::node::GrpcApiConfig {
            address: args.grpc_address,
            ..Default::default()
        };

        match grpc_server::start_simulacrum_grpc_server(
            simulacrum.clone(),
            grpc_config,
            shutdown_token.clone(),
        )
        .await
        {
            Ok(handle) => {
                info!("gRPC server started successfully on {}", handle.address());
                Some(handle)
            }
            Err(e) => {
                panic!("Failed to start gRPC server: {e}");
            }
        }
    };

    // Start REST API server
    let rest_handle = {
        info!("Starting REST API server on {}", args.rest_address);

        let router = rest_api::create_router(app_state)
            .layer(ServiceBuilder::new().layer(axum::middleware::from_fn(
            |req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| async move {
                let start = std::time::Instant::now();
                let method = req.method().clone();
                let uri = req.uri().clone();

                let response = next.run(req).await;
                let elapsed = start.elapsed();

                tracing::debug!("{method} {uri} took {elapsed:?}");
                response
            },
        )));

        let listener = tokio::net::TcpListener::bind(args.rest_address).await?;

        Some(tokio::spawn(async move {
            info!("REST API server listening on {}", args.rest_address);
            if let Err(e) = axum::serve(listener, router).await {
                error!("REST API server error: {}", e);
            }
        }))
    };

    info!("Simulacrum Server started successfully");
    info!("");
    info!("Available REST endpoints:");
    info!("  GET  /status                     - Simulacrum status");
    info!("  GET  /checkpoint                 - Get latest checkpoint");
    info!("  POST /checkpoint/create          - Create a new checkpoint");
    info!("  POST /checkpoint/create_multiple - Create multiple checkpoints");
    info!("  POST /clock/advance              - Advance simulacrum clock");
    info!("  POST /epoch/advance              - Advance to next epoch");
    info!("");

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Received shutdown signal, stopping servers...");
        }
        Err(err) => {
            warn!("Failed to listen for shutdown signal: {err}");
        }
    }

    // Shutdown servers
    shutdown_token.cancel();

    if let Some(handle) = rest_handle {
        handle.abort();
        info!("REST API server stopped");
    }

    if let Some(handle) = grpc_handle {
        if let Err(e) = handle.shutdown().await {
            warn!("Error shutting down gRPC server: {}", e);
        }
        info!("gRPC server stopped");
    }

    info!("Simulacrum Server stopped");
    Ok(())
}
