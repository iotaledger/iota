// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::package_metadata_tests;

use iota::package_metadata::{
    Self,
    create_package_metadata_v1_for_testing,
    create_package_metadata_v1_with_dynamic_metadata_for_testing
};
use iota::test_utils::{Self, assert_eq, assert_ref_eq};
use std::ascii;
use std::type_name;

#[test, allow(deprecated_usage)]
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

    let module_metadata = package_metadata.module_metadata(&module_name);
    assert_eq(module_metadata.borrow_view_functions_metadata_v1().length(), 1);
    let is_view_function_metadata = module_metadata.is_view_function_v1(
        &view_function_name,
    );
    assert_eq(is_view_function_metadata, true);

    test_utils::destroy(package_metadata);
}

#[test, allow(deprecated_usage)]
// Regression test: `try_get_modules_metadata_v1` must return `none` (not
// abort) for a module without metadata, in both the inline and the
// dynamic-field layouts.
fun try_get_modules_metadata_v1_missing_module_returns_none() {
    let module_name = ascii::string(b"module");
    let missing_module_name = ascii::string(b"missing_module");
    let auth_function_name = ascii::string(b"auth_function");
    let account_type = type_name::get<u64>();

    let inline_metadata = create_package_metadata_v1_for_testing(
        object::id_from_address(@0xA),
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
    );
    assert!(inline_metadata.try_get_modules_metadata_v1(&missing_module_name).is_none());
    assert!(inline_metadata.try_get_modules_metadata_v1(&module_name).is_some());
    test_utils::destroy(inline_metadata);

    let dynamic_metadata = create_package_metadata_v1_with_dynamic_metadata_for_testing(
        object::id_from_address(@0xB),
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
        vector[vector<ascii::String>[]],
    );
    assert!(dynamic_metadata.try_get_modules_metadata_v1(&missing_module_name).is_none());
    assert!(dynamic_metadata.try_get_modules_metadata_v1(&module_name).is_some());
    test_utils::destroy(dynamic_metadata);
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

    let md_a = package_metadata.module_metadata(&module_a);
    assert_eq(md_a.is_view_function_v1(&view_a), true);
    let md_b = package_metadata.module_metadata(&module_b);
    assert_eq(md_b.is_view_function_v1(&view_b), true);

    test_utils::destroy(package_metadata);
}

#[test]
fun package_metadata_getters() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_with_dynamic_metadata_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
        vector[vector<ascii::String>[]],
    );

    // The dynamic-metadata test builder uses `storage_id` as the runtime id and
    // pins the package version to 1.
    assert_eq(package_metadata.storage_id(), id);
    assert_eq(package_metadata.runtime_id(), id);
    assert_eq(package_metadata.package_version(), 1);
    assert_eq(*package_metadata.borrow_package_metadata_version_field(), 2);

    test_utils::destroy(package_metadata);
}

#[test]
// Exercises the dynamic-field authenticator accessors (the layout this change
// introduces), as opposed to the deprecated `*_v1` path covered above.
fun dynamic_authenticator_accessors_happy_path() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_with_dynamic_metadata_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
        vector[vector<ascii::String>[]],
    );

    let from_package = package_metadata.module_authenticator_function_metadata_v1(
        &module_name,
        &auth_function_name,
    );
    assert_eq(from_package.account_type(), account_type);
    assert_ref_eq(from_package.function_name(), &auth_function_name);

    let module_metadata = package_metadata.module_metadata(&module_name);
    let from_module = package_metadata::authenticator_function_metadata_v1(
        module_metadata,
        &auth_function_name,
    );
    assert_eq(from_module.account_type(), account_type);
    assert_ref_eq(from_module.function_name(), &auth_function_name);

    test_utils::destroy(package_metadata);
}

#[test]
fun try_get_authenticator_function_metadata_v1_present_and_missing() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let missing_function_name = ascii::string(b"missing_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_with_dynamic_metadata_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
        vector[vector<ascii::String>[]],
    );

    let module_metadata = package_metadata.module_metadata(&module_name);
    let found = package_metadata::try_get_authenticator_function_metadata_v1(
        module_metadata,
        &auth_function_name,
    );
    assert!(found.is_some());
    assert_eq(found.borrow().account_type(), account_type);
    assert!(
        package_metadata::try_get_authenticator_function_metadata_v1(
            module_metadata,
            &missing_function_name,
        ).is_none(),
    );

    test_utils::destroy(package_metadata);
}

#[test, allow(deprecated_usage)]
fun try_get_authenticator_metadata_v1_present_and_missing() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let missing_function_name = ascii::string(b"missing_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
    );

    let module_metadata_v1 = package_metadata.modules_metadata_v1(&module_name);
    assert!(module_metadata_v1.try_get_authenticator_metadata_v1(&auth_function_name).is_some());
    assert!(module_metadata_v1.try_get_authenticator_metadata_v1(&missing_function_name).is_none());

    test_utils::destroy(package_metadata);
}

// === Abort behaviour ===

#[test]
#[expected_failure(abort_code = iota::package_metadata::EWrongPackageVersion)]
// `module_metadata` requires the dynamic-field layout; the legacy inline layout
// has no package-metadata version field and must abort.
fun module_metadata_on_legacy_layout_aborts() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
    );

    package_metadata.module_metadata(&module_name);
    test_utils::destroy(package_metadata);
}

#[test]
#[expected_failure(abort_code = iota::package_metadata::EModuleMetadataNotFound)]
fun module_metadata_missing_module_aborts() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let missing_module_name = ascii::string(b"missing_module");
    let auth_function_name = ascii::string(b"auth_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_with_dynamic_metadata_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
        vector[vector<ascii::String>[]],
    );

    package_metadata.module_metadata(&missing_module_name);
    test_utils::destroy(package_metadata);
}

#[test, allow(deprecated_usage)]
#[expected_failure(abort_code = iota::package_metadata::EAuthenticatorMetadataNotFound)]
fun authenticator_metadata_v1_missing_function_aborts() {
    let id = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let auth_function_name = ascii::string(b"auth_function");
    let missing_function_name = ascii::string(b"missing_function");
    let account_type = type_name::get<u64>();

    let package_metadata = create_package_metadata_v1_for_testing(
        id,
        vector[module_name],
        vector[vector[auth_function_name]],
        vector[vector[account_type]],
    );

    let module_metadata_v1 = package_metadata.modules_metadata_v1(&module_name);
    module_metadata_v1.authenticator_metadata_v1(&missing_function_name);
    test_utils::destroy(package_metadata);
}
