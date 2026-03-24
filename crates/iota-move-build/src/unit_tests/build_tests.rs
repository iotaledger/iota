// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use move_compiler::editions::Edition;

use crate::BuildConfig;

fn unit_test_data_path(test_package: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("unit_tests")
        .join("data")
        .join(test_package)
}

#[test]
fn generate_struct_layouts() {
    // build the IOTA framework and generate struct layouts to make sure nothing
    // crashes
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
        .join("iota-framework")
        .join("packages")
        .join("iota-framework");
    let pkg = BuildConfig::new_for_testing().build(&path).unwrap();
    let registry = pkg.generate_struct_layouts();
    // check for a couple of types that aren't likely to go away
    assert!(registry.contains_key(
        "0000000000000000000000000000000000000000000000000000000000000001::string::String"
    ));
    assert!(registry.contains_key(
        "0000000000000000000000000000000000000000000000000000000000000002::object::UID"
    ));
    assert!(registry.contains_key(
        "0000000000000000000000000000000000000000000000000000000000000002::tx_context::TxContext"
    ));
}

#[test]
fn development_mode_not_allowed() {
    let path = unit_test_data_path("no_development_mode");
    let err = BuildConfig::new_for_testing()
        .build(&path)
        .expect_err("Should have failed due to unsupported edition");
    assert!(
        err.to_string()
            .contains(&Edition::DEVELOPMENT.unknown_edition_error().to_string())
    );
}

#[test]
fn struct_fields_at_limit_is_allowed() {
    let path = unit_test_data_path("struct_fields_limit_ok");
    let mut config = BuildConfig::new_for_testing();
    config.config.max_fields_in_struct = Some(32);
    config
        .build(&path)
        .expect("Build should succeed when struct has exactly 32 fields");
}

#[test]
fn struct_fields_above_limit_fails_at_compile_time() {
    let path = unit_test_data_path("struct_fields_limit_exceeded");
    let mut config = BuildConfig::new_for_testing();
    config.config.max_fields_in_struct = Some(32);
    let err = config
        .build(&path)
        .expect_err("Build should fail when struct has more than 32 fields");
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("Compilation error"),
        "expected build failure due to compiler error, got: {err_msg}"
    );
}
