// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::*;

#[derive(Parser)]
#[command(
    name = "iota-graphql-rpc",
    about = "IOTA GraphQL RPC",
    rename_all = "kebab-case",
    author
)]
pub enum Command {
    /// Output a TOML config (suitable for passing into the --config parameter
    /// of the start-server command) with all values set to their defaults.
    GenerateConfig {
        /// Optional path to an output file. Prints to `stdout` if not provided.
        output: Option<PathBuf>,
    },
    GenerateSchema {
        /// Path to output GraphQL schema to, in SDL format.
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
    StartServer {
        /// The title to display at the top of the page
        #[arg(short, long)]
        ide_title: Option<String>,
        /// DB URL for data fetching
        #[arg(short, long)]
        db_url: Option<String>,
        /// Pool size for DB connections
        #[arg(long)]
        db_pool_size: Option<u32>,
        /// Port to bind the server to
        #[arg(short, long)]
        port: Option<u16>,
        /// Host to bind the server to
        #[arg(long)]
        host: Option<String>,
        /// Port to bind the prom server to
        #[arg(long)]
        prom_port: Option<u16>,
        /// Host to bind the prom server to
        #[arg(long)]
        prom_host: Option<String>,

        /// Path to TOML file containing configuration for service.
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// RPC url to the Node for tx execution
        #[arg(long)]
        node_rpc_url: Option<String>,
    },
}
