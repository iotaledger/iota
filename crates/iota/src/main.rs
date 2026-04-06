// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use colored::Colorize;
use iota::iota_commands::IotaCommand;
use iota_types::exit_main;
use tracing::debug;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    author,
    version = VERSION,
    propagate_version = true,
)]
struct Args {
    #[command(subcommand)]
    command: IotaCommand,
}

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();

    let args = Args::parse();
    let _guard = match args.command {
        IotaCommand::KeyTool { .. } | IotaCommand::Move { .. } => Some(
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("error")
                .with_env()
                .init(),
        ),
        IotaCommand::Analyzer => None,
        _ => Some(
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("error")
                .with_env()
                .init(),
        ),
    };
    debug!("IOTA CLI version: {VERSION}");
    exit_main!(args.command.execute().await);
}
