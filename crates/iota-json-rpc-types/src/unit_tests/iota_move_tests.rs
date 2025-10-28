// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_enum_compat_util::*;
use iota_types::IOTA_SYSTEM_PACKAGE_ID;

use crate::{IotaMoveStruct, IotaMoveValue, MoveFunctionName};

#[test]
fn enforce_order_test() {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "staged", "iota_move_struct.yaml"]);
    check_enum_compat_order::<IotaMoveStruct>(path);

    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "staged", "iota_move_value.yaml"]);
    check_enum_compat_order::<IotaMoveValue>(path);
}

#[test]
fn parse_move_function_name() {
    let name = "0x03::wat::call";
    let parsed: MoveFunctionName = name.parse().unwrap();
    assert_eq!(parsed.package, IOTA_SYSTEM_PACKAGE_ID);
    assert_eq!(parsed.module.as_str(), "wat");
    assert_eq!(parsed.function.as_str(), "call");
}

#[test]
fn parse_move_function_name_unsupported_pkg_address() {
    let name = "namedpackage::wat::call";
    let parsed: Result<MoveFunctionName, _> = name.parse();
    assert!(parsed.is_err());
}

#[test]
fn parse_move_function_name_non_ascii_mod() {
    let name = "0x03::βατ::call";
    let parsed: Result<MoveFunctionName, _> = name.parse();
    assert!(parsed.is_err());
}

#[test]
fn parse_move_function_name_non_ascii_fun() {
    let name = "0x03::wat::βατ";
    let parsed: Result<MoveFunctionName, _> = name.parse();
    assert!(parsed.is_err());
}
