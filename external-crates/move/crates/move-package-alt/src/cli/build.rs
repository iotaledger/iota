// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;

use clap::{Command, Parser, Subcommand};

use crate::{errors::PackageResult, flavor::Vanilla, package::Package};

/// Build the package
#[derive(Debug, Clone, Parser)]
pub struct Build {
    /// Path to the project
    #[arg(name = "path", short = 'p', long = "path", default_value = ".")]
    path: Option<PathBuf>,
}

impl Build {
    pub async fn execute(&self) -> PackageResult<()> {
        let path = self.path.clone().unwrap_or_else(|| PathBuf::from("."));

        let package = Package::<Vanilla>::load_root(path).await?;

        Ok(())
    }
}
