// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
extern crate core;

use clap::*;
use colored::Colorize;
use iota_tool::commands::ToolCommand;
use iota_types::exit_main;

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    colored::control::set_virtual_terminal(true).unwrap();

    let cmd: ToolCommand = ToolCommand::parse();
    let (_guards, handle) = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    exit_main!(cmd.execute(handle).await);
}
