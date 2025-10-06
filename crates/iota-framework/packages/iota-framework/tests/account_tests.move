// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::account_tests;

use iota::account::{Self, AuthenticatorInfoV1};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{Self, assert_eq, assert_ref_eq};
use std::ascii;

// This struct is used as an account for testing.
public struct TestAccount has key {
    id: UID,
}

#[test]
fun authenticator_info_v1_happy_path() {
    account_test_mut!(|_, account_id| {
        let default_authenticator_info = create_default_authenticator_info_v1_for_testing();

        // Check that there is no an attached `AuthenticatorInfoV1` just after creation.
        assert_eq(account::has_auth_info_v1(account_id), false);

        // Attach an `AuthenticatorInfoV1` instance to the account.
        account::attach_auth_info_v1(account_id, default_authenticator_info);

        assert_eq(account::has_auth_info_v1(account_id), true);
        assert_ref_eq(account::borrow_auth_info_v1(account_id), &default_authenticator_info);

        // Rotate the `AuthenticatorInfoV1` instance.
        let updated_authenticator_info = account::create_auth_info_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );

        let previous_authenticator_info = account::rotate_auth_info_v1(
            account_id,
            updated_authenticator_info,
        );

        assert_eq(previous_authenticator_info, default_authenticator_info);

        assert_eq(account::has_auth_info_v1(account_id), true);
        assert_ref_eq(account::borrow_auth_info_v1(account_id), &updated_authenticator_info);

        // Detach the `AuthenticatorInfoV1` instance from the account.
        let detached_authenticator_info = account::detach_auth_info_v1(account_id);

        assert_eq(detached_authenticator_info, updated_authenticator_info);

        assert_eq(account::has_auth_info_v1(account_id), false);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1AlreadyAttached)]
fun authenticator_info_v1_double_attach() {
    account_test_mut!(|_, account_id| {
        let authenticator_info_1 = create_default_authenticator_info_v1_for_testing();
        let authenticator_info_2 = account::create_auth_info_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );

        account::attach_auth_info_v1(account_id, authenticator_info_1);
        // Attach another `AuthenticatorInfoV1` instance that is forbidden.
        account::attach_auth_info_v1(account_id, authenticator_info_2);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_detach_non_existent() {
    account_test_mut!(|_, account_id| {
        // Detach a non-existing `AuthenticatorInfoV1` instance.
        account::detach_auth_info_v1(account_id);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_double_detach() {
    account_test_mut!(|_, account_id| {
        let authenticator_info = create_default_authenticator_info_v1_for_testing();

        account::attach_auth_info_v1(account_id, authenticator_info);
        account::detach_auth_info_v1(account_id);

        // Detach a non-existing `AuthenticatorInfoV1` instance.
        account::detach_auth_info_v1(account_id);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_borrow_non_existent() {
    account_test!(|_, account_id| {
        // Borrow a non-existing `AuthenticatorInfoV1` instance.
        account::borrow_auth_info_v1(account_id);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_borrow_after_detach() {
    account_test_mut!(|_, account_id| {
        let authenticator_info = create_default_authenticator_info_v1_for_testing();

        account::attach_auth_info_v1(account_id, authenticator_info);
        account::detach_auth_info_v1(account_id);

        // Borrow a non-existing `AuthenticatorInfoV1` instance.
        account::borrow_auth_info_v1(account_id);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_rotate_non_existent() {
    account_test_mut!(|_, account_id| {
        let authenticator_info = create_default_authenticator_info_v1_for_testing();

        // Rotate a non-existing `AuthenticatorInfoV1` instance.
        account::rotate_auth_info_v1(account_id, authenticator_info);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_rotate_after_detach() {
    account_test_mut!(|_, account_id| {
        let authenticator_info = create_default_authenticator_info_v1_for_testing();

        account::attach_auth_info_v1(account_id, authenticator_info);
        account::detach_auth_info_v1(account_id);

        // Rotate a non-existing `AuthenticatorInfoV1` instance.
        account::rotate_auth_info_v1(account_id, authenticator_info);
    });
}

fun create_test_account(scenario: &mut Scenario): TestAccount {
    TestAccount { id: object::new(test_scenario::ctx(scenario)) }
}

fun create_default_authenticator_info_v1_for_testing(): AuthenticatorInfoV1 {
    account::create_auth_info_v1_for_testing(
        @0x1,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

macro fun account_test($f: |&mut Scenario, &UID|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account = create_test_account(scenario);

    $f(scenario, &account.id);

    test_utils::destroy(account);

    test_scenario::end(scenario_val);
}

macro fun account_test_mut($f: |&mut Scenario, &mut UID|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let mut account = create_test_account(scenario);

    $f(scenario, &mut account.id);

    test_utils::destroy(account);

    test_scenario::end(scenario_val);
}
