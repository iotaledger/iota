// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::smart_account_tests;

use iota::authenticator_function;
use iota::claim_registry;
use iota::public_key;
use iota::signature_scheme;
use iota::smart_account::{Self, SmartAccount};
use iota::test_scenario::{Self, Scenario};
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

// === builder_v1 ===

#[test]
fun builder_v1_builds_mutable_account() {
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
fun builder_v1_builds_immutable_account() {
    let mut scenario = test_scenario::begin(@0x0);

    let authenticator = test_authenticator();
    let addr = smart_account::builder_v1(authenticator, scenario.ctx()).build_immutable_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_immutable<SmartAccount>();

    assert_eq(account.account_address(), addr);
    assert_ref_eq(account.borrow_auth_function_ref_v1(), &authenticator);

    test_scenario::return_immutable(account);
    scenario.end();
}

// === builtin_auth_builder_v1 ===

#[test]
fun builtin_auth_builder_v1_attaches_public_key_and_authenticator() {
    account_test!(|account| {
        assert_eq(account.has_builtin_auth_public_key(), true);
        assert_ref_eq(
            account.borrow_builtin_auth_public_key(),
            &ed25519_public_key(),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::EUnsupportedSignatureScheme)]
fun builtin_auth_builder_v1_aborts_on_unsupported_scheme() {
    let mut scenario = test_scenario::begin(@0x0);

    let unsupported_scheme = signature_scheme::from_flag_for_testing(0x04);
    let public_key = public_key::create_for_testing(unsupported_scheme, x"00");
    smart_account::builtin_auth_builder_v1(public_key, scenario.ctx()).build_v1();

    scenario.end();
}

// === claim_builder_v1 ===

#[test]
fun claim_builder_v1_account_address_matches_sender() {
    let public_key = ed25519_public_key();
    let sender = public_key.to_iota_address();

    claim_account_test!(sender, |registry, scenario| {
        let addr = smart_account::claim_builder_v1(
            registry,
            public_key,
            scenario.ctx(),
        ).build_v1();
        assert_eq(addr, sender);
    });
}

#[test]
#[expected_failure(abort_code = iota::claim_registry::EAddressMismatch)]
fun claim_builder_v1_aborts_on_address_mismatch() {
    let public_key = ed25519_public_key();

    claim_account_test!(@0x1, |registry, scenario| {
        smart_account::claim_builder_v1(registry, public_key, scenario.ctx()).build_v1();
    });
}

#[test]
#[expected_failure(abort_code = iota::claim_registry::EAlreadyClaimed)]
fun claim_builder_v1_aborts_on_double_claim() {
    let public_key = ed25519_public_key();
    let sender = public_key.to_iota_address();

    claim_account_test!(sender, |registry, scenario| {
        smart_account::claim_builder_v1(registry, public_key, scenario.ctx()).build_v1();
        smart_account::claim_builder_v1(registry, public_key, scenario.ctx()).build_v1();
    });
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

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun borrow_builtin_auth_public_key_aborts_if_missing() {
    let mut scenario = test_scenario::begin(@0x0);
    smart_account::builder_v1(test_authenticator(), scenario.ctx()).build_v1();

    scenario.next_tx(@0x0);
    let account = scenario.take_shared<SmartAccount>();

    account.borrow_builtin_auth_public_key();

    test_scenario::return_shared(account);
    scenario.end();
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

// === Admin: builtin auth public key ===

#[test]
fun attach_borrow_detach_builtin_auth_public_key_lifecycle() {
    account_test_mut!(|account, scenario| {
        assert_eq(account.has_builtin_auth_public_key(), true);

        let returned = account.detach_builtin_auth_public_key(scenario.ctx());
        assert_eq(returned, ed25519_public_key());
        assert_eq(account.has_builtin_auth_public_key(), false);

        let public_key = secp256k1_public_key();
        account.attach_builtin_auth_public_key(public_key, scenario.ctx());
        assert_eq(account.has_builtin_auth_public_key(), true);
        assert_ref_eq(account.borrow_builtin_auth_public_key(), &public_key);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun attach_builtin_auth_public_key_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.attach_builtin_auth_public_key(ed25519_public_key(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyAlreadyAttached)]
fun attach_builtin_auth_public_key_aborts_if_already_attached() {
    account_test_mut!(|account, scenario| {
        account.attach_builtin_auth_public_key(ed25519_public_key(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun detach_builtin_auth_public_key_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.detach_builtin_auth_public_key(scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun detach_builtin_auth_public_key_aborts_if_missing() {
    let mut scenario = test_scenario::begin(@0x0);
    let addr = smart_account::builder_v1(test_authenticator(), scenario.ctx()).build_v1();

    scenario.next_tx(addr);
    let mut account = scenario.take_shared<SmartAccount>();

    account.detach_builtin_auth_public_key(scenario.ctx());

    test_scenario::return_shared(account);
    scenario.end();
}

#[test]
fun rotate_builtin_auth_public_key_returns_old_and_stores_new() {
    account_test_mut!(|account, scenario| {
        let new_public_key = secp256k1_public_key();
        let returned = account.rotate_builtin_auth_public_key(new_public_key, scenario.ctx());
        assert_eq(returned, ed25519_public_key());
        assert_ref_eq(account.borrow_builtin_auth_public_key(), &new_public_key);
    });
}

#[test]
#[expected_failure(abort_code = iota::smart_account::ETransactionSenderIsNotTheSmartAccount)]
fun rotate_builtin_auth_public_key_aborts_if_sender_not_account() {
    account_test_wrong_sender!(|account, scenario| {
        account.rotate_builtin_auth_public_key(ed25519_public_key(), scenario.ctx());
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun rotate_builtin_auth_public_key_aborts_if_missing() {
    let mut scenario = test_scenario::begin(@0x0);
    let addr = smart_account::builder_v1(test_authenticator(), scenario.ctx()).build_v1();

    scenario.next_tx(addr);
    let mut account = scenario.take_shared<SmartAccount>();

    account.rotate_builtin_auth_public_key(ed25519_public_key(), scenario.ctx());

    test_scenario::return_shared(account);
    scenario.end();
}

// === Admin: authenticator ===

#[test]
fun rotate_auth_function_ref_v1_returns_old_and_stores_new() {
    account_test_mut!(|account, scenario| {
        let old_ref = *account.borrow_auth_function_ref_v1();
        let new_ref = test_authenticator();
        assert!(old_ref != new_ref);

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

// === Helpers ===

/// Creates a mutable shared `SmartAccount` backed by an ed25519 key and returns its address.
fun make_account(scenario: &mut Scenario): address {
    smart_account::builtin_auth_builder_v1(ed25519_public_key(), scenario.ctx()).build_v1()
}

fun ed25519_public_key(): public_key::PublicKey {
    public_key::create(
        signature_scheme::ed25519(),
        x"0000000000000000000000000000000000000000000000000000000000000000",
    )
}

fun secp256k1_public_key(): public_key::PublicKey {
    // Compressed secp256k1 generator point G.
    public_key::create(
        signature_scheme::secp256k1(),
        x"0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    )
}

fun test_authenticator(): authenticator_function::AuthenticatorFunctionRefV1<SmartAccount> {
    authenticator_function::create_auth_function_ref_v1_for_testing(
        @0xABC,
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

/// Creates a `ClaimRegistry`, advances to `$sender`, and runs `$f` with a mutable
/// reference to the registry and the scenario.
macro fun claim_account_test(
    $sender: address,
    $f: |&mut claim_registry::ClaimRegistry, &mut Scenario|,
) {
    let mut scenario = test_scenario::begin(@0x0);

    claim_registry::create_for_testing(scenario.ctx());

    scenario.next_tx($sender);
    let mut registry = scenario.take_shared<claim_registry::ClaimRegistry>();

    $f(&mut registry, &mut scenario);

    test_scenario::return_shared(registry);
    scenario.end();
}
