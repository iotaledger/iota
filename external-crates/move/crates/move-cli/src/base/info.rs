// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use clap::*;
use move_package::BuildConfig;

use super::reroot_path;

/// Print address information.
#[derive(Parser)]
#[clap(name = "info")]
pub struct Info;

impl Info {
    pub fn execute(self, path: Option<&Path>, config: BuildConfig) -> anyhow::Result<()> {
        let rerooted_path = reroot_path(path)?;
        config
            .resolution_graph_for_package(&rerooted_path, None, &mut std::io::stdout())?
            .print_info()
    }
}
