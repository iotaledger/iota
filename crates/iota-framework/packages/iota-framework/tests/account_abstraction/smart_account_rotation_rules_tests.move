// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::smart_account_rotation_rules_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::builtin_authenticator_functions::PublicKeyFieldName;
use iota::event;
use iota::smart_account::{Self, SmartAccount, SmartAccountBuilder};
use iota::smart_account_rotation_rules;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;
use std::type_name;

use fun smart_account_rotation_rules::with_auth_rotation_rule
    as SmartAccountBuilder.with_auth_rotation_rule;
use fun smart_account_rotation_rules::add_auth_rotation_rule as SmartAccount.add_auth_rotation_rule;
use fun smart_account_rotation_rules::remove_auth_rotation_rule
    as SmartAccount.remove_auth_rotation_rule;
use fun smart_account_rotation_rules::has_auth_rotation_rule as SmartAccount.has_auth_rotation_rule;
use fun smart_account_rotation_rules::auth_rotation_rules as SmartAccount.auth_rotation_rules;
use fun smart_account_rotation_rules::borrow_auth_rotation_rule_config
    as SmartAccount.borrow_auth_rotation_rule_config;
use fun smart_account_rotation_rules::borrow_auth_rotation_rule_config_mut
    as SmartAccount.borrow_auth_rotation_rule_config_mut;
use fun smart_account_rotation_rules::request_auth_function_ref_rotation_v1
    as SmartAccount.request_auth_function_ref_rotation_v1;
use fun smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1
    as SmartAccount.confirm_auth_function_ref_rotation_v1;

// Rotation rules used by the tests.
public struct RuleA has drop {}
public struct RuleB has drop {}

// A field key type guarded by the field rotation rule tests.
public struct GuardedKey has copy, drop, store {}

// === Core rotation ===

#[test]
#[expected_failure(abort_code = iota::smart_account::EAuthRotationRulesAttached)]
fun rotate_auth_function_ref_v1_aborts_with_a_rule_attached() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());

        account.rotate_auth_function_ref_v1(other_test_authenticator(), scenario.ctx());
    });
}

#[test]
fun removing_the_last_rule_allows_core_rotation_again() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        assert_eq(account.has_auth_rotation_rules(), true);

        account.remove_auth_rotation_rule<_, u64>(RuleA {}, scenario.ctx());
        assert_eq(account.has_auth_rotation_rules(), false);

        let new_ref = other_test_authenticator();
        account.rotate_auth_function_ref_v1(new_ref, scenario.ctx());
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &new_ref);
    });
}

// === Rotation rules ===

#[test]
fun add_and_remove_auth_rotation_rules() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        account.add_auth_rotation_rule(RuleB {}, 2u64, scenario.ctx());

        assert_eq(account.has_auth_rotation_rule<RuleA>(), true);
        assert_eq(account.has_auth_rotation_rule<RuleB>(), true);
        assert_eq(
            account.auth_rotation_rules(),
            vector[type_name::get<RuleA>(), type_name::get<RuleB>()],
        );

        let config = account.remove_auth_rotation_rule<_, u64>(RuleA {}, scenario.ctx());

        assert_eq(config, 1u64);
        assert_eq(account.has_auth_rotation_rule<RuleA>(), false);
        assert_eq(account.auth_rotation_rules(), vector[type_name::get<RuleB>()]);
        assert_eq(
            event::events_by_type<smart_account_rotation_rules::AuthRotationRuleAdded>().length(),
            2,
        );
        assert_eq(
            event::events_by_type<smart_account_rotation_rules::AuthRotationRuleRemoved>().length(),
            1,
        );
    });
}

#[test]
fun auth_rotation_rule_config_can_be_read_and_updated() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());

        assert_ref_eq(account.borrow_auth_rotation_rule_config<_, u64>(RuleA {}), &1u64);

        *account.borrow_auth_rotation_rule_config_mut<_, u64>(RuleA {}, scenario.ctx()) = 5;

        assert_ref_eq(account.borrow_auth_rotation_rule_config<_, u64>(RuleA {}), &5u64);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAuthRotationRuleNotAttached)]
