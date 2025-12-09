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

fun id(self: &TestAccount): &UID {
    &self.id
}

#[test]
fun authenticator_info_v1_account_happy_path() {
    account_test!(|scenario, account| {
        let default_authenticator_info = create_default_authenticator_info_v1_for_testing();

        // Check that there is no an attached `AuthenticatorInfoV1` just after creation.
        assert_eq(account::has_auth_info_v1(account.id()), false);

        // Create an account as a shared object with an attached `AuthenticatorInfoV1` instance.
        account::create_account_v1(account, default_authenticator_info);

        scenario.next_tx(@0x0);

        let mut account = scenario.take_shared<TestAccount>();

        assert_eq(account::has_auth_info_v1(account.id()), true);
        assert_ref_eq(account::borrow_auth_info_v1(account.id()), &default_authenticator_info);

        // Rotate the `AuthenticatorInfoV1` instance.
        let updated_authenticator_info = account::create_auth_info_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );
        let previous_authenticator_info = account::rotate_auth_info_v1(
            &mut account,
            updated_authenticator_info,
        );

        assert_eq(previous_authenticator_info, default_authenticator_info);

        assert_eq(account::has_auth_info_v1(account.id()), true);
        assert_ref_eq(account::borrow_auth_info_v1(account.id()), &updated_authenticator_info);

        test_scenario::return_shared(account);
    });
}

#[test]
fun authenticator_info_v1_immutable_account_happy_path() {
    account_test!(|scenario, account| {
        let default_authenticator_info = create_default_authenticator_info_v1_for_testing();

        // Check that there is no an attached `AuthenticatorInfoV1` just after creation.
        assert_eq(account::has_auth_info_v1(account.id()), false);

        // Create an immutable account with an attached `AuthenticatorInfoV1` instance.
        account::create_immutable_account_v1(account, default_authenticator_info);

        scenario.next_tx(@0x0);

        let account = scenario.take_immutable<TestAccount>();

        assert_eq(account::has_auth_info_v1(account.id()), true);
        assert_ref_eq(account::borrow_auth_info_v1(account.id()), &default_authenticator_info);

        test_scenario::return_immutable(account);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1AlreadyAttached)]
fun authenticator_info_v1_double_account_creation() {
    account_test!(|scenario, account| {
        let authenticator_info_1 = create_default_authenticator_info_v1_for_testing();
        let authenticator_info_2 = account::create_auth_info_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );

        account::create_account_v1(account, authenticator_info_1);

        scenario.next_tx(@0x0);

        let account = scenario.take_shared<TestAccount>();

        // Call `account::create_account_v1` one more time for the same object that is forbidden.
        account::create_account_v1(account, authenticator_info_2);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1AlreadyAttached)]
fun authenticator_info_v1_double_immutable_account_creation() {
    account_test!(|scenario, account| {
        let authenticator_info_1 = create_default_authenticator_info_v1_for_testing();
        let authenticator_info_2 = account::create_auth_info_v1_for_testing(
            @0x2,
            ascii::string(b"module2"),
            ascii::string(b"function2"),
        );

        account::create_immutable_account_v1(account, authenticator_info_1);

        scenario.next_tx(@0x0);

        let account = scenario.take_immutable<TestAccount>();
        // Call `account::create_account_v1` one more time for the same object that is forbidden.
        account::create_immutable_account_v1(account, authenticator_info_2);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_borrow_non_attached() {
    account_test!(|_, account| {
        // Borrow a non-attached `AuthenticatorInfoV1` instance.
        account::borrow_auth_info_v1<TestAccount>(account.id());

        test_utils::destroy(account);
    });
}

#[test]
#[expected_failure(abort_code = account::EAuthenticatorInfoV1NotAttached)]
fun authenticator_info_v1_rotate_non_attached() {
    account_test!(|_, mut account| {
        let authenticator_info = create_default_authenticator_info_v1_for_testing();

        account::rotate_auth_info_v1(&mut account, authenticator_info);

        test_utils::destroy(account);
    });
}

fun create_test_account(scenario: &mut Scenario): TestAccount {
    TestAccount { id: object::new(test_scenario::ctx(scenario)) }
}

fun create_default_authenticator_info_v1_for_testing(): AuthenticatorInfoV1<TestAccount> {
    account::create_auth_info_v1_for_testing(
        @0x1,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

macro fun account_test($f: |&mut Scenario, TestAccount|) {
    let mut scenario_val = test_scenario::begin(@0x0);
    let scenario = &mut scenario_val;
    let account = create_test_account(scenario);

    $f(scenario, account);

    test_scenario::end(scenario_val);
}
