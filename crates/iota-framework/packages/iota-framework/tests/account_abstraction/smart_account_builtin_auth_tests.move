// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::smart_account_builtin_auth_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme;
use iota::smart_account::{Self, SmartAccount};
use iota::smart_account_builtin_auth::{Self, BuiltinAuthRule};
use iota::smart_account_rotation_rules;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

use fun smart_account_rotation_rules::has_auth_rotation_rule
    as SmartAccount.has_auth_rotation_rule;
use fun smart_account_rotation_rules::add_auth_rotation_rule
    as SmartAccount.add_auth_rotation_rule;
use fun smart_account_rotation_rules::request_auth_function_ref_rotation_v1
    as SmartAccount.request_auth_function_ref_rotation_v1;
use fun smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1
    as SmartAccount.confirm_auth_function_ref_rotation_v1;

// A rotation rule from another module, used next to `BuiltinAuthRule`.
public struct OtherRule has drop {}

// === builder_v1 ===

#[test]
fun builder_v1_attaches_public_key_and_builtin_auth() {
    builtin_account_view_test!(|account| {
        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &ed25519_public_key(),
        );
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &builtin_authenticator());
        assert_eq(account.has_auth_rotation_rule<BuiltinAuthRule>(), true);
    });
}

#[test]
fun builder_v1_accepts_extra_fields() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account_builtin_auth::builder_v1(ed25519_public_key(), scenario.ctx())
        .with_field(b"answer", 42u64)
        .build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();

    assert_ref_eq(account.borrow_field<_, u64>(b"answer"), &42u64);
    assert_eq(smart_account_builtin_auth::has_public_key(&account), true);

    test_scenario::return_shared(account);
    scenario.end();
}

// === claim_account_v1 ===

#[test]
fun claim_account_v1_creates_shared_account_at_sender_address() {
    let public_key = ed25519_public_key();
    let sender = public_key.to_iota_address();
    let mut scenario = test_scenario::begin(sender);

    smart_account_builtin_auth::claim_account_v1_for_testing(public_key, scenario.ctx());

    scenario.next_tx(sender);
    let account = scenario.take_shared<SmartAccount>();
    assert_eq(account.account_address(), sender);
    assert_ref_eq(smart_account_builtin_auth::borrow_public_key(&account), &public_key);
    assert_eq(smart_account_builtin_auth::has_builtin_auth(&account), true);
    assert_eq(account.has_auth_rotation_rule<BuiltinAuthRule>(), true);
    test_scenario::return_shared(account);

    scenario.end();
}

#[test]
#[expected_failure(abort_code = iota::claim::EAddressMismatch)]
fun claim_account_v1_aborts_on_address_mismatch() {
    let mut scenario = test_scenario::begin(@0x1);

    smart_account_builtin_auth::claim_account_v1_for_testing(ed25519_public_key(), scenario.ctx());

    scenario.end();
}

// === View functions ===

#[test]
fun custom_account_has_no_public_key_or_builtin_auth() {
    custom_account_view_test!(|account| {
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), false);
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun borrow_public_key_aborts_without_a_key() {
    custom_account_view_test!(|account| {
        smart_account_builtin_auth::borrow_public_key(account);
    });
}

// === rotate_to_builtin_auth_v1 ===

#[test]
fun rotate_to_builtin_auth_v1_attaches_key_and_sets_builtin_auth() {
    custom_account_test!(|account, scenario| {
        let previous = smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            multisig_public_key(),
            scenario.ctx(),
        );

        assert_eq(previous, custom_authenticator());
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &multisig_public_key(),
        );
        assert_eq(account.has_auth_rotation_rule<BuiltinAuthRule>(), true);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EBuiltinAuthAlreadySet)]
fun rotate_to_builtin_auth_v1_aborts_if_builtin_auth_already_set() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            multisig_public_key(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_to_builtin_auth_v1_aborts_if_sender_not_account() {
    custom_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            ed25519_public_key(),
            scenario.ctx(),
        );
    });
}

// === rotate_to_custom_auth_v1 ===

#[test]
fun rotate_to_custom_auth_v1_detaches_the_key() {
    builtin_account_test!(|account, scenario| {
        let detached = smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );

        assert_eq(detached, option::some(ed25519_public_key()));
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &custom_authenticator());
        assert_eq(account.has_auth_rotation_rule<BuiltinAuthRule>(), true);
    });
}

