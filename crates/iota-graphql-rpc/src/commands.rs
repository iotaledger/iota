// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::*;

use crate::config::{ConnectionConfig, Ide, TxExecFullNodeConfig};

#[derive(Parser)]
#[clap(
    name = "iota-graphql-rpc",
    about = "Iota GraphQL RPC",
    rename_all = "kebab-case",
    author,
    version
)]
pub enum Command {
    /// Output a TOML config (suitable for passing into the --config parameter
    /// of the start-server command) with all values set to their defaults.
    GenerateConfig {
        /// Optional path to an output file. Prints to `stdout` if not provided.
        output: Option<PathBuf>,
    },

    StartServer {
        #[clap(flatten)]
        ide: Ide,

        #[clap(flatten)]
        connection: ConnectionConfig,

        /// Path to TOML file containing configuration for service.
        #[clap(short, long)]
        config: Option<PathBuf>,

        #[clap(flatten)]
        tx_exec_full_node: TxExecFullNodeConfig,
    },
}
