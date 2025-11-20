// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::account_tests;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::package_metadata;
use iota::test_scenario;
use iota::test_utils::{Self, assert_eq, assert_ref_eq};
use std::ascii;
use std::type_name::{Self, TypeName};

// This struct is used as an account for testing.
public struct TestAccount has key {
    id: UID,
}

fun id(self: &TestAccount): &UID {
    &self.id
}

fun id_mut(self: &mut TestAccount): &mut UID {
    &mut self.id
}

#[test]
fun authenticator_info_v1_happy_path() {
    account_test_mut!(|account, _| {
        let default_authenticator_info = create_default_authenticator_info_v1_for_testing();
        let default_package_metadata = create_default_package_metadata_for_testing();

        // Check that there is no an attached `AuthenticatorInfoV1` just after creation.
        assert_eq(account::has_auth_info_v1(account.id()), false);

        // Attach an `AuthenticatorInfoV1` instance to the account.
        let compatibility_proof = account::check_auth_info_v1_compatibility(
            account,
            &default_package_metadata,
            default_module_name(),
            default_function_name(),
        );
        account::attach_auth_info_v1(account.id_mut(), compatibility_proof);

        assert_eq(account::has_auth_info_v1(account.id()), true);
        assert_ref_eq(account::borrow_auth_info_v1(account.id()), &default_authenticator_info);

        // Rotate the `AuthenticatorInfoV1` instance.
        let updated_authenticator_info = account::create_auth_info_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );
        let updated_package_metadata = create_package_metadata_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
            default_account_type(),
        );

        let compatibility_proof = account::check_auth_info_v1_compatibility(
            account,
            &updated_package_metadata,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );
        account::rotate_auth_info_v1(
            account.id_mut(),
            compatibility_proof,
        );

        assert_eq(account::has_auth_info_v1(account.id()), true);
        assert_ref_eq(account::borrow_auth_info_v1(account.id()), &updated_authenticator_info);

        test_utils::destroy(default_package_metadata);
        test_utils::destroy(updated_package_metadata);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1AlreadyAttached)]
fun authenticator_info_v1_double_attach() {
    account_test_mut!(|account, _| {
        let package_metadata_1 = create_default_package_metadata_for_testing();
        let package_metadata_2 = create_package_metadata_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
            default_account_type(),
        );

        let compatibility_proof_1 = account::check_auth_info_v1_compatibility(
            account,
            &package_metadata_1,
            default_module_name(),
            default_function_name(),
        );
        account::attach_auth_info_v1(account.id_mut(), compatibility_proof_1);
        // Attach another `AuthenticatorInfoV1` instance that is forbidden.
        let compatibility_proof_2 = account::check_auth_info_v1_compatibility(
            account,
            &package_metadata_2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );
        account::attach_auth_info_v1(account.id_mut(), compatibility_proof_2);

        test_utils::destroy(package_metadata_1);
        test_utils::destroy(package_metadata_2);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1CompatibilityNotProven)]
fun authenticator_info_v1_not_proven_attach() {
    account_test_mut!(|account, ctx| {
        let package_metadata = create_default_package_metadata_for_testing();

        let account_2 = create_test_account(ctx);
        let compatibility_proof = account::check_auth_info_v1_compatibility(
            &account_2,
            &package_metadata,
            default_module_name(),
            default_function_name(),
        );
        // Attach a not proven `AuthenticatorInfoV1` instance.
        account::attach_auth_info_v1(account.id_mut(), compatibility_proof);

        test_utils::destroy(package_metadata);
        test_utils::destroy(account_2);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_borrow_non_existent() {
    account_test!(|account_id, _| {
        // Borrow a non-existing `AuthenticatorInfoV1` instance.
        account::borrow_auth_info_v1(account_id);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_rotate_non_existent() {
    account_test_mut!(|account, _| {
        let package_metadata = create_default_package_metadata_for_testing();

        let compatibility_proof = account::check_auth_info_v1_compatibility(
            account,
            &package_metadata,
            default_module_name(),
            default_function_name(),
        );
        account::rotate_auth_info_v1(account.id_mut(), compatibility_proof);

        test_utils::destroy(package_metadata);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1CompatibilityNotProven)]
fun authenticator_info_v1_rotate_not_proven() {
    account_test_mut!(|account, ctx| {
        let package_metadata = create_default_package_metadata_for_testing();

        let compatibility_proof = account::check_auth_info_v1_compatibility(
            account,
            &package_metadata,
            default_module_name(),
            default_function_name(),
        );
        account::attach_auth_info_v1(account.id_mut(), compatibility_proof);

        let account_2 = create_test_account(ctx);
        let compatibility_proof = account::check_auth_info_v1_compatibility(
            &account_2,
            &package_metadata,
            default_module_name(),
            default_function_name(),
        );
        // Rotate a not proven `AuthenticatorInfoV1` instance.
        account::rotate_auth_info_v1(account.id_mut(), compatibility_proof);

        test_utils::destroy(package_metadata);
        test_utils::destroy(account_2);
    });
}

fun create_test_account(ctx: &mut TxContext): TestAccount {
    TestAccount { id: object::new(ctx) }
}

fun default_package(): address {
    @0x1
}

fun default_module_name(): ascii::String {
    ascii::string(b"module")
}

fun default_function_name(): ascii::String {
    ascii::string(b"function")
}

fun default_account_type(): TypeName {
    type_name::get<TestAccount>()
}

fun create_default_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    account::create_auth_info_v1_for_testing(
        default_package(),
        default_module_name(),
        default_function_name(),
    )
}

fun create_package_metadata_for_testing(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
    type_name: TypeName,
): package_metadata::PackageMetadataV1 {
    package_metadata::create_package_metadata_v1_for_testing_one_authenticator(
        package.to_id(),
        module_name,
        function_name,
        type_name,
    )
}

fun create_default_package_metadata_for_testing(): package_metadata::PackageMetadataV1 {
    create_package_metadata_for_testing(
        default_package(),
        default_module_name(),
        default_function_name(),
        default_account_type(),
    )
}

macro fun account_test($f: |&UID, &mut TxContext|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let account = create_test_account(ctx);

    $f(&account.id, ctx);

    test_utils::destroy(account);

    test_scenario::end(scenario_val);
}

macro fun account_test_mut($f: |&mut TestAccount, &mut TxContext|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let ctx = scenario.ctx();
    let mut account = create_test_account(ctx);

    $f(&mut account, ctx);

    test_utils::destroy(account);

    test_scenario::end(scenario_val);
}
