// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

#[allow(unused_variables)]
fn run_all(args_path: &Path) -> datatest_stable::Result<()> {
    #[cfg(feature = "tracing")]
    {
        use std::path::PathBuf;

        use move_cli::sandbox::commands::test;
        let cli_exe = env!("CARGO_BIN_EXE_move");
        let use_temp_dir = !args_path.parent().unwrap().join("NO_TEMPDIR").exists();
        test::run_one(
            args_path,
            &PathBuf::from(cli_exe),
            use_temp_dir,
            false, // track_cov
        )?;
    }
    Ok(())
}

// runs all the tests
datatest_stable::harness!(run_all, "tests/tracing_tests", r"args\.txt$");
