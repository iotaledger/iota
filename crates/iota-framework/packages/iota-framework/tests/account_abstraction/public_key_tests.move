// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::public_key_tests;

use iota::public_key;
use iota::signature_scheme;
use iota::test_utils::{assert_eq, assert_ref_eq};

// === Happy-path construction ===

#[test]
fun create_ed25519_key() {
    // 32 zero bytes — raw ed25519 key material
    let raw = x"0000000000000000000000000000000000000000000000000000000000000000";
    let public_key = public_key::create(signature_scheme::ed25519(), raw);

    assert_eq(public_key.scheme(), signature_scheme::ed25519());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_secp256k1_key() {
    // 33 zero bytes — raw secp256k1 compressed point
    let raw = x"000000000000000000000000000000000000000000000000000000000000000000";
    let public_key = public_key::create(signature_scheme::secp256k1(), raw);

    assert_eq(public_key.scheme(), signature_scheme::secp256k1());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_secp256r1_key() {
    // 33 zero bytes — raw secp256r1 compressed point
    let raw = x"000000000000000000000000000000000000000000000000000000000000000000";
    let public_key = public_key::create(signature_scheme::secp256r1(), raw);

    assert_eq(public_key.scheme(), signature_scheme::secp256r1());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_passkey_key() {
    // 33 zero bytes — raw P-256 compressed point
    let raw = x"000000000000000000000000000000000000000000000000000000000000000000";
    let public_key = public_key::create(signature_scheme::passkey(), raw);

    assert_eq(public_key.scheme(), signature_scheme::passkey());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_multisig_key() {
    // 1 byte of BCS-encoded payload — minimum valid MultiSig key
    let raw = x"00";
    let public_key = public_key::create(signature_scheme::multisig(), raw);

    assert_eq(public_key.scheme(), signature_scheme::multisig());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

// === Failure: empty bytes ===

#[test]
#[expected_failure(abort_code = iota::public_key::EPublicKeyBytesEmpty)]
fun create_empty_bytes_aborts() {
    public_key::create(signature_scheme::ed25519(), x"");
}

// === Failure: unknown scheme flag ===

#[test]
#[expected_failure(abort_code = iota::public_key::EUnknownPublicKeyScheme)]
fun create_unknown_scheme_flag_aborts() {
    // 0x04 is not a recognized scheme
    let scheme = signature_scheme::from_flag_for_testing(0x04);
    public_key::create(scheme, x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EUnknownPublicKeyScheme)]
fun create_scheme_flag_0x05_aborts() {
    // 0x05 is not a recognized scheme
    let scheme = signature_scheme::from_flag_for_testing(0x05);
    public_key::create(scheme, x"00");
}

// === Failure: wrong lengths ===

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_ed25519_too_short_aborts() {
    // 1 raw byte — ed25519 requires 32
    public_key::create(signature_scheme::ed25519(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_ed25519_too_long_aborts() {
    // 33 raw bytes — one too many for ed25519 (requires 32)
    let raw = x"000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::ed25519(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_secp256k1_too_short_aborts() {
    // 1 raw byte — secp256k1 requires 33
    public_key::create(signature_scheme::secp256k1(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_secp256k1_too_long_aborts() {
    // 34 raw bytes — one too many for secp256k1 (requires 33)
    let raw = x"0000000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::secp256k1(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_secp256r1_too_short_aborts() {
    // 1 raw byte — secp256r1 requires 33
    public_key::create(signature_scheme::secp256r1(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_secp256r1_too_long_aborts() {
    // 34 raw bytes — one too many for secp256r1 (requires 33)
    let raw = x"0000000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::secp256r1(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_passkey_too_short_aborts() {
    // 1 raw byte — passkey requires 33
    public_key::create(signature_scheme::passkey(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyLength)]
fun create_passkey_too_long_aborts() {
    // 34 raw bytes — one too many for passkey (requires 33)
    let raw = x"0000000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::passkey(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EPublicKeyBytesEmpty)]
fun create_multisig_payload_missing_aborts() {
    // Empty raw bytes — MultiSig requires at least 1 byte
    public_key::create(signature_scheme::multisig(), x"");
}
