// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use colored::Colorize;
use iota_tool::commands::ToolCommand;
use iota_types::exit_main;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[command(
    name = "iota-tool",
    about = "Debugging utilities for iota",
    author,
    version = VERSION,
)]
struct App {
    #[command(subcommand)]
    command: ToolCommand,
}

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();

    let app = App::parse();
    let (_guards, handle) = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    exit_main!(app.command.execute(handle).await);
}
