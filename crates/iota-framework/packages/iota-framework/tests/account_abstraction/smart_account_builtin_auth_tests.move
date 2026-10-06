// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::smart_account_builtin_auth_tests;

use iota::authenticator_function::{Self, AuthenticatorFunctionRefV1};
use iota::public_key::{Self, PublicKey};
use iota::signature_scheme;
use iota::smart_account::{Self, SmartAccount};
use iota::smart_account_builtin_auth;
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

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
fun borrow_public_key_aborts_if_missing() {
    custom_account_view_test!(|account| {
        smart_account_builtin_auth::borrow_public_key(account);
    });
}

// === attach_public_key ===

#[test]
fun attach_public_key_keeps_custom_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::attach_public_key(
            account,
            secp256k1_public_key(),
            scenario.ctx(),
        );

        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &secp256k1_public_key(),
        );
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &custom_authenticator());
    });
}

#[test]
fun attach_public_key_again_after_detach() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::attach_public_key(
            account,
            secp256k1_public_key(),
            scenario.ctx(),
        );
        smart_account_builtin_auth::detach_public_key(account, scenario.ctx());
        smart_account_builtin_auth::attach_public_key(
            account,
            ed25519_public_key(),
            scenario.ctx(),
        );

        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &ed25519_public_key(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyAlreadyAttached)]
fun attach_public_key_aborts_if_already_attached() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::attach_public_key(
            account,
            secp256k1_public_key(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun attach_public_key_aborts_if_sender_not_account() {
    custom_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::attach_public_key(
            account,
            ed25519_public_key(),
            scenario.ctx(),
        );
    });
}

// === detach_public_key ===

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EBuiltinAuthAttached)]
fun detach_public_key_aborts_while_builtin_auth_attached() {
    builtin_account_test!(|account, scenario| {
        smart_account_builtin_auth::detach_public_key(account, scenario.ctx());
    });
}

#[test]
fun detach_public_key_succeeds_after_rotating_to_custom_authenticator() {
    builtin_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(custom_authenticator(), scenario.ctx());

        let returned = smart_account_builtin_auth::detach_public_key(account, scenario.ctx());

        assert_eq(returned, ed25519_public_key());
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun detach_public_key_aborts_if_missing() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::detach_public_key(account, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun detach_public_key_aborts_if_sender_not_account() {
    custom_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::detach_public_key(account, scenario.ctx());
    });
}

// === rotate_public_key ===

#[test]
fun rotate_public_key_to_other_scheme_keeps_builtin_auth() {
    builtin_account_test!(|account, scenario| {
        let returned = smart_account_builtin_auth::rotate_public_key(
            account,
            passkey_public_key(),
            scenario.ctx(),
        );

        assert_eq(returned, ed25519_public_key());
        assert_ref_eq(
            smart_account_builtin_auth::borrow_public_key(account),
            &passkey_public_key(),
        );
        assert_ref_eq(account.borrow_auth_function_ref_v1(), &builtin_authenticator());
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun rotate_public_key_aborts_if_missing() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            ed25519_public_key(),
            scenario.ctx(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_public_key_aborts_if_sender_not_account() {
    custom_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_public_key(
            account,
            ed25519_public_key(),
            scenario.ctx(),
        );
    });
}

// === Authenticator rotation ===

#[test]
fun rotate_to_builtin_auth_v1_switches_from_custom_authenticator() {
    custom_account_test!(|account, scenario| {
        smart_account_builtin_auth::attach_public_key(
            account,
            multisig_public_key(),
            scenario.ctx(),
        );

        let previous = smart_account_builtin_auth::rotate_to_builtin_auth_v1(
            account,
            scenario.ctx(),
        );

        assert_eq(previous, custom_authenticator());
        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account_builtin_auth::EPublicKeyMissing)]
fun rotate_to_builtin_auth_v1_aborts_after_the_key_was_detached() {
    builtin_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(custom_authenticator(), scenario.ctx());
        smart_account_builtin_auth::detach_public_key(account, scenario.ctx());

        smart_account_builtin_auth::rotate_to_builtin_auth_v1(account, scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_to_builtin_auth_v1_aborts_if_sender_not_account() {
    custom_account_wrong_sender_test!(|account, scenario| {
        smart_account_builtin_auth::rotate_to_builtin_auth_v1(account, scenario.ctx());
    });
}

#[test]
fun core_rotation_sets_builtin_auth_without_public_key() {
    custom_account_test!(|account, scenario| {
        account.rotate_auth_function_ref_v1(builtin_authenticator(), scenario.ctx());

        assert_eq(smart_account_builtin_auth::has_builtin_auth(account), true);
        assert_eq(smart_account_builtin_auth::has_public_key(account), false);
    });
}

// === Helpers ===

fun ed25519_public_key(): PublicKey {
    public_key::create(
        signature_scheme::ed25519(),
        x"0000000000000000000000000000000000000000000000000000000000000000",
    )
}

fun secp256k1_public_key(): PublicKey {
    // Compressed secp256k1 generator point G.
    public_key::create(
        signature_scheme::secp256k1(),
        x"0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    )
}

fun passkey_public_key(): PublicKey {
    public_key::create(
        signature_scheme::passkey(),
        x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a",
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