fun borrow_auth_rotation_rule_config_aborts_if_not_attached() {
    account_test!(|account| {
        account.borrow_auth_rotation_rule_config<_, u64>(RuleA {});
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun borrow_auth_rotation_rule_config_mut_aborts_if_sender_not_account() {
    let mut scenario = test_scenario::begin(@0x0);
    let addr = make_account(&mut scenario);

    scenario.next_tx(addr);
    let mut account = scenario.take_shared<SmartAccount>();
    account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
    test_scenario::return_shared(account);

    scenario.next_tx(@0x1);
    let mut account = scenario.take_shared<SmartAccount>();
    account.borrow_auth_rotation_rule_config_mut<_, u64>(RuleA {}, scenario.ctx());

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
fun with_auth_rotation_rule_attaches_rule_at_build() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account::builder_v1(test_authenticator(), scenario.ctx())
        .with_auth_rotation_rule(RuleA {}, 1u64)
        .build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();
    assert_eq(account.has_auth_rotation_rule<RuleA>(), true);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
fun with_auth_rotation_rule_attaches_two_rules_at_build() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account::builder_v1(test_authenticator(), scenario.ctx())
        .with_auth_rotation_rule(RuleA {}, 1u64)
        .with_auth_rotation_rule(RuleB {}, 2u64)
        .build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();
    assert_eq(
        account.auth_rotation_rules(),
        vector[type_name::get<RuleA>(), type_name::get<RuleB>()],
    );
    assert_eq(account.has_auth_rotation_rules(), true);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EAuthRotationRuleAlreadyAttached,
)]
fun with_auth_rotation_rule_aborts_if_already_attached_at_build() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account::builder_v1(test_authenticator(), scenario.ctx())
        .with_auth_rotation_rule(RuleA {}, 1u64)
        .with_auth_rotation_rule(RuleA {}, 2u64)
        .build_v1();

    scenario.end();
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EAuthRotationRuleAlreadyAttached,
)]
fun add_auth_rotation_rule_aborts_if_already_attached() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAuthRotationRuleNotAttached)]
fun remove_auth_rotation_rule_aborts_if_not_attached() {
    account_test_mut!(|account, scenario| {
        account.remove_auth_rotation_rule<_, u64>(RuleA {}, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun add_auth_rotation_rule_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
    });
}

#[test]
fun rotation_request_with_every_receipt_rotates() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        account.add_auth_rotation_rule(RuleB {}, 2u64, scenario.ctx());
        let new_ref = other_test_authenticator();

        let mut request = account.request_auth_function_ref_rotation_v1(new_ref, scenario.ctx());
        assert_eq(request.request_account_id(), object::id(account));
        assert_ref_eq(request.request_authenticator(), &new_ref);
        request.add_auth_rotation_receipt(RuleB {});
        request.add_auth_rotation_receipt(RuleA {});
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());

        assert_ref_eq(account.borrow_auth_function_ref_v1(), &new_ref);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAuthRotationRulesNotSatisfied)]
fun rotation_request_aborts_with_a_receipt_missing() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        account.add_auth_rotation_rule(RuleB {}, 2u64, scenario.ctx());

        let mut request = account.request_auth_function_ref_rotation_v1(
            other_test_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(RuleA {});
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAuthRotationRulesNotSatisfied)]
fun rotation_request_aborts_with_a_receipt_of_an_unattached_rule() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());

        let mut request = account.request_auth_function_ref_rotation_v1(
            other_test_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(RuleA {});
        request.add_auth_rotation_receipt(RuleB {});
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());
    });
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EAuthRotationRequestForAnotherAccount,
)]
fun rotation_request_aborts_on_another_account() {
    let mut scenario = test_scenario::begin(@0x0);
    let first = make_account(&mut scenario);
    let second = make_account(&mut scenario);

    scenario.next_tx(first);
    let first_account = scenario.take_shared_by_id<SmartAccount>(object::id_from_address(first));
    let request = first_account.request_auth_function_ref_rotation_v1(
        other_test_authenticator(),
        scenario.ctx(),
    );
    test_scenario::return_shared(first_account);

    scenario.next_tx(second);
    let mut second_account = scenario.take_shared_by_id<SmartAccount>(
        object::id_from_address(second),
    );
    second_account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());

    test_scenario::return_shared(second_account);
    scenario.end();
}

// === Field rotation rules ===

#[test]
fun add_and_remove_field_rotation_rule() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        smart_account_rotation_rules::add_field_rotation_rule<_, GuardedKey>(
            account,
            RuleA {},
            scenario.ctx(),
        );

        assert_eq(account.has_field_rotation_rules<GuardedKey>(), true);
        assert_eq(
            smart_account_rotation_rules::field_rotation_rules<GuardedKey>(account),
            vector[type_name::get<RuleA>()],
        );

        smart_account_rotation_rules::remove_field_rotation_rule<_, GuardedKey>(
            account,
            RuleA {},
            scenario.ctx(),
        );

        assert_eq(account.has_field_rotation_rules<GuardedKey>(), false);
        assert_eq(
            event::events_by_type<smart_account_rotation_rules::FieldRotationRuleAdded>().length(),
            1,
        );
        assert_eq(
            event::events_by_type<smart_account_rotation_rules::FieldRotationRuleRemoved>()
                .length(),
            1,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAuthRotationRuleNotAttached)]
