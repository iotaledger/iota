// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::*;

use crate::config::{ConnectionConfig, Ide, TxExecFullNodeConfig};

#[derive(Parser)]
#[command(name = "iota-graphql-rpc", about = "IOTA GraphQL RPC", author)]
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
        #[command(flatten)]
        ide: Ide,

        #[command(flatten)]
        connection: ConnectionConfig,

        /// Path to TOML file containing configuration for service.
        #[arg(short, long)]
        config: Option<PathBuf>,

        #[command(flatten)]
        tx_exec_full_node: TxExecFullNodeConfig,
    },
}
