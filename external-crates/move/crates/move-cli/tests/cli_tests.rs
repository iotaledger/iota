// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(feature = "tracing")]
#[test]
fn run_metatest() {
    use std::{env, path::PathBuf};

    use move_cli::sandbox::commands::test;

    let path_cli_binary = PathBuf::from(env!("CARGO_BIN_EXE_move"));
    let path_metatest = PathBuf::from("tests/metatests/args.txt");

    // local workspace + with coverage
    assert!(test::run_all(&path_metatest, path_cli_binary.as_path(), false, true).is_ok());

    // temp workspace + with coverage
    assert!(test::run_all(&path_metatest, &path_cli_binary, true, true).is_ok());

    // local workspace + without coverage
    assert!(test::run_all(&path_metatest, &path_cli_binary, false, false).is_ok());

    // temp workspace + without coverage
    assert!(test::run_all(&path_metatest, &path_cli_binary, true, false).is_ok());
}
