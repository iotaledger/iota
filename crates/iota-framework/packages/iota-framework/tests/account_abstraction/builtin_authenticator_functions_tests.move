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

// === Signed transaction fixtures ===

// A BCS-encoded `Transaction` and, for each built-in scheme, a public key and a signature of
// that transaction in `UserSignature` wire format. Generated with `iota-sdk-crypto`.
const TX_DATA_BYTES: vector<u8> =
    x"0000010020000000000000000000000000000000000000000000000000000000000000000001010100010000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e803000000000000102700000000000000";
// A different BCS-encoded `Transaction`; it differs from `TX_DATA_BYTES` only in its gas budget.
const OTHER_TX_DATA_BYTES: vector<u8> =
    x"0000010020000000000000000000000000000000000000000000000000000000000000000001010100010000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e803000000000000112700000000000000";
const ED25519_PUBLIC_KEY: vector<u8> =
    x"8c553335eee80b9bfa0c544a45fe63474a09dff9c4b0b33db2b662f934ea46c4";
const ED25519_SIGNATURE: vector<u8> =
    x"00649c1b109c25b515d1417fd8b47bb4aa7c34f7b4dbe38227bee2943e52d93d2306901d88d17544a4362e9c6279974e3e93a89d20b4bf1fcedb9d24bf2bc8d60a8c553335eee80b9bfa0c544a45fe63474a09dff9c4b0b33db2b662f934ea46c4";
const SECP256K1_PUBLIC_KEY: vector<u8> =
    x"0286bcc70599ebc420b3b8977ecc60e594bb56749beaa562d7f80a9bdfffcaaa1d";
const SECP256K1_SIGNATURE: vector<u8> =
    x"01bc454c2a0b358d02c7f6e8b8f4a66f09a525d252328437f0ef90e633d5d093ab5f75184f9dcff47c5fe2eaed5780f47238d7916584c1f96ad058077cde1a5b1c0286bcc70599ebc420b3b8977ecc60e594bb56749beaa562d7f80a9bdfffcaaa1d";
const SECP256R1_PUBLIC_KEY: vector<u8> =
    x"02cd32c13b418c9ffa3e5e2f7565e1c24a9681fd222a4317d36f1dcc9c0a2a9c24";
const SECP256R1_SIGNATURE: vector<u8> =
    x"025cffe6642d21d90e7e9913f3cedd7ffb3c835d158af4ffa22c8a5dd28f068a692a2c41001878d61ab93e05368f8231edc00b1a92a4eee28346ee10a8f4ca3d3702cd32c13b418c9ffa3e5e2f7565e1c24a9681fd222a4317d36f1dcc9c0a2a9c24";
const MULTISIG_PUBLIC_KEY: vector<u8> =
    x"02001b823a9a4b87863aa419fc6922e28944c29a1677852490ced5625dd9b319f280010102f769f618d113c8e5798a81d1f67f381ed46b34cc151ee873078cb82517368b91010100";
const MULTISIG_SIGNATURE: vector<u8> =
    x"0301008299a1183eddec121c12ad648bc9a1dedaebb49226195acbc1603d9785cfef17ef67c35559ccf5a935bc370c0643e76a810d10adbb45d9ac6d99deed7d45e00c010002001b823a9a4b87863aa419fc6922e28944c29a1677852490ced5625dd9b319f280010102f769f618d113c8e5798a81d1f67f381ed46b34cc151ee873078cb82517368b91010100";
const PASSKEY_PUBLIC_KEY: vector<u8> =
    x"02eaecc5054b198215cc1c02f5e8694cdaf53bedc6cdee122a3189f76cb70568d2";
