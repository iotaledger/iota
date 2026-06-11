// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::builtin_authenticator_functions_tests;

use iota::builtin_authenticator_functions;
use iota::public_key;
use iota::signature_scheme;
use iota::test_scenario;
use iota::test_utils::{assert_eq, assert_ref_eq};
use std::ascii;

// Used as a stand-in account type throughout the tests.
public struct TestAccount has key {
    id: UID,
}

fun id(self: &TestAccount): &UID { &self.id }

fun id_mut(self: &mut TestAccount): &mut UID { &mut self.id }

// === Authenticator function ref construction ===

#[test]
fun ed25519_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::ed25519_authenticator_function_ref_v1<TestAccount>();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"ed25519_authenticator_function_ref_v1"));
}

#[test]
fun secp256k1_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<
        TestAccount,
    >();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"secp256k1_authenticator_function_ref_v1"));
}

#[test]
fun secp256r1_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::secp256r1_authenticator_function_ref_v1<
        TestAccount,
    >();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"secp256r1_authenticator_function_ref_v1"));
}

#[test]
fun multisig_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::multisig_authenticator_function_ref_v1<
        TestAccount,
    >();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"multisig_authenticator_function_ref_v1"));
}

#[test]
fun passkey_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::passkey_authenticator_function_ref_v1<TestAccount>();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"passkey_authenticator_function_ref_v1"));
}

// === from_signature_scheme ===

#[test]
fun from_signature_scheme_returns_correct_ref_for_all_supported_schemes() {
    assert_eq(
        builtin_authenticator_functions::from_signature_scheme<TestAccount>(
            signature_scheme::ed25519(),
        ),
        builtin_authenticator_functions::ed25519_authenticator_function_ref_v1<TestAccount>(),
    );
    assert_eq(
        builtin_authenticator_functions::from_signature_scheme<TestAccount>(
            signature_scheme::secp256k1(),
        ),
        builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<TestAccount>(),
    );
    assert_eq(
        builtin_authenticator_functions::from_signature_scheme<TestAccount>(
            signature_scheme::secp256r1(),
        ),
        builtin_authenticator_functions::secp256r1_authenticator_function_ref_v1<TestAccount>(),
    );
    assert_eq(
        builtin_authenticator_functions::from_signature_scheme<TestAccount>(
            signature_scheme::multisig(),
        ),
        builtin_authenticator_functions::multisig_authenticator_function_ref_v1<TestAccount>(),
    );
    assert_eq(
        builtin_authenticator_functions::from_signature_scheme<TestAccount>(
            signature_scheme::passkey(),
        ),
        builtin_authenticator_functions::passkey_authenticator_function_ref_v1<TestAccount>(),
    );
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EUnsupportedSignatureScheme)]
fun from_signature_scheme_aborts_on_unsupported_scheme() {
    let unsupported_scheme = signature_scheme::from_flag_for_testing(0x04);
    builtin_authenticator_functions::from_signature_scheme<TestAccount>(unsupported_scheme);
}

// === attach_public_key / has_public_key / borrow_public_key / detach_public_key ===

#[test]
fun attach_borrow_detach_lifecycle() {
    account_test_mut!(|account| {
        let public_key = ed25519_public_key();
        assert_eq(builtin_authenticator_functions::has_public_key(account.id()), false);

        builtin_authenticator_functions::attach_public_key(account.id_mut(), public_key);

        assert_eq(builtin_authenticator_functions::has_public_key(account.id()), true);
        assert_ref_eq(
            builtin_authenticator_functions::borrow_public_key(account.id()),
            &public_key,
        );

        let returned = builtin_authenticator_functions::detach_public_key(account.id_mut());

        assert_eq(returned, public_key);
        assert_eq(builtin_authenticator_functions::has_public_key(account.id()), false);
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyAlreadyAttached)]
fun attach_twice_aborts() {
    account_test_mut!(|account| {
        builtin_authenticator_functions::attach_public_key(account.id_mut(), ed25519_public_key());
        builtin_authenticator_functions::attach_public_key(account.id_mut(), ed25519_public_key());
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun borrow_without_attach_aborts() {
    account_test!(|account| {
        builtin_authenticator_functions::borrow_public_key(account.id());
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun detach_without_attach_aborts() {
    account_test_mut!(|account| {
        builtin_authenticator_functions::detach_public_key(account.id_mut());
    });
}

// === rotate_public_key ===

#[test]
fun rotate_returns_old_key_and_stores_new() {
    account_test_mut!(|account| {
        let old_public_key = ed25519_public_key();
        let new_public_key = secp256k1_public_key();

        builtin_authenticator_functions::attach_public_key(account.id_mut(), old_public_key);
        let returned = builtin_authenticator_functions::rotate_public_key(
            account.id_mut(),
            new_public_key,
        );

        assert_eq(returned, old_public_key);
        assert_ref_eq(
            builtin_authenticator_functions::borrow_public_key(account.id()),
            &new_public_key,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun rotate_without_attach_aborts() {
    account_test_mut!(|account| {
        builtin_authenticator_functions::rotate_public_key(account.id_mut(), ed25519_public_key());
    });
}

// === Helpers ===

fun ed25519_public_key(): public_key::PublicKey {
    // 32 zero bytes — raw ed25519 key material
    public_key::create(
        signature_scheme::ed25519(),
        x"0000000000000000000000000000000000000000000000000000000000000000",
    )
}

fun secp256k1_public_key(): public_key::PublicKey {
    // 33 zero bytes — raw secp256k1 compressed point
    public_key::create(
        signature_scheme::secp256k1(),
        x"000000000000000000000000000000000000000000000000000000000000000000",
    )
}

macro fun account_test($f: |&TestAccount|) {
    let mut scenario = test_scenario::begin(@0x0);
    let account = TestAccount { id: object::new(scenario.ctx()) };

    $f(&account);

    iota::test_utils::destroy(account);
    scenario.end();
}

macro fun account_test_mut($f: |&mut TestAccount|) {
    let mut scenario = test_scenario::begin(@0x0);
    let mut account = TestAccount { id: object::new(scenario.ctx()) };

    $f(&mut account);

    iota::test_utils::destroy(account);
    scenario.end();
}
