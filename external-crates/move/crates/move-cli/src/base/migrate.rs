// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::*;
use move_package::BuildConfig;

use super::reroot_path;

/// Migrate to Move 2024 for the package at `path`. If no path is provided
/// defaults to current directory.
#[derive(Parser)]
#[clap(name = "migrate")]
pub struct Migrate;

impl Migrate {
    pub fn execute(self, path: Option<PathBuf>, config: BuildConfig) -> anyhow::Result<()> {
        let rerooted_path = reroot_path(path)?;
        config.migrate_package(
            &rerooted_path,
            &mut std::io::stdout(),
            &mut std::io::stdin().lock(),
        )?;
        Ok(())
    }
}
