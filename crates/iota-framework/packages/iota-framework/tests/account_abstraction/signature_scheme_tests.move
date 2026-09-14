// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::signature_scheme_tests;

use iota::signature_scheme;
use iota::test_utils::assert_eq;

#[test]
fun ed25519_flag_is_zero() {
    assert_eq(signature_scheme::ed25519().flag(), 0x00);
}

#[test]
fun secp256k1_flag_is_one() {
    assert_eq(signature_scheme::secp256k1().flag(), 0x01);
}

#[test]
fun secp256r1_flag_is_two() {
    assert_eq(signature_scheme::secp256r1().flag(), 0x02);
}

#[test]
fun multisig_flag_is_three() {
    assert_eq(signature_scheme::multisig().flag(), 0x03);
}

#[test]
fun passkey_flag_is_six() {
    assert_eq(signature_scheme::passkey().flag(), 0x06);
}

// === from_flag happy paths ===

#[test]
fun from_flag_ed25519() {
    assert_eq(signature_scheme::from_flag(0x00), signature_scheme::ed25519());
}

#[test]
fun from_flag_secp256k1() {
    assert_eq(signature_scheme::from_flag(0x01), signature_scheme::secp256k1());
}

#[test]
fun from_flag_secp256r1() {
    assert_eq(signature_scheme::from_flag(0x02), signature_scheme::secp256r1());
}

#[test]
fun from_flag_multisig() {
    assert_eq(signature_scheme::from_flag(0x03), signature_scheme::multisig());
}

#[test]
fun from_flag_passkey() {
    assert_eq(signature_scheme::from_flag(0x06), signature_scheme::passkey());
}

// === from_flag error path ===

#[test]
#[expected_failure(abort_code = signature_scheme::EUnknownScheme)]
fun from_flag_unknown_aborts() {
    signature_scheme::from_flag(0xff);
}