#[test]
fun rotate_to_custom_auth_v1_without_a_key_returns_none() {
    custom_account_test!(|account, scenario| {
        let detached = smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            other_custom_authenticator(),
            scenario.ctx(),
        );

        assert_eq(detached, option::none());
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &other_custom_authenticator());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EAuthenticatorIsBuiltin)]
fun rotate_to_custom_auth_v1_aborts_for_the_builtin_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            builtin_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_to_custom_auth_v1_aborts_if_sender_not_account() {
    builtin_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
fun rotating_to_custom_and_back_to_builtin_auth_takes_a_new_key() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            multisig_public_key(),
            scenario.ctx(),
        );

        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &multisig_public_key(),
        );
    });
}

// === rotate_public_key ===

#[test]
fun rotate_public_key_replaces_the_key_with_another_scheme() {
    builtin_account_test!(|account, scenario| {
        let previous = smart_account_builtin_auth::rotate_public_key(
            account,
            multisig_public_key(),
            scenario.ctx(),
        );

        assert_eq(previous, ed25519_public_key());
        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &multisig_public_key(),
        );
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EBuiltinAuthNotSet)]
fun rotate_public_key_aborts_without_builtin_auth() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );

        smart_account_builtin_auth::rotate_public_key(
            account,
            multisig_public_key(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_public_key_aborts_if_sender_not_account() {
    builtin_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            multisig_public_key(),
            scenario.ctx(),
        );
    });
}

// === BuiltinAuthRule ===

#[test]
#[expected_failure(abort_code = iota::smart_account::EAuthRotationRulesAttached)]
fun core_rotation_aborts_with_builtin_auth_rule() {
    builtin_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(custom_authenticator(), scenario.ctx());
    });
}

#[test]
fun core_rotation_sets_builtin_auth_without_public_key_on_an_account_without_the_rule() {
    custom_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(builtin_authenticator(), scenario.ctx());

        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EPublicKeyAttached)]
fun approve_auth_rotation_refuses_a_custom_authenticator_while_the_key_is_attached() {
    builtin_account_test!(|account, scenario| {
        let mut request = account.request_auth_function_ref_rotation_v1(
            custom_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::approve_auth_rotation(account, &mut request);
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EPublicKeyMissing)]
fun approve_auth_rotation_refuses_the_builtin_authenticator_without_a_key() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );

        let mut request = account.request_auth_function_ref_rotation_v1(
            builtin_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::approve_auth_rotation(account, &mut request);
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());
    });
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_builtin_auth::EAuthRotationRequestForAnotherAccount,
)]
fun approve_auth_rotation_aborts_for_another_account() {
    let mut scenario = test_scenario::begin(@0x0);

    let first = smart_account_builtin_auth::builder_v1(ed25519_public_key(), scenario.ctx())
        .build_v1();
    let second = smart_account_builtin_auth::builder_v1(ed25519_public_key(), scenario.ctx())
        .build_v1();

    scenario.next_tx(first);
    let first_account = scenario.take_shared_by_id<SmartAccount>(object::id_from_address(first));
    let second_account = scenario.take_shared_by_id<SmartAccount>(object::id_from_address(second));

    let mut request = first_account.request_auth_function_ref_rotation_v1(
        builtin_authenticator(),
        scenario.ctx(),
    );
    smart_account_builtin_auth::approve_auth_rotation(&second_account, &mut request);

    abort
}

#[test]
fun approve_auth_rotation_allows_custom_to_custom_without_a_key() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );

        let mut request = account.request_auth_function_ref_rotation_v1(
            other_custom_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::approve_auth_rotation(account, &mut request);
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());

        assert_ref_eq(account.borrow_auth_function_ref_v1(), &other_custom_authenticator());
    });
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EAccountChangedAfterRequest,
)]
fun approval_does_not_survive_a_rotation_to_builtin_auth() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
        let mut request = account.request_auth_function_ref_rotation_v1(
            other_custom_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::approve_auth_rotation(account, &mut request);

        smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            ed25519_public_key(),
            scenario.ctx(),
        );
        account.confirm_auth_function_ref_rotation_v1(request, scenario.ctx());
    });
}

// === With other rules ===

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EAuthRotationRulesNotSatisfied,
)]
fun one_call_rotation_aborts_with_another_rule() {
    builtin_account_test!(|account, scenario| {
        account.add_auth_rotation_rule(OtherRule {}, 0u64, scenario.ctx());

        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            custom_authenticator(),
            scenario.ctx(),
        );
    });
}