fun add_field_rotation_rule_aborts_if_rule_not_attached() {
    account_test_mut!(|account, scenario| {
        smart_account_rotation_rules::add_field_rotation_rule<_, GuardedKey>(
            account,
            RuleA {},
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EFieldRotationRuleAlreadyAttached,
)]
fun add_field_rotation_rule_aborts_if_already_guarding() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);
        smart_account_rotation_rules::add_field_rotation_rule<_, GuardedKey>(
            account,
            RuleA {},
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EAuthRotationRuleGuardsFields,
)]
fun remove_auth_rotation_rule_aborts_while_guarding_fields() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);

        account.remove_auth_rotation_rule<_, u64>(RuleA {}, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EFieldTypeNotGuardable)]
fun add_field_rotation_rule_aborts_for_the_builtin_public_key_field() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        smart_account_rotation_rules::add_field_rotation_rule<_, PublicKeyFieldName>(
            account,
            RuleA {},
            scenario.ctx(),
        );
    });
}

#[test]
fun with_field_rotation_rule_guards_at_build() {
    let mut scenario = test_scenario::begin(@0x0);

    let builder = smart_account::builder_v1(test_authenticator(), scenario.ctx())
        .with_field(GuardedKey {}, 1u64)
        .with_auth_rotation_rule(RuleA {}, 1u64);
    smart_account_rotation_rules::with_field_rotation_rule<_, GuardedKey>(builder, RuleA {})
        .build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();
    assert_eq(account.has_field_rotation_rules<GuardedKey>(), true);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EFieldRotationRulesAttached)]
fun core_add_field_aborts_while_guarded() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);
        account.add_field(GuardedKey {}, 1u64, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EFieldRotationRulesAttached)]
fun core_remove_field_aborts_while_guarded() {
    account_test_mut!(|account, scenario| {
        account.add_field(GuardedKey {}, 1u64, scenario.ctx());
        guard_with_rule_a(account, scenario);
        account.remove_field<_, u64>(GuardedKey {}, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EFieldRotationRulesAttached)]
fun core_rotate_field_aborts_while_guarded() {
    account_test_mut!(|account, scenario| {
        account.add_field(GuardedKey {}, 1u64, scenario.ctx());
        guard_with_rule_a(account, scenario);
        account.rotate_field(GuardedKey {}, 2u64, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EFieldRotationRulesAttached)]
fun core_borrow_field_mut_aborts_while_guarded() {
    account_test_mut!(|account, scenario| {
        account.add_field(GuardedKey {}, 1u64, scenario.ctx());
        guard_with_rule_a(account, scenario);
        account.borrow_field_mut<_, u64>(GuardedKey {}, scenario.ctx());
    });
}

#[test]
fun guarded_field_changes_with_every_receipt() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);

        let request = approved_request(
            account,
            smart_account_rotation_rules::field_add(),
            scenario,
        );
        smart_account_rotation_rules::add_field(
            account,
            request,
            GuardedKey {},
            1u64,
            scenario.ctx(),
        );

        let request = approved_request(
            account,
            smart_account_rotation_rules::field_rotate(),
            scenario,
        );
        let previous = smart_account_rotation_rules::rotate_field(
            account,
            request,
            GuardedKey {},
            2u64,
            scenario.ctx(),
        );
        assert_eq(previous, 1u64);

        let request = approved_request(
            account,
            smart_account_rotation_rules::field_borrow_mut(),
            scenario,
        );
        *smart_account_rotation_rules::borrow_field_mut<_, u64>(
            account,
            request,
            GuardedKey {},
            scenario.ctx(),
        ) = 3;
        assert_ref_eq(account.borrow_field<_, u64>(GuardedKey {}), &3u64);

        let request = approved_request(
            account,
            smart_account_rotation_rules::field_remove(),
            scenario,
        );
        let removed = smart_account_rotation_rules::remove_field<_, u64>(
            account,
            request,
            GuardedKey {},
            scenario.ctx(),
        );
        assert_eq(removed, 3u64);
        assert_eq(account.has_field(GuardedKey {}), false);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAuthRotationRulesNotSatisfied)]
fun guarded_field_change_aborts_with_a_receipt_missing() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);

        let request = smart_account_rotation_rules::request_field_change_v1<GuardedKey>(
            account,
            smart_account_rotation_rules::field_add(),
            scenario.ctx(),
        );
        smart_account_rotation_rules::add_field(
            account,
            request,
            GuardedKey {},
            1u64,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EFieldChangeOperationMismatch,
)]
fun guarded_field_change_aborts_for_another_operation() {
    account_test_mut!(|account, scenario| {
        account.add_field(GuardedKey {}, 1u64, scenario.ctx());
        guard_with_rule_a(account, scenario);

        let request = approved_request(
            account,
            smart_account_rotation_rules::field_rotate(),
            scenario,
        );
        smart_account_rotation_rules::remove_field<_, u64>(
            account,
            request,
            GuardedKey {},
            scenario.ctx(),
        );
    });
}

