// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::package_metadata_tests;

use iota::package_metadata::create_package_metadata_v2_for_testing;
use iota::test_utils::{Self, assert_eq, assert_ref_eq};
use std::ascii;

#[test]
fun view_functions_metadata_happy_path() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let view_function_name = ascii::string(b"view_function");

    let package_metadata_v2 = create_package_metadata_v2_for_testing(
        id,
        vector[module_name],
        vector[vector[]],
        vector[vector[]],
        vector[vector[view_function_name]],
    );

    let module_metadata = package_metadata_v2.modules_metadata_v2(&module_name);

    let view_function_metadata = module_metadata.view_function_metadata(&view_function_name);

    assert_eq(module_metadata.view_functions_metadata().length(), 1);
    assert_ref_eq(view_function_metadata.view_function_name(), &view_function_name);

    test_utils::destroy(package_metadata_v2);
}
