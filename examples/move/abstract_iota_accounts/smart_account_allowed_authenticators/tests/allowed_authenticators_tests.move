// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module smart_account_allowed_authenticators::allowed_authenticators_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::builtin_authenticator_functions::builtin_authenticator_function_ref_v1;
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme;
use iota::smart_account::{Self, SmartAccount};
use iota::smart_account_builtin_auth::{Self, BuiltinAuthRule};
use iota::smart_account_rotation_rules;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use smart_account_allowed_authenticators::allowed_authenticators::{
    Self,
    AllowedAuthenticatorsRule
};
use smart_account_allowed_authenticators::ed25519_authenticator;
use smart_account_allowed_authenticators::time_locked_authenticator;
use std::ascii;

// An Ed25519 key, a transaction digest and the key's signature of it.
const PUBLIC_KEY: vector<u8> = x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
const DIGEST: vector<u8> = x"315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3";
const SIGNATURE: vector<u8> =
    x"cce72947906dbae4c166fc01fd096432784032be43db540909bc901dbc057992b4d655ca4f4355cf0868e1266baacf6919902969f063e74162f8f04bc4056105";

// === Attaching the list ===

#[test]
fun with_allowed_authenticators_attaches_the_list_next_to_the_builtin_rule() {
    account_view_test!(|account| {
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_eq(
            smart_account_rotation_rules::has_auth_rotation_rule<BuiltinAuthRule>(account),
            true,
        );
        assert_eq(
            smart_account_rotation_rules::has_auth_rotation_rule<AllowedAuthenticatorsRule>(
                account,
            ),
            true,
        );
        assert_eq(allowed_authenticators::allowed_authenticators(account), allowed());
    });
}