#[test]
fun unguarded_field_needs_no_request() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);

        account.add_field(b"unguarded", 1u64, scenario.ctx());

        assert_eq(account.has_field(b"unguarded"), true);
    });
}

// === Changes after a request ===

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAccountChangedAfterRequest)]
fun rotation_aborts_if_another_rotation_was_confirmed_after_the_request() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());

        let mut first = account.request_auth_function_ref_rotation_v1(
            other_test_authenticator(),
            scenario.ctx(),
        );
        first.add_auth_rotation_receipt(RuleA {});
        let mut second = account.request_auth_function_ref_rotation_v1(
            test_authenticator(),
            scenario.ctx(),
        );
        second.add_auth_rotation_receipt(RuleA {});

        account.confirm_auth_function_ref_rotation_v1(second, scenario.ctx());
        account.confirm_auth_function_ref_rotation_v1(first, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAccountChangedAfterRequest)]
fun rotation_aborts_if_a_guarded_field_changed_after_the_request() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);

        let mut request = account.request_auth_function_ref_rotation_v1(
            other_test_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(RuleA {});

        let field_request = approved_request(
            account,
            smart_account_rotation_rules::field_add(),
            scenario,
        );
        smart_account_rotation_rules::add_field(
            account,
            field_request,
            GuardedKey {},
            1u64,
            scenario.ctx(),
        );
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAccountChangedAfterRequest)]
fun field_change_aborts_if_a_rotation_was_confirmed_after_the_request() {
    account_test_mut!(|account, scenario| {
        guard_with_rule_a(account, scenario);

        let field_request = approved_request(
            account,
            smart_account_rotation_rules::field_add(),
            scenario,
        );

        let mut request = account.request_auth_function_ref_rotation_v1(
            other_test_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(RuleA {});
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());

        smart_account_rotation_rules::add_field(
            account,
            field_request,
            GuardedKey {},
            1u64,
            scenario.ctx(),
        );
    });
}

#[test]
fun unguarded_field_change_after_the_request_is_ignored() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
        let new_ref = other_test_authenticator();

        let mut request = account.request_auth_function_ref_rotation_v1(new_ref, scenario.ctx());
        request.add_auth_rotation_receipt(RuleA {});
        account.add_field(b"unguarded", 1u64, scenario.ctx());
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());

        assert_ref_eq(account.borrow_auth_function_ref_v1(), &new_ref);
    });
}

#[test]
fun request_made_after_a_change_is_accepted() {
    account_test_mut!(|account, scenario| {
        account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());

        let mut request = account.request_auth_function_ref_rotation_v1(
            other_test_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(RuleA {});
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());

        let mut request = account.request_auth_function_ref_rotation_v1(
            test_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(RuleA {});
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());

        assert_ref_eq(account.borrow_auth_function_ref_v1(), &test_authenticator());
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

/// Attaches `RuleA` to the account and makes it guard `GuardedKey`.
fun guard_with_rule_a(account: &mut SmartAccount, scenario: &mut Scenario) {
    account.add_auth_rotation_rule(RuleA {}, 1u64, scenario.ctx());
    smart_account_rotation_rules::add_field_rotation_rule<_, GuardedKey>(
        account,
        RuleA {},
        scenario.ctx(),
    );
}

/// Returns a `GuardedKey` change request for `operation`, approved by `RuleA`.
fun approved_request(
    account: &SmartAccount,
    operation: u8,
    scenario: &mut Scenario,
): smart_account_rotation_rules::FieldChangeRequest<GuardedKey> {
    let mut request = smart_account_rotation_rules::request_field_change_v1<GuardedKey>(
        account,
        operation,
        scenario.ctx(),
    );
    request.add_field_change_receipt(RuleA {});
    request
}
