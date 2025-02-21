// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use move_analyzer::analyzer;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    rename_all = "kebab-case",
    author,
    version = VERSION,
)]
struct App {}

fn main() {
    App::parse();
    analyzer::run();
}
