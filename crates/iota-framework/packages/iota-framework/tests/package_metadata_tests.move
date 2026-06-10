// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::package_metadata_tests;

use iota::package_metadata::{
    create_package_metadata_v1_for_testing,
    create_package_metadata_v1_with_dynamic_metadata_for_testing
};
use iota::test_utils::{Self, assert_eq, assert_ref_eq};
use std::ascii;
use std::type_name;

#[test]
fun package_metadata_v1_happy_path() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let account_type = type_name::get<u64>();

    let package_metadata_v1 = create_package_metadata_v1_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
    );

    let module_metadata = package_metadata_v1.modules_metadata_v1(&module_name);
    let authenticator_metadata = module_metadata.authenticator_metadata_v1(&auth_function_name);
    assert_eq(authenticator_metadata.account_type(), account_type);
    assert_ref_eq(authenticator_metadata.function_name(), &auth_function_name);

    test_utils::destroy(package_metadata_v1);
}

#[test]
fun package_metadata_dynamic_view_functions_happy_path() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let view_function_name = ascii::string(b"view_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_with_dynamic_metadata_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
        vector[vector[view_function_name]],
    );

    let module_metadata = package_metadata.modules_metadata(&module_name);
    assert_eq(module_metadata.borrow_view_functions_metadata_v1().length(), 1);
    let view_function_metadata = module_metadata.borrow_view_function_metadata_v1(
        &view_function_name,
    );
    assert_ref_eq(view_function_metadata, &view_function_name);

    test_utils::destroy(package_metadata);
}

#[test]
// Regression test: a package with more than one metadata-bearing module must
// produce a distinct `ModuleMetadata` per module (no derived-address collision).
fun package_metadata_dynamic_multi_module() {
    let id = object::id_from_address(@0xA);
    let module_a = ascii::string(b"module_a");
    let module_b = ascii::string(b"module_b");
    let auth_a = ascii::string(b"auth_a");
    let auth_b = ascii::string(b"auth_b");
    let view_a = ascii::string(b"view_a");
    let view_b = ascii::string(b"view_b");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_with_dynamic_metadata_for_testing(
        id,
        vector[module_a, module_b],
        vector[vector[auth_a], vector[auth_b]],
        vector[vector[account_type], vector[account_type]],
        vector[vector[view_a], vector[view_b]],
    );

    let md_a = package_metadata.modules_metadata(&module_a);
    assert_ref_eq(md_a.borrow_view_function_metadata_v1(&view_a), &view_a);
    let md_b = package_metadata.modules_metadata(&module_b);
    assert_ref_eq(md_b.borrow_view_function_metadata_v1(&view_b), &view_b);

    test_utils::destroy(package_metadata);
}
