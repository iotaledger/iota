// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf};

use insta::assert_snapshot;
use iota_mvr_graphql_rpc::server::builder::export_schema;

#[test]
fn test_schema_sdl_export() {
    let sdl = export_schema();

    let file = if cfg!(feature = "staging") {
        "staging.graphql"
    } else {
        "schema.graphql"
    };

    // update the current schema file
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
    fs::write(path, &sdl).unwrap();

    assert_snapshot!(file, sdl);
}
