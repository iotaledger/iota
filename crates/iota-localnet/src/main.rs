// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use colored::Colorize;
use iota_localnet::commands::LocalnetCommand;
use tracing::debug;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[command(
    name = "iota-localnet",
    about = "Start and manage IOTA local networks",
    author,
    version = VERSION,
    propagate_version = true,
)]
struct Args {
    #[command(subcommand)]
    command: LocalnetCommand,
}

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();

    let args = Args::parse();
    let _guard = match &args.command {
        LocalnetCommand::Start { .. } => Some(
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("info")
                .with_env()
                .init(),
        ),
        LocalnetCommand::Genesis { .. } => Some(
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("info")
                .with_env()
                .init(),
        ),
    };
    debug!("iota-localnet version: {VERSION}");
    // `start --print-config` writes the node configs to stdout, so errors
    // go to stderr to keep that stream a payload.
    if let Err(err) = args.command.execute().await {
        eprintln!("{}", format!("{err:?}").bold().red());
        std::process::exit(1);
    }
}
