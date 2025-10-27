// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::*;
use iota_move_build::implicit_deps;
use iota_package_management::system_package_versions::latest_system_packages;
use move_analyzer::analyzer;

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[derive(Parser)]
#[command(
    name = env!("CARGO_BIN_NAME"),
    author,
    version = VERSION,
)]
struct App {}

fn main() {
    App::parse();
    analyzer::run(implicit_deps(latest_system_packages()));
}
