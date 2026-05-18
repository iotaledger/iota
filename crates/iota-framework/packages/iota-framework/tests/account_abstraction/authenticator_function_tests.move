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
fun authenticator_function_ref_v1_create_from_package_metadata_v2_happy_path() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"authenticate");
    let view_function_name = ascii::string(b"get_value");

    let metadata = package_metadata::create_package_metadata_v2_for_testing(
        package,
        vector[module_name],
        vector[function_name],
        vector[type_name::get<TestAccount>()],
        vector[module_name],
        vector[view_function_name],
    );

    let auth_function_ref = authenticator_function::create_auth_function_ref_from_package_metadata_v2<
        TestAccount,
    >(
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
fun package_metadata_v2_view_function_happy_path() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"get_value");

    let metadata = package_metadata::create_package_metadata_v2_for_testing_one_view_function(
        package,
        module_name,
        function_name,
    );

    let module_metadata = metadata.modules_metadata_v2(&module_name);
    assert!(module_metadata.is_view_function_v1(&function_name));
    let view_metadata = module_metadata.view_function_metadata_v1(&function_name);
    assert_eq(view_metadata.view_function_name_v1(), function_name);

    test_utils::destroy(metadata)
}

#[test]
fun package_metadata_conversion_preserves_id_and_authenticator_metadata() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"authenticate");

    let metadata = package_metadata::create_package_metadata_v1_for_testing_one_authenticator(
        package,
        module_name,
        function_name,
        type_name::get<TestAccount>(),
    );
    let metadata_id = object::id(&metadata);

    let metadata = package_metadata::package_metadata_v1_to_v2(metadata);
    assert_eq(object::id(&metadata), metadata_id);
    assert_eq(metadata.storage_id_v2(), package);
    let authenticator_metadata = metadata
        .modules_metadata_v2(&module_name)
        .authenticator_metadata_v2(&function_name);
    assert_eq(authenticator_metadata.account_type(), type_name::get<TestAccount>());

    let metadata = package_metadata::package_metadata_v2_to_v1(metadata);
    assert_eq(object::id(&metadata), metadata_id);
    assert_eq(metadata.storage_id(), package);
    let authenticator_metadata = metadata
        .modules_metadata_v1(&module_name)
        .authenticator_metadata_v1(&function_name);
    assert_eq(authenticator_metadata.account_type(), type_name::get<TestAccount>());

    test_utils::destroy(metadata)
}

#[test]
#[expected_failure(abort_code = package_metadata::EViewFunctionMetadataNotEmpty)]
fun package_metadata_v2_to_v1_with_view_metadata_aborts() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"get_value");

    let metadata = package_metadata::create_package_metadata_v2_for_testing_one_view_function(
        package,
        module_name,
        function_name,
    );

    let metadata = package_metadata::package_metadata_v2_to_v1(metadata);
    test_utils::destroy(metadata)
}

#[test]
#[expected_failure(abort_code = package_metadata::EViewFunctionMetadataNotFound)]
fun package_metadata_v2_view_function_unknown_function_name() {
    let package = object::id_from_address(@0xA);
    let module_name = ascii::string(b"module");
    let function_name = ascii::string(b"get_value");

    let metadata = package_metadata::create_package_metadata_v2_for_testing_one_view_function(
        package,
        module_name,
        function_name,
    );

    metadata
        .modules_metadata_v2(&module_name)
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