const PASSKEY_SIGNATURE: vector<u8> =
    x"0601ab81017b2274797065223a22776562617574686e2e676574222c226368616c6c656e6765223a2276676c3464306d4c6f34446939357539457144444c656d44495754784367484f4a67493266496f374f6359222c226f726967696e223a2268747470733a2f2f696f74612e6f7267222c2263726f73734f726967696e223a66616c73657d6202c12da0eb39aea4172afea070b0f5adb96453b58a0715e690f275512f9be00fd82499664ebb4dbc9ab900de59b3d4ccb52efb4c821a6673a2050ab630990fa88002eaecc5054b198215cc1c02f5e8694cdaf53bedc6cdee122a3189f76cb70568d2";

const DIGEST: vector<u8> = x"0000000000000000000000000000000000000000000000000000000000000000";

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
    assert_ref_eq(ref.function_name(), &ascii::string(b"ed25519_authenticator_v1"));
}

#[test]
fun secp256k1_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::secp256k1_authenticator_function_ref_v1<
        TestAccount,
    >();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"secp256k1_authenticator_v1"));
}

#[test]
fun secp256r1_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::secp256r1_authenticator_function_ref_v1<
        TestAccount,
    >();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"secp256r1_authenticator_v1"));
}

#[test]
fun multisig_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::multisig_authenticator_function_ref_v1<
        TestAccount,
    >();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"multisig_authenticator_v1"));
}

