// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use move_cli::base::migrate;
use move_package::BuildConfig as MoveBuildConfig;
use std::path::Path;

#[derive(Parser)]
#[group(id = "iota-move-migrate")]
pub struct Migrate {
    #[clap(flatten)]
    pub migrate: migrate::Migrate,
}

impl Migrate {
    pub fn execute(self, path: Option<&Path>, config: MoveBuildConfig) -> anyhow::Result<()> {
        self.migrate.execute(path, config)
    }
}
