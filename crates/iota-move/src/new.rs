// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::create_dir_all,
    io::Write,
    path::{Path, PathBuf},
};

use clap::Parser;
use move_cli::base::new;
use move_package::source_package::layout::SourcePackageLayout;

const IOTA_PKG_NAME: &str = "Iota";

// Use testnet by default. Probably want to add options to make this
// configurable later
const IOTA_PKG_PATH: &str = "{ git = \"https://github.com/iotaledger/iota.git\", subdir = \"crates/iota-framework/packages/iota-framework\", rev = \"framework/testnet\" }";

#[derive(Parser)]
#[group(id = "iota-move-new")]
pub struct New {
    #[clap(flatten)]
    pub new: new::New,
}

impl New {
    pub fn execute(self, path: Option<PathBuf>) -> anyhow::Result<()> {
        let name = &self.new.name.to_lowercase();
        let p = match &path {
            Some(path) => path,
            None => Path::new(&name),
        };

        self.new.execute(
            path.clone(),
            [(IOTA_PKG_NAME, IOTA_PKG_PATH)],
            [(name, "0x0")],
            "",
        )?;

        let mut w = std::fs::File::create(
            p.join(SourcePackageLayout::Sources.path())
                .join(format!("{name}.move")),
        )?;
        writeln!(
            w,
            r#"/*
/// Module: {name}
module {name}::{name} {{

}}
*/"#,
            name = name
        )?;

        create_dir_all(p.join(SourcePackageLayout::Tests.path()))?;
        let mut w = std::fs::File::create(
            p.join(SourcePackageLayout::Tests.path())
                .join(format!("{name}_tests.move")),
        )?;
        writeln!(
            w,
            r#"/*
#[test_only]
module {name}::{name}_tests {{
    // uncomment this line to import the module
    // use {name}::{name};

    const ENotImplemented: u64 = 0;

    #[test]
    fun test_{name}() {{
        // pass
    }}

    #[test, expected_failure(abort_code = {name}::{name}_tests::ENotImplemented)]
    fun test_{name}_fail() {{
        abort ENotImplemented
    }}
}}
*/"#,
            name = name
        )?;

        Ok(())
    }
}