#[test]
fun rotations_with_request_carry_other_rules_receipts() {
    builtin_account_test!(|account, scenario| {
        account.add_auth_rotation_rule(OtherRule {}, 0u64, scenario.ctx());

        let mut request = account.request_auth_function_ref_rotation_v1(
            custom_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(OtherRule {});
        let detached = smart_account_builtin_auth::rotate_to_custom_auth_with_request_v1(
            account,
            request,
            scenario.ctx(),
        );
        assert_eq(detached, option::some(ed25519_public_key()));

        let mut request = account.request_auth_function_ref_rotation_v1(
            builtin_authenticator(),
            scenario.ctx(),
        );
        request.add_auth_rotation_receipt(OtherRule {});
        smart_account_builtin_auth::rotate_to_builtin_auth_with_request_v1(
            account,
            request,
            multisig_public_key(),
            scenario.ctx(),
        );

        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &multisig_public_key(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EAuthenticatorIsNotBuiltin)]
fun rotate_to_builtin_auth_with_request_v1_aborts_for_a_custom_request() {
    custom_account_test!(|account, scenario| {
        let request = account.request_auth_function_ref_rotation_v1(
            other_custom_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::rotate_to_builtin_auth_with_request_v1(
            account,
            request,
            ed25519_public_key(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EAuthenticatorIsBuiltin)]
fun rotate_to_custom_auth_with_request_v1_aborts_for_a_builtin_request() {
    custom_account_test!(|account, scenario| {
        let request = account.request_auth_function_ref_rotation_v1(
            builtin_authenticator(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::rotate_to_custom_auth_with_request_v1(
            account,
            request,
            scenario.ctx(),
        );
    });
}

// === Helpers ===

fun ed25519_public_key(): PublicKey {
    public_key::create(
        signature_scheme::ed25519(),
        x"0000000000000000000000000000000000000000000000000000000000000000",
    )
}

fun multisig_public_key(): PublicKey {
    // 1-of-1 MultiSig over the zero Ed25519 key.
    public_key::create(
        signature_scheme::multisig(),
        x"01000000000000000000000000000000000000000000000000000000000000000000010100",
    )
}

fun custom_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xABC,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

fun other_custom_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xDEF,
        ascii::string(b"module"),
        ascii::string(b"function"),
    )
}

/// The built-in authenticator ref, as `smart_account_builtin_auth::builder_v1` attaches it.
fun builtin_authenticator(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0x2,
        ascii::string(b"builtin_authenticator_functions"),
        ascii::string(b"builtin_authenticator_v1"),
    )
}

/// Runs `$f` on a shared `SmartAccount` built by `smart_account_builtin_auth::builder_v1` with an
/// Ed25519 key, where the sender is the account itself.
macro fun builtin_account_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = test_scenario::begin(@0x0);

    let addr = smart_account_builtin_auth::builder_v1(ed25519_public_key(), scenario.ctx())
        .build_v1();

    scenario.next_tx(addr);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` built by `smart_account_builtin_auth::builder_v1` with an
/// Ed25519 key, where the sender is `@0x1` — not the account.
macro fun builtin_account_wrong_sender_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account_builtin_auth::builder_v1(ed25519_public_key(), scenario.ctx()).build_v1();

    scenario.next_tx(@0x1);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` built by `smart_account_builtin_auth::builder_v1` with an
/// Ed25519 key.
macro fun builtin_account_view_test($f: |&SmartAccount|) {
    let mut scenario = test_scenario::begin(@0x0);
    smart_account_builtin_auth::builder_v1(ed25519_public_key(), scenario.ctx()).build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();

    $f(&account);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` built with a custom authenticator and no key, where the
/// sender is the account itself.
macro fun custom_account_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = test_scenario::begin(@0x0);

    let addr = smart_account::builder_v1(custom_authenticator(), scenario.ctx()).build_v1();

    scenario.next_tx(addr);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` built with a custom authenticator and no key.
macro fun custom_account_view_test($f: |&SmartAccount|) {
    let mut scenario = test_scenario::begin(@0x0);
    smart_account::builder_v1(custom_authenticator(), scenario.ctx()).build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();

    $f(&account);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on a shared `SmartAccount` built with a custom authenticator, where the sender is
/// `@0x1` — not the account.
macro fun custom_account_wrong_sender_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account::builder_v1(custom_authenticator(), scenario.ctx()).build_v1();

    scenario.next_tx(@0x1);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}
