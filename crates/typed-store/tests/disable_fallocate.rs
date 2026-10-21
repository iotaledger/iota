// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! In its own test binary, because `disable_fallocate` affects the whole
//! process.

use typed_store::rocks::{MetricConf, default_db_options, disable_fallocate, open_cf_opts};

/// `allow_fallocate` stays on by default, and databases opened after
/// `disable_fallocate` have it turned off.
#[tokio::test]
async fn disable_fallocate_turns_off_allow_fallocate() {
    assert_eq!(allow_fallocate_of_new_db(), "true");
    disable_fallocate();
    assert_eq!(allow_fallocate_of_new_db(), "false");
}

/// Opens a new database and reads `allow_fallocate` from its OPTIONS file.
fn allow_fallocate_of_new_db() -> String {
    let tmp_dir = iota_common::tempdir();
    let _db = open_cf_opts(
        tmp_dir.path(),
        None,
        MetricConf::default(),
        &[("cf", default_db_options().options)],
    )
    .unwrap();

    let options_file = std::fs::read_dir(tmp_dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("OPTIONS-")
        })
        .max()
        .unwrap();
    std::fs::read_to_string(options_file)
        .unwrap()
        .lines()
        .find_map(|line| line.trim().strip_prefix("allow_fallocate="))
        .unwrap()
        .to_owned()
}
