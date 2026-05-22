// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::package_metadata_tests;

use iota::package_metadata;
use iota::test_utils::{Self, assert_eq, assert_ref_eq};
use std::ascii;

#[test]
fun borrow_package_view_functions_metadata_happy_path() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let view_function = ascii::string(b"view_function");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_with_view_functions(
        package,
        module_name,
        vector[view_function],
    );

    let view_metadata = package_metadata::borrow_package_view_functions_metadata(&metadata);
    let module_view_functions = package_metadata::module_view_functions(
        view_metadata,
        &module_name,
    );

    assert_eq(module_view_functions.length(), 1);
    assert_ref_eq(&module_view_functions[0], &view_function);

    let package_view_functions = package_metadata::view_functions(view_metadata);
    assert_eq(package_view_functions.size(), 1);
    assert_ref_eq(package_view_functions.get(&module_name), &vector[view_function]);

    test_utils::destroy(metadata)
}
