// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf};

use clap::{CommandFactory, FromArgMatches};
use iota_graphql_rpc::{
    commands::Command,
    config::{ConnectionConfig, Ide, ServerConfig, ServiceConfig, TxExecFullNodeConfig, Version},
    server::{builder::export_schema, graphiql_server::start_graphiql_server},
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

// VERSION_VAL mimics what other iota binaries use for the VERSION const
static VERSION_VAL: Version = Version {
    year: env!("CARGO_PKG_VERSION_MAJOR"),
    month: env!("CARGO_PKG_VERSION_MINOR"),
    patch: env!("CARGO_PKG_VERSION_PATCH"),
    sha: GIT_REVISION,
    full: VERSION,
};

#[tokio::main]
async fn main() {
    let cmd = Command::from_arg_matches_mut(&mut Command::command().version(VERSION).get_matches())
        .unwrap();
    match cmd {
        Command::GenerateConfig { output } => {
            let config = ServiceConfig::default();
            let toml = toml::to_string_pretty(&config).expect("Failed to serialize configuration");

            if let Some(path) = output {
                fs::write(&path, toml).unwrap_or_else(|e| {
                    panic!("Failed to write configuration to {}: {e}", path.display())
                });
            } else {
                println!("{}", toml);
            }
        }
        Command::GenerateSchema { file } => {
            let out = export_schema();
            if let Some(file) = file {
                println!("Write schema to file: {:?}", file);
                std::fs::write(file, &out).unwrap();
            } else {
                println!("{}", out);
            }
        }
        Command::StartServer {
            ide_title,
            db_url,
            db_pool_size,
            port,
            host,
            config,
            node_rpc_url,
            prom_host,
            prom_port,
        } => {
            let connection =
                ConnectionConfig::new(port, host, db_url, db_pool_size, prom_host, prom_port);
            let service_config = service_config(config);
            let _guard = telemetry_subscribers::TelemetryConfig::new()
                .with_env()
                .init();
            let tracker = TaskTracker::new();
            let cancellation_token = CancellationToken::new();

            println!("Starting server...");
            let server_config = ServerConfig {
                connection,
                service: service_config,
                ide: Ide::new(ide_title),
                tx_exec_full_node: TxExecFullNodeConfig::new(node_rpc_url),
                ..ServerConfig::default()
            };

            let cancellation_token_clone = cancellation_token.clone();
            let graphql_service_handle = tracker.spawn(async move {
                start_graphiql_server(&server_config, &VERSION_VAL, cancellation_token_clone)
                    .await
                    .unwrap();
            });

            // Wait for shutdown signal
            tokio::select! {
                result = graphql_service_handle => {
                    if let Err(e) = result {
                        println!("GraphQL service crashed or exited with error: {:?}", e);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("Ctrl+C signal received.");
                },
            }

            println!("Shutting down...");

            // Send shutdown signal to application
            cancellation_token.cancel();
            tracker.close();
            tracker.wait().await;
        }
    }
}

fn service_config(path: Option<PathBuf>) -> ServiceConfig {
    let Some(path) = path else {
        return ServiceConfig::default();
    };

    let contents = fs::read_to_string(path).expect("Reading configuration");
    ServiceConfig::read(&contents).expect("Deserializing configuration")
}
