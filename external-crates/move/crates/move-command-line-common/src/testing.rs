// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::env::read_bool_env_var;

/// Extension for raw output files
pub const OUT_EXT: &str = "out";
/// Extension for expected output files
pub const EXP_EXT: &str = "exp";

/// If any of these env vars is set, the test harness should overwrite
/// the existing .exp files with the output instead of checking
/// them against the output.
pub const UPDATE_BASELINE: &str = "UPDATE_BASELINE";
pub const UPBL: &str = "UPBL";
pub const UB: &str = "UB";

pub const PRETTY: &str = "PRETTY";
pub const FILTER: &str = "FILTER";

pub fn read_env_update_baseline() -> bool {
    read_bool_env_var(UPDATE_BASELINE) || read_bool_env_var(UPBL) || read_bool_env_var(UB)
}

pub fn add_update_baseline_fix(s: impl AsRef<str>) -> String {
    format!(
        "{}\n\
        Run with `env {}=1` (or `env {}=1`) to save the current output as \
        the new expected output",
        s.as_ref(),
        UB,
        UPDATE_BASELINE
    )
}

pub fn format_diff(expected: impl AsRef<str>, actual: impl AsRef<str>) -> String {
    use colored::Colorize;
    use similar::ChangeTag;
    let diff = similar::TextDiff::from_lines(expected.as_ref(), actual.as_ref());

    diff.iter_all_changes()
        .map(|change| match change.tag() {
            ChangeTag::Delete => format!("{}{}", "-".bold(), change.value()).red(),
            ChangeTag::Insert => format!("{}{}", "+".bold(), change.value()).green(),
            ChangeTag::Equal => change.value().dimmed(),
        })
        .map(|s| s.to_string())
        .collect()
}
