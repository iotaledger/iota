// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs::create_dir_all, io::Write, path::Path};

use clap::Parser;
use move_cli::base::new;
use move_package::source_package::layout::SourcePackageLayout;

#[derive(Parser)]
#[group(id = "iota-move-new")]
pub struct New {
    #[command(flatten)]
    pub new: new::New,
}

impl New {
    pub fn execute(self, path: Option<&Path>) -> anyhow::Result<()> {
        let name = &self.new.name.to_lowercase();
        let provided_name = &self.new.name.to_string();

        self.new
            .execute(path, [] as [(&str, &str); 0], [(name, "0x0")], "")?;
        let p = path.unwrap_or_else(|| Path::new(&provided_name));
        let mut w = std::fs::File::create(
            p.join(SourcePackageLayout::Sources.path())
                .join(format!("{name}.move")),
        )?;
        writeln!(
            w,
            r#"/*
/// Module: {name}
module {name}::{name};
*/

// For Move coding conventions, see
// https://docs.iota.org/developer/iota-101/move-overview/conventions

"#
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
module {name}::{name}_tests;
// uncomment this line to import the module
// use {name}::{name};

const ENotImplemented: u64 = 0;

#[test]
fun test_{name}() {{
    // pass
}}

#[test, expected_failure(abort_code = ::{name}::{name}_tests::ENotImplemented)]
fun test_{name}_fail() {{
    abort ENotImplemented
}}
*/"#
        )?;

        Ok(())
    }
}
