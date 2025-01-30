// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use clap::Parser;
use iota_move_build::set_iota_flavor;
#[cfg(feature = "unit_test")]
use move_cli::base::test::UnitTestResult;
use move_package::BuildConfig;

#[cfg(feature = "build")]
pub mod build;
#[cfg(feature = "coverage")]
pub mod coverage;
#[cfg(feature = "disassemble")]
pub mod disassemble;
pub mod manage_package;
pub mod migrate;
pub mod new;
#[cfg(feature = "unit_test")]
pub mod unit_test;

#[derive(Parser)]
pub enum Command {
    #[cfg(feature = "build")]
    Build(build::Build),
    #[cfg(feature = "coverage")]
    Coverage(coverage::Coverage),
    #[cfg(feature = "disassemble")]
    Disassemble(disassemble::Disassemble),
    ManagePackage(manage_package::ManagePackage),
    Migrate(migrate::Migrate),
    New(new::New),
    #[cfg(feature = "unit_test")]
    Test(unit_test::Test),
}
#[derive(Parser)]
pub struct Calib {
    #[arg(name = "runs", short = 'r', long = "runs", default_value = "1")]
    runs: usize,
    #[arg(name = "summarize", short = 's', long = "summarize")]
    summarize: bool,
}

pub fn execute_move_command(
    package_path: Option<&Path>,
    mut build_config: BuildConfig,
    command: Command,
) -> anyhow::Result<()> {
    if let Some(err_msg) = set_iota_flavor(&mut build_config) {
        anyhow::bail!(err_msg);
    }
    match command {
        #[cfg(feature = "build")]
        Command::Build(c) => c.execute(package_path, build_config),
        #[cfg(feature = "coverage")]
        Command::Coverage(c) => c.execute(package_path, build_config),
        #[cfg(feature = "disassemble")]
        Command::Disassemble(c) => c.execute(package_path, build_config),
        Command::ManagePackage(c) => c.execute(package_path, build_config),
        Command::Migrate(c) => c.execute(package_path, build_config),
        Command::New(c) => c.execute(package_path),

        #[cfg(feature = "unit_test")]
        Command::Test(c) => {
            let result = c.execute(package_path, build_config)?;

            // Return a non-zero exit code if any test failed
            if let UnitTestResult::Failure = result {
                std::process::exit(1)
            }

            Ok(())
        }
    }
}