#[test]
fun attach_to_an_existing_builtin_auth_account() {
    let mut scenario = test_scenario::begin(@0x0);

    let account_address = smart_account_builtin_auth::builder_v1(public_key(), scenario.ctx())
        .build_v1();

    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    allowed_authenticators::attach_allowed_authenticators(&mut account, allowed(), scenario.ctx());

    allowed_authenticators::rotate_to_custom_auth_v1(&mut account, ed25519_ref(), scenario.ctx());
    assert_ref_eq(account.borrow_auth_function_ref_v1(), &ed25519_ref());
    allowed_authenticators::rotate_to_builtin_auth_v1(&mut account, public_key(), scenario.ctx());
    assert_eq(smart_account_builtin_auth::has_builtin_auth(&account), true);

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
fun attach_to_an_existing_custom_auth_account() {
    let mut scenario = test_scenario::begin(@0x0);

    let account_address = smart_account::builder_v1(unlisted_ref(), scenario.ctx()).build_v1();

    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    allowed_authenticators::attach_allowed_authenticators(
        &mut account,
        vector[time_locked_ref()],
        scenario.ctx(),
    );
    assert_eq(
        smart_account_rotation_rules::has_auth_rotation_rule<BuiltinAuthRule>(&account),
        false,
    );

    let detached = allowed_authenticators::rotate_to_custom_auth_v1(
        &mut account,
        time_locked_ref(),
        scenario.ctx(),
    );
    assert_eq(detached, option::none());
    assert_ref_eq(account.borrow_auth_function_ref_v1(), &time_locked_ref());

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
#[expected_failure(
    abort_code = iota::smart_account_rotation_rules::EAuthRotationRuleAlreadyAttached,
)]
fun attach_twice_aborts() {
    account_test!(|account, scenario| {
        allowed_authenticators::attach_allowed_authenticators(account, allowed(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun attach_aborts_for_another_sender() {
    let mut scenario = test_scenario::begin(@0x0);

    smart_account_builtin_auth::builder_v1(public_key(), scenario.ctx()).build_v1();

    scenario.next_tx(@0x1);
    let mut account = scenario.take_shared<SmartAccount>();
    allowed_authenticators::attach_allowed_authenticators(&mut account, allowed(), scenario.ctx());

    abort
}

// === Rotation ===

#[test]
fun rotates_to_allowed_authenticators_and_back() {
    account_test!(|account, scenario| {
        let detached = allowed_authenticators::rotate_to_custom_auth_v1(
            account,
            ed25519_ref(),
            scenario.ctx(),
        );
        assert_eq(detached, option::some(public_key()));
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &ed25519_ref());

        let detached = allowed_authenticators::rotate_to_custom_auth_v1(
            account,
            time_locked_ref(),
            scenario.ctx(),
        );
        assert_eq(detached, option::none());
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &time_locked_ref());

        allowed_authenticators::rotate_to_builtin_auth_v1(account, public_key(), scenario.ctx());
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_ref_eq(smart_account_builtin_auth::borrow_public_key(account), &public_key());
    });
}

#[test]
#[expected_failure(abort_code = allowed_authenticators::EAuthenticatorNotAllowed)]
fun rotation_to_an_unlisted_authenticator_aborts() {
    account_test!(|account, scenario| {
        allowed_authenticators::rotate_to_custom_auth_v1(account, unlisted_ref(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = allowed_authenticators::EAuthenticatorNotAllowed)]
fun rotation_to_an_unlisted_builtin_authenticator_aborts() {
    let mut scenario = test_scenario::begin(@0x0);

    let builder = smart_account_builtin_auth::builder_v1(public_key(), scenario.ctx());
    let account_address = allowed_authenticators::with_allowed_authenticators(
        builder,
        vector[ed25519_ref()],
    ).build_v1();

    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();
    allowed_authenticators::rotate_to_custom_auth_v1(&mut account, ed25519_ref(), scenario.ctx());
    allowed_authenticators::rotate_to_builtin_auth_v1(&mut account, public_key(), scenario.ctx());

    abort
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EPublicKeyAttached)]
fun builtin_rule_refuses_a_custom_authenticator_while_the_key_is_attached() {
    account_test!(|account, scenario| {
        let mut request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
            account,
            ed25519_ref(),
            scenario.ctx(),
        );
        allowed_authenticators::approve_auth_rotation(account, &mut request);
        smart_account_builtin_auth::approve_auth_rotation(account, &mut request);
        smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1(
            account,
            request,
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_rotation_rules::EAuthRotationRulesNotSatisfied)]
fun rotation_without_the_list_receipt_aborts() {
    account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_custom_auth_v1(
            account,
            ed25519_ref(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EAuthRotationRulesAttached)]
fun core_rotation_aborts() {
    account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(unlisted_ref(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = allowed_authenticators::ERequestForAnotherAccount)]
fun approval_for_another_account_aborts() {
    let mut scenario = test_scenario::begin(@0x0);

    let first = make_account(&mut scenario);
    let second = make_account(&mut scenario);

    scenario.next_tx(first);
    let first_account = scenario.take_shared_by_id<SmartAccount>(object::id_from_address(first));
    let second_account = scenario.take_shared_by_id<SmartAccount>(object::id_from_address(second));

    let mut request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
        &first_account,
        ed25519_ref(),
        scenario.ctx(),
    );
    allowed_authenticators::approve_auth_rotation(&second_account, &mut request);

    abort
}

// === disallow_authenticator ===

#[test]
#[expected_failure(abort_code = allowed_authenticators::EAuthenticatorNotAllowed)]
fun rotation_to_a_disallowed_authenticator_aborts() {
    account_test!(|account, scenario| {
        allowed_authenticators::disallow_authenticator(account, &ed25519_ref(), scenario.ctx());
        assert_eq(allowed_authenticators::is_allowed(account, &ed25519_ref()), false);

        allowed_authenticators::rotate_to_custom_auth_v1(account, ed25519_ref(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun disallow_authenticator_aborts_for_another_sender() {
    let mut scenario = test_scenario::begin(@0x0);

    make_account(&mut scenario);

    scenario.next_tx(@0x1);
    let mut account = scenario.take_shared<SmartAccount>();
    allowed_authenticators::disallow_authenticator(&mut account, &ed25519_ref(), scenario.ctx());

    abort
}

// === Authenticators ===

#[test]
fun ed25519_authenticator_accepts_the_key_signature() {
    account_test!(|account, scenario| {
        ed25519_authenticator::set_public_key(account, PUBLIC_KEY, scenario.ctx());
        assert_eq(ed25519_authenticator::public_key(account), option::some(PUBLIC_KEY));

        let ctx = tx_context::new(account.account_address(), DIGEST, 0, 0, 0);
        ed25519_authenticator::authenticate(account, SIGNATURE, &auth_context(), &ctx);
    });
}

#[test]
#[expected_failure(abort_code = ed25519_authenticator::EEd25519VerificationFailed)]
fun ed25519_authenticator_rejects_another_digest() {
    account_test!(|account, scenario| {
        ed25519_authenticator::set_public_key(account, PUBLIC_KEY, scenario.ctx());

        let mut digest = DIGEST;
        *digest.borrow_mut(0) = 0;
        let ctx = tx_context::new(account.account_address(), digest, 0, 0, 0);
        ed25519_authenticator::authenticate(account, SIGNATURE, &auth_context(), &ctx);
    });
}

#[test]
#[expected_failure(abort_code = ed25519_authenticator::EPublicKeyMissing)]
fun ed25519_authenticator_rejects_without_its_key() {
    account_view_test!(|account| {
        let ctx = tx_context::new(account.account_address(), DIGEST, 0, 0, 0);
        ed25519_authenticator::authenticate(account, SIGNATURE, &auth_context(), &ctx);
    });
}

#[test]
fun time_locked_authenticator_accepts_after_the_unlock_time() {
    account_test!(|account, scenario| {
        ed25519_authenticator::set_public_key(account, PUBLIC_KEY, scenario.ctx());
        time_locked_authenticator::set_unlock_time(account, 1000, scenario.ctx());
        time_locked_authenticator::set_unlock_time(account, 2000, scenario.ctx());
        assert_eq(time_locked_authenticator::unlock_time(account), option::some(2000));

        let ctx = tx_context::new(account.account_address(), DIGEST, 0, 2000, 0);
        time_locked_authenticator::authenticate(account, SIGNATURE, &auth_context(), &ctx);
    });
}

#[test]
#[expected_failure(abort_code = time_locked_authenticator::EAccountStillLocked)]
fun time_locked_authenticator_rejects_before_the_unlock_time() {
    account_test!(|account, scenario| {
        ed25519_authenticator::set_public_key(account, PUBLIC_KEY, scenario.ctx());
        time_locked_authenticator::set_unlock_time(account, 2000, scenario.ctx());

        let ctx = tx_context::new(account.account_address(), DIGEST, 0, 1999, 0);
        time_locked_authenticator::authenticate(account, SIGNATURE, &auth_context(), &ctx);
    });
}

#[test]
#[expected_failure(abort_code = time_locked_authenticator::EUnlockTimeMissing)]
fun time_locked_authenticator_rejects_without_an_unlock_time() {
    account_view_test!(|account| {
        let ctx = tx_context::new(account.account_address(), DIGEST, 0, 0, 0);
        time_locked_authenticator::authenticate(account, SIGNATURE, &auth_context(), &ctx);
    });
}

// === Helpers ===

fun public_key(): PublicKey {
    public_key::create(signature_scheme::ed25519(), PUBLIC_KEY)
}

/// The list used by the tests: the built-in authenticator and this package's two.
fun allowed(): vector<AuthenticatorFunctionRefV1<SmartAccount>> {
    vector[builtin_authenticator_function_ref_v1(), ed25519_ref(), time_locked_ref()]
}

fun ed25519_ref(): AuthenticatorFunctionRefV1<SmartAccount> {
    example_ref(b"ed25519_authenticator")
}

fun time_locked_ref(): AuthenticatorFunctionRefV1<SmartAccount> {
    example_ref(b"time_locked_authenticator")
}

fun unlisted_ref(): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xABC,
        ascii::string(b"module"),
        ascii::string(b"authenticate"),
    )
}

fun example_ref(module_name: vector<u8>): AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @smart_account_allowed_authenticators,
        ascii::string(module_name),
        ascii::string(b"authenticate"),
    )
}

fun auth_context(): AuthContext {
    auth_context::new_for_testing(
        b"00000000000000000000000000000000",
        vector[],
        vector[],
        vector[],
        b"00000000000000000000000000000000",
        option::none(),
        option::none(),
        option::none(),
    )
}

/// Builds an account with the built-in authenticator and the list `allowed()`, and returns its
/// address.
fun make_account(scenario: &mut Scenario): address {
    let builder = smart_account_builtin_auth::builder_v1(public_key(), scenario.ctx());
    allowed_authenticators::with_allowed_authenticators(builder, allowed()).build_v1()
}

/// Runs `$f` on an account made by `make_account`, in a transaction sent by the account.
macro fun account_test($f: |&mut SmartAccount, &mut Scenario|) {
    let mut scenario = test_scenario::begin(@0x0);

    let account_address = make_account(&mut scenario);

    scenario.next_tx(account_address);
    let mut account = scenario.take_shared<SmartAccount>();

    $f(&mut account, &mut scenario);

    test_scenario::return_shared(account);
    scenario.end();
}

/// Runs `$f` on an account made by `make_account`.
macro fun account_view_test($f: |&SmartAccount|) {
    let mut scenario = test_scenario::begin(@0x0);

    let account_address = make_account(&mut scenario);

    scenario.next_tx(account_address);
    let account = scenario.take_shared<SmartAccount>();

    $f(&account);

    test_scenario::return_shared(account);
    scenario.end();
}
