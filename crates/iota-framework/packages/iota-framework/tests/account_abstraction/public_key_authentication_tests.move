// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::public_key_authentication_tests;

use iota::public_key;
use iota::public_key_authentication;
use iota::signature_scheme;
use iota::test_scenario;
use iota::test_utils::{assert_eq, assert_ref_eq};

// Used as a stand-in account type throughout the tests.
public struct TestAccount has key {
    id: UID,
}

fun id(self: &TestAccount): &UID { &self.id }

fun id_mut(self: &mut TestAccount): &mut UID { &mut self.id }

// === Attaching a key to an account ===

#[test]
fun attach_borrow_detach_lifecycle() {
    account_test_mut!(|account| {
        let public_key = ed25519_public_key();
        assert_eq(public_key_authentication::has_public_key(account.id()), false);

        public_key_authentication::attach_public_key(account.id_mut(), public_key);

        assert_eq(public_key_authentication::has_public_key(account.id()), true);
        assert_ref_eq(
            public_key_authentication::borrow_public_key(account.id()),
            &public_key,
        );

        let returned = public_key_authentication::detach_public_key(account.id_mut());

        assert_eq(returned, public_key);
        assert_eq(public_key_authentication::has_public_key(account.id()), false);
    });
}

#[test]
#[expected_failure(abort_code = iota::public_key_authentication::EPublicKeyAlreadyAttached)]
fun attach_twice_aborts() {
    account_test_mut!(|account| {
        public_key_authentication::attach_public_key(account.id_mut(), ed25519_public_key());
        public_key_authentication::attach_public_key(account.id_mut(), ed25519_public_key());
    });
}

#[test]
#[expected_failure(abort_code = iota::public_key_authentication::EPublicKeyMissing)]
fun borrow_without_attach_aborts() {
    account_test!(|account| {
        public_key_authentication::borrow_public_key(account.id());
    });
}

#[test]
#[expected_failure(abort_code = iota::public_key_authentication::EPublicKeyMissing)]
fun detach_without_attach_aborts() {
    account_test_mut!(|account| {
        public_key_authentication::detach_public_key(account.id_mut());
    });
}

// === rotate_public_key ===

#[test]
fun rotate_returns_old_key_and_stores_new() {
    account_test_mut!(|account| {
        let old_public_key = ed25519_public_key();
        let new_public_key = secp256k1_public_key();

        public_key_authentication::attach_public_key(account.id_mut(), old_public_key);
        let returned = public_key_authentication::rotate_public_key(
            account.id_mut(),
            new_public_key,
        );

        assert_eq(returned, old_public_key);
        assert_ref_eq(
            public_key_authentication::borrow_public_key(account.id()),
            &new_public_key,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::public_key_authentication::EPublicKeyMissing)]
fun rotate_without_attach_aborts() {
    account_test_mut!(|account| {
        public_key_authentication::rotate_public_key(account.id_mut(), ed25519_public_key());
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
    public_key::create(
        signature_scheme::secp256k1(),
        x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c",
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
