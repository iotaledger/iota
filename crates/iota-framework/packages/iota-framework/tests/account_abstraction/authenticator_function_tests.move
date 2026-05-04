// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::authenticator_function_tests;

use iota::authenticator_function;
use iota::package_metadata;
use iota::test_utils::{Self, assert_eq, assert_ref_eq};
use std::ascii;
use std::type_name;

// These structs are used as accounts for testing.
public struct TestAccount has key {
    id: UID,
}

public struct TestAccount2 has key {
    id: UID,
}

#[test]
fun authenticator_function_ref_v1_create_happy_path() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"authenticate");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_one_authenticator(
        package,
        module_name,
        function_name,
        type_name::get<TestAccount>(),
    );

    let auth_function_ref = authenticator_function::create_auth_function_ref_v1<TestAccount>(
        &metadata,
        module_name,
        function_name,
    );

    assert_eq(auth_function_ref.package(), package);
    assert_ref_eq(auth_function_ref.module_name(), &module_name);
    assert_ref_eq(auth_function_ref.function_name(), &function_name);

    test_utils::destroy(metadata)
}

#[test]
fun package_metadata_v1_view_function_happy_path() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"get_value");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_one_view_function(
        package,
        module_name,
        function_name,
    );

    let module_metadata = metadata.modules_metadata_v1(&module_name);
    assert!(module_metadata.is_view_function_v1(&function_name));
    let view_metadata = module_metadata.view_function_metadata_v1(&function_name);
    assert_eq(view_metadata.view_function_name(), function_name);

    test_utils::destroy(metadata)
}

#[test]
#[expected_failure(abort_code = package_metadata::EViewFunctionMetadataNotFound)]
fun package_metadata_v1_view_function_unknown_function_name() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"get_value");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_one_view_function(
        package,
        module_name,
        function_name,
    );

    metadata
        .modules_metadata_v1(&module_name)
        .view_function_metadata_v1(&ascii::string(b"get_other_value"));

    test_utils::destroy(metadata)
}

#[test]
#[expected_failure(abort_code = package_metadata::EModuleMetadataNotFound)]
fun authenticator_function_ref_v1_create_with_unknown_module_name() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"authenticate");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_one_authenticator(
        package,
        module_name,
        function_name,
        type_name::get<TestAccount>(),
    );

    authenticator_function::create_auth_function_ref_v1<TestAccount>(
        &metadata,
        ascii::string(b"module2"),
        function_name,
    );

    test_utils::destroy(metadata)
}

#[test]
#[expected_failure(abort_code = package_metadata::EAuthenticatorMetadataNotFound)]
fun authenticator_function_ref_v1_create_with_unknown_function_name() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"authenticate");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_one_authenticator(
        package,
        module_name,
        function_name,
        type_name::get<TestAccount>(),
    );

    authenticator_function::create_auth_function_ref_v1<TestAccount>(
        &metadata,
        module_name,
        ascii::string(b"authenticate2"),
    );

    test_utils::destroy(metadata)
}

#[test]
#[
    expected_failure(
        abort_code = authenticator_function::EAuthenticatorFunctionRefV1NotCompatibleWithAccount,
    ),
]
fun authenticator_function_ref_v1_create_with_wrong_account_type() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"authenticate");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_one_authenticator(
        package,
        module_name,
        function_name,
        type_name::get<TestAccount>(),
    );

    authenticator_function::create_auth_function_ref_v1<TestAccount2>(
        &metadata,
        module_name,
        function_name,
    );

    test_utils::destroy(metadata)
}
