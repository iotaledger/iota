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