#[test]
fun passkey_auth_function_ref_has_correct_fields() {
    let ref = builtin_authenticator_functions::passkey_authenticator_function_ref_v1<TestAccount>();

    assert_eq(ref.package(), object::id_from_address(@0x2));
    assert_ref_eq(ref.module_name(), &ascii::string(b"builtin_authenticator_functions"));
    assert_ref_eq(ref.function_name(), &ascii::string(b"passkey_authenticator_v1"));
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

// === Built-in authenticators ===

// --- ed25519 ---

#[test]
fun ed25519_authenticator_accepts_valid_signature() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun ed25519_authenticator_rejects_tampered_signature() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            tampered(ED25519_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun ed25519_authenticator_rejects_malformed_signature() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            malformed(ED25519_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun ed25519_authenticator_rejects_signature_by_another_key() {
    authenticator_test!(
        signature_scheme::ed25519(),
        *ed25519_public_key().raw_bytes(),
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun ed25519_authenticator_rejects_signature_of_another_scheme() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            SECP256K1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun ed25519_authenticator_rejects_invalid_transaction_bytes() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, x"00", |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun ed25519_authenticator_rejects_signature_of_another_transaction() {
    authenticator_test!(
        signature_scheme::ed25519(),
        ED25519_PUBLIC_KEY,
        OTHER_TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeySchemeMismatch)]
fun ed25519_authenticator_aborts_on_public_key_scheme_mismatch() {
    authenticator_test!(
        signature_scheme::secp256k1(),
        SECP256K1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun ed25519_authenticator_aborts_without_public_key() {
    account_test!(|account| {
        builtin_authenticator_functions::ed25519_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

// --- secp256k1 ---

#[test]
fun secp256k1_authenticator_accepts_valid_signature() {
    authenticator_test!(
        signature_scheme::secp256k1(),
        SECP256K1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            SECP256K1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256k1_authenticator_rejects_tampered_signature() {
    authenticator_test!(
        signature_scheme::secp256k1(),
        SECP256K1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            tampered(SECP256K1_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256k1_authenticator_rejects_malformed_signature() {
    authenticator_test!(
        signature_scheme::secp256k1(),
        SECP256K1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            malformed(SECP256K1_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256k1_authenticator_rejects_signature_by_another_key() {
    authenticator_test!(
        signature_scheme::secp256k1(),
        *secp256k1_public_key().raw_bytes(),
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            SECP256K1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256k1_authenticator_rejects_signature_of_another_scheme() {
    authenticator_test!(
        signature_scheme::secp256k1(),
        SECP256K1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256k1_authenticator_rejects_invalid_transaction_bytes() {
    authenticator_test!(signature_scheme::secp256k1(), SECP256K1_PUBLIC_KEY, x"00", |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            SECP256K1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256k1_authenticator_rejects_signature_of_another_transaction() {
    authenticator_test!(
        signature_scheme::secp256k1(),
        SECP256K1_PUBLIC_KEY,
        OTHER_TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            SECP256K1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeySchemeMismatch)]
fun secp256k1_authenticator_aborts_on_public_key_scheme_mismatch() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            SECP256K1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun secp256k1_authenticator_aborts_without_public_key() {
    account_test!(|account| {
        builtin_authenticator_functions::secp256k1_authenticator_v1_for_testing(
            account,
            SECP256K1_SIGNATURE,
        );
    });
}

// --- secp256r1 ---

#[test]
fun secp256r1_authenticator_accepts_valid_signature() {
    authenticator_test!(
        signature_scheme::secp256r1(),
        SECP256R1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            SECP256R1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256r1_authenticator_rejects_tampered_signature() {
    authenticator_test!(
        signature_scheme::secp256r1(),
        SECP256R1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            tampered(SECP256R1_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256r1_authenticator_rejects_malformed_signature() {
    authenticator_test!(
        signature_scheme::secp256r1(),
        SECP256R1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            malformed(SECP256R1_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256r1_authenticator_rejects_signature_by_another_key() {
    authenticator_test!(
        signature_scheme::secp256r1(),
        PASSKEY_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            SECP256R1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256r1_authenticator_rejects_signature_of_another_scheme() {
    authenticator_test!(
        signature_scheme::secp256r1(),
        SECP256R1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256r1_authenticator_rejects_invalid_transaction_bytes() {
    authenticator_test!(signature_scheme::secp256r1(), SECP256R1_PUBLIC_KEY, x"00", |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            SECP256R1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun secp256r1_authenticator_rejects_signature_of_another_transaction() {
    authenticator_test!(
        signature_scheme::secp256r1(),
        SECP256R1_PUBLIC_KEY,
        OTHER_TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            SECP256R1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeySchemeMismatch)]
fun secp256r1_authenticator_aborts_on_public_key_scheme_mismatch() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            SECP256R1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun secp256r1_authenticator_aborts_without_public_key() {
    account_test!(|account| {
        builtin_authenticator_functions::secp256r1_authenticator_v1_for_testing(
            account,
            SECP256R1_SIGNATURE,
        );
    });
}

// --- multisig ---

#[test]
fun multisig_authenticator_accepts_valid_signature() {
    authenticator_test!(
        signature_scheme::multisig(),
        MULTISIG_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            MULTISIG_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun multisig_authenticator_rejects_tampered_signature() {
    authenticator_test!(
        signature_scheme::multisig(),
        MULTISIG_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            tampered(MULTISIG_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun multisig_authenticator_rejects_malformed_signature() {
    authenticator_test!(
        signature_scheme::multisig(),
        MULTISIG_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            malformed(MULTISIG_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun multisig_authenticator_rejects_signature_by_another_key() {
    authenticator_test!(
        signature_scheme::multisig(),
        other_multisig_public_key(),
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            MULTISIG_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun multisig_authenticator_rejects_signature_of_another_scheme() {
    authenticator_test!(
        signature_scheme::multisig(),
        MULTISIG_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            ED25519_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun multisig_authenticator_rejects_invalid_transaction_bytes() {
    authenticator_test!(signature_scheme::multisig(), MULTISIG_PUBLIC_KEY, x"00", |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            MULTISIG_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun multisig_authenticator_rejects_signature_of_another_transaction() {
    authenticator_test!(
        signature_scheme::multisig(),
        MULTISIG_PUBLIC_KEY,
        OTHER_TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            MULTISIG_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeySchemeMismatch)]
fun multisig_authenticator_aborts_on_public_key_scheme_mismatch() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            MULTISIG_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun multisig_authenticator_aborts_without_public_key() {
    account_test!(|account| {
        builtin_authenticator_functions::multisig_authenticator_v1_for_testing(
            account,
            MULTISIG_SIGNATURE,
        );
    });
}

// --- passkey ---

#[test]
fun passkey_authenticator_accepts_valid_signature() {
    authenticator_test!(signature_scheme::passkey(), PASSKEY_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            PASSKEY_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun passkey_authenticator_rejects_tampered_signature() {
    authenticator_test!(signature_scheme::passkey(), PASSKEY_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            tampered(PASSKEY_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun passkey_authenticator_rejects_malformed_signature() {
    authenticator_test!(signature_scheme::passkey(), PASSKEY_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            malformed(PASSKEY_SIGNATURE),
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun passkey_authenticator_rejects_signature_by_another_key() {
    authenticator_test!(
        signature_scheme::passkey(),
        SECP256R1_PUBLIC_KEY,
        TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            PASSKEY_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun passkey_authenticator_rejects_signature_of_another_scheme() {
    authenticator_test!(signature_scheme::passkey(), PASSKEY_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            SECP256R1_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun passkey_authenticator_rejects_invalid_transaction_bytes() {
    authenticator_test!(signature_scheme::passkey(), PASSKEY_PUBLIC_KEY, x"00", |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            PASSKEY_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EInvalidSignature)]
fun passkey_authenticator_rejects_signature_of_another_transaction() {
    authenticator_test!(
        signature_scheme::passkey(),
        PASSKEY_PUBLIC_KEY,
        OTHER_TX_DATA_BYTES,
        |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            PASSKEY_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeySchemeMismatch)]
fun passkey_authenticator_aborts_on_public_key_scheme_mismatch() {
    authenticator_test!(signature_scheme::ed25519(), ED25519_PUBLIC_KEY, TX_DATA_BYTES, |account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            PASSKEY_SIGNATURE,
        );
    });
}

#[test]
#[expected_failure(abort_code = iota::builtin_authenticator_functions::EPublicKeyMissing)]
fun passkey_authenticator_aborts_without_public_key() {
    account_test!(|account| {
        builtin_authenticator_functions::passkey_authenticator_v1_for_testing(
            account,
            PASSKEY_SIGNATURE,
        );
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

/// Runs `$f` on an account with the `$scheme` public key `$raw_public_key` attached, while
/// `$tx_data_bytes` is the transaction being authenticated.
macro fun authenticator_test(
    $scheme: signature_scheme::SignatureScheme,
    $raw_public_key: vector<u8>,
    $tx_data_bytes: vector<u8>,
    $f: |&TestAccount|,
) {
    let mut scenario = test_scenario::begin(@0x0);
    let mut account = TestAccount { id: object::new(scenario.ctx()) };
    builtin_authenticator_functions::attach_public_key(
        account.id_mut(),
        public_key::create($scheme, $raw_public_key),
    );
    set_tx_data_bytes($tx_data_bytes);

    $f(&account);

    iota::test_utils::destroy(account);
    scenario.end();
}

fun set_tx_data_bytes(tx_data_bytes: vector<u8>) {
    auth_context::new_for_testing(
        DIGEST,
        vector[],
        vector[],
        tx_data_bytes,
        DIGEST,
        option::none(),
        option::none(),
        option::none(),
    );
}

/// Returns `signature` with its middle byte flipped.
fun tampered(mut signature: vector<u8>): vector<u8> {
    let middle = signature.length() / 2;
    *&mut signature[middle] = signature[middle] ^ 0xff;
    signature
}

/// Returns the `MULTISIG_PUBLIC_KEY` committee with its threshold raised from 1 to 2, which
/// derives a different address.
fun other_multisig_public_key(): vector<u8> {
    let mut committee = MULTISIG_PUBLIC_KEY;
    // The BCS-encoded threshold is the trailing little-endian `u16`.
    let threshold_index = committee.length() - 2;
    *&mut committee[threshold_index] = 2;
    committee
}

/// Returns a signature with the flag of `signature` and an unparsable payload.
fun malformed(signature: vector<u8>): vector<u8> {
    vector[signature[0], 0xab]
}
