// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use colored::Colorize;
use iota::{
    client_commands::IotaClientCommands::{ProfileTransaction, ReplayBatch, ReplayTransaction},
    iota_commands::IotaCommand,
};
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
        IotaCommand::KeyTool { .. } | IotaCommand::Move { .. } => {
            telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("error")
                .with_env()
                .init()
        }
        IotaCommand::Client {
            cmd: Some(ReplayBatch { .. }),
            ..
        } => telemetry_subscribers::TelemetryConfig::new()
            .with_log_level("info")
            .with_env()
            .init(),

        IotaCommand::Client {
            cmd: Some(ReplayTransaction {
                gas_info, ptb_info, ..
            }),
            ..
        } => {
            let mut config = telemetry_subscribers::TelemetryConfig::new()
                .with_log_level("info")
                .with_env();
            if gas_info {
                config = config.with_trace_target("replay_gas_info");
            }
            if ptb_info {
                config = config.with_trace_target("replay_ptb_info");
            }
            config.init()
        }
        IotaCommand::Client {
            cmd: Some(ProfileTransaction { .. }),
            ..
        } => {
            // enable full logging for ProfileTransaction and ReplayTransaction
            telemetry_subscribers::TelemetryConfig::new()
                .with_env()
                .init()
        }
        IotaCommand::Start { .. } => telemetry_subscribers::TelemetryConfig::new()
            .with_log_level("info")
            .with_env()
            .init(),
        _ => telemetry_subscribers::TelemetryConfig::new()
            .with_log_level("error")
            .with_env()
            .init(),
    };
    debug!("IOTA CLI version: {VERSION}");
    exit_main!(args.command.execute().await);
}
