// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::smart_account_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::smart_account::{Self, SmartAccount};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

// === builder_v1 ===

#[test]
fun builder_v1_builds_shared_account() {
    let mut scenario = test_scenario::begin(@0x0);

    let authenticator = test_authenticator();
    let addr = smart_account::builder_v1(authenticator, scenario.ctx()).build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();

    assert_eq(account.account_address(), addr);
    assert_ref_eq(account.borrow_auth_function_ref_v1(), &authenticator);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EBuiltinAuthWithoutPublicKey)]
fun build_v1_aborts_for_the_builtin_authenticator_without_a_key() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account::builder_v1(builtin_test_authenticator(), scenario.ctx()).build_v1();

    abort
}

// === with_field ===

#[test]
fun with_field_is_accessible_after_build() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account::builder_v1(test_authenticator(), scenario.ctx())
        .with_field(b"answer", 42u64)
        .build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();

    assert_eq(account.has_field(b"answer"), true);
    assert_ref_eq(account.borrow_field<_, u64>(b"answer"), &42u64);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldAlreadyExists)]
fun with_field_aborts_on_duplicate_name() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account::builder_v1(test_authenticator(), scenario.ctx())
        .with_field(b"key", 1u64)
        .with_field(b"key", 2u64)
        .build_v1();

    scenario.end();
}

// === View functions ===

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldDoesNotExist)]
fun borrow_field_aborts_if_missing() {
    account_test!(|account| {
        account.borrow_field<_, u64>(b"missing");
    });
}

// === Admin: dynamic fields ===

#[test]
fun add_remove_field_lifecycle() {
    account_test_mut!(|account, scenario| {
        account.add_field(b"key", 99u64, scenario.ctx());

        assert_eq(account.has_field(b"key"), true);

        let removed = account.remove_field<_, u64>(b"key", scenario.ctx());

        assert_eq(removed, 99u64);
        assert_eq(account.has_field(b"key"), false);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun add_field_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.add_field(b"key", 0u64, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun remove_field_aborts_if_sender_not_account() {
    let mut scenario = test_scenario::begin(@0x0);
    let addr = make_account(&mut scenario);

    scenario.next_tx(addr);
    let mut account = scenario.take_shared<SmartAccount>();
    account.add_field(b"key", 0u64, scenario.ctx());
    test_scenario::return_shared(account);

    scenario.next_tx(@0x1);
    let mut account = scenario.take_shared<SmartAccount>();
    account.remove_field<_, u64>(b"key", scenario.ctx());

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldDoesNotExist)]
fun remove_field_aborts_if_missing() {
    account_test_mut!(|account, scenario| {
        account.remove_field<_, u64>(b"missing", scenario.ctx());
    });
}

#[test]
fun borrow_field_mut_allows_mutation() {
    account_test_mut!(|account, scenario| {
        account.add_field(b"key", 1u64, scenario.ctx());

        *account.borrow_field_mut<_, u64>(b"key", scenario.ctx()) = 2u64;

        assert_ref_eq(account.borrow_field<_, u64>(b"key"), &2u64);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun borrow_field_mut_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.borrow_field_mut<_, u64>(b"key", scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldDoesNotExist)]
fun borrow_field_mut_aborts_if_missing() {
    account_test_mut!(|account, scenario| {
        account.borrow_field_mut<_, u64>(b"missing", scenario.ctx());
    });
}

#[test]
fun rotate_field_returns_old_and_stores_new() {
    account_test_mut!(|account, scenario| {
        account.add_field(b"key", 1u64, scenario.ctx());

        let old = account.rotate_field<_, u64>(b"key", 2u64, scenario.ctx());

        assert_eq(old, 1u64);
        assert_ref_eq(account.borrow_field<_, u64>(b"key"), &2u64);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_field_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.rotate_field<_, u64>(b"key", 0u64, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::dynamic_field::EFieldDoesNotExist)]
fun rotate_field_aborts_if_missing() {
    account_test_mut!(|account, scenario| {
        account.rotate_field<_, u64>(b"missing", 0u64, scenario.ctx());
    });
}

// === Admin: authenticator ===

#[test]
fun rotate_auth_function_ref_v1_returns_old_and_stores_new() {
    account_test_mut!(|account, scenario| {
        let old_ref = *account.borrow_auth_function_ref_v1();
        let new_ref = other_test_authenticator();

        let returned = account.rotate_auth_function_ref_v1(new_ref, scenario.ctx());
        assert_eq(returned, old_ref);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &new_ref);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_auth_function_ref_v1_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.rotate_auth_function_ref_v1(test_authenticator(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EBuiltinAuthWithoutPublicKey)]
fun rotate_auth_function_ref_v1_aborts_for_the_builtin_authenticator_without_a_key() {
    account_test_mut!(|account, scenario| {
        account.rotate_auth_function_ref_v1(builtin_test_authenticator(), scenario.ctx());
    });
}

// === Helpers ===

/// Creates a mutable shared `SmartAccount` with a custom authenticator and returns its address.
fun make_account(scenario: &mut Scenario): address {
    smart_account::builder_v1(test_authenticator(), scenario.ctx()).build_v1()
}

fun test_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xABC,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun other_test_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xDEF,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun builtin_test_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x2,
        ascii::string(b"builtin_authenticator_functions"),
        ascii::string(b"builtin_authenticator_v1"),
    )
}

/// Runs `$f` with an immutable reference to a shared `SmartAccount` (any sender).
macro fun account_test($f: |&SmartAccount|) {
    let mut scenario = test_scenario::begin(@0x0);

    let addr = make_account(&mut scenario);

    scenario.next_tx(addr);
    let account = scenario.take_shared<SmartAccount>();

    $f(&account);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` with a mutable reference to a shared `SmartAccount` where the sender is
/// the account itself — satisfying the admin-function sender check.
macro fun account_test_mut($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = test_scenario::begin(@0x0);

    let addr = make_account(&mut scenario);

    scenario.next_tx(addr);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` with a mutable reference to a shared `SmartAccount` where the sender is
/// `@0x1` — a different address from the account, triggering the admin-function
/// sender check to abort.
macro fun account_test_wrong_sender($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = test_scenario::begin(@0x0);

    make_account(&mut scenario);

    scenario.next_tx(@0x1);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}
