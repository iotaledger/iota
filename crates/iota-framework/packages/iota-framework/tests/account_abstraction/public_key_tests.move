// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::public_key_tests;

use iota::bcs;
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
fun create_multisig_key_ed25519_signer() {
    // BCS-encoded MultiSigPublicKey: 1-of-1 with one Ed25519 signer (32 zero bytes), weight=1,
    // threshold=1. Layout: vec_len(1) | tag(0=Ed25519) | key(32B) | weight(1) | threshold_le(1)
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000010100";
    let public_key = public_key::create(signature_scheme::multisig(), raw);

    assert_eq(public_key.scheme(), signature_scheme::multisig());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_multisig_key_secp256k1_signer() {
    // BCS-encoded MultiSigPublicKey: 1-of-1 with one Secp256k1 signer (33 zero bytes), weight=1,
    // threshold=1. Layout: vec_len(1) | tag(1=Secp256k1) | key(33B) | weight(1) | threshold_le(1)
    let raw = x"0101000000000000000000000000000000000000000000000000000000000000000000010100";
    let public_key = public_key::create(signature_scheme::multisig(), raw);

    assert_eq(public_key.scheme(), signature_scheme::multisig());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_multisig_key_multiple_signers() {
    // BCS-encoded MultiSigPublicKey: 1-of-2 with an Ed25519 and a Secp256k1 signer, each
    // weight=1, threshold=1. Layout: vec_len(2) | Ed25519-entry | Secp256k1-entry | threshold_le(1)
    let raw =
        x"020000000000000000000000000000000000000000000000000000000000000000000101000000000000000000000000000000000000000000000000000000000000000000010100";
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

// === Failure: MultiSig BCS structure ===

#[test]
#[expected_failure(abort_code = iota::public_key::EMultiSigEmptySigners)]
fun create_multisig_empty_signers_aborts() {
    // BCS vec_len=0 — at least one signer is required
    public_key::create(signature_scheme::multisig(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EMultiSigTooManySigners)]
fun create_multisig_too_many_signers_aborts() {
    // BCS-encoded MultiSigPublicKey with 11 Ed25519 signers (max is 10), each weight=1,
    // threshold=1. vec_len=11 (0x0b), followed by 11 × (tag=0 | 32 zero bytes | weight=1)
    let raw =
        x"0b00000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010100";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EMultiSigZeroThreshold)]
fun create_multisig_zero_threshold_aborts() {
    // Same as the valid 1-of-1 Ed25519 layout but threshold_le = 0x0000
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000010000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EMultiSigWeightBelowThreshold)]
fun create_multisig_weight_below_threshold_aborts() {
    // 1 signer with weight=1, threshold=2 — total weight (1) < threshold (2)
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000010200";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EMultiSigTrailingBytes)]
fun create_multisig_trailing_bytes_aborts() {
    // Valid 1-of-1 Ed25519 payload followed by a spurious trailing byte (0xff)
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000010100ff";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EUnknownPublicKeyScheme)]
fun create_multisig_unknown_sub_key_scheme_aborts() {
    // 1 signer using BCS enum tag 3 (ZkLoginDeprecated — no key bytes, not allowed), weight=1,
    // threshold=1
    let raw = x"0103010100";
    public_key::create(signature_scheme::multisig(), raw);
}

// === Failure: MultiSig truncated BCS peel ===

#[test]
#[expected_failure(vector_error, minor_status = 2, location = iota::bcs)]
fun create_multisig_truncated_signer_count_aborts() {
    // Byte 0x80 has the variable-length integer continuation bit set, so peel_vec_length
    // expects a second byte to complete the signer count — but none exists. This exhausts
    // on the very first peel in validate_multisig_public_key.
    public_key::create(signature_scheme::multisig(), x"80");
}

// === Failure: MultiSig inner key byte count ===

#[test]
#[expected_failure(abort_code = bcs::EOutOfRange)]
fun create_multisig_inner_ed25519_key_too_short_aborts() {
    // 1 signer with Ed25519 tag but only 31 key bytes (32 required); BCS reader exhausts
    // on the 32nd peel inside the inner key loop.
    // Layout: vec_len(1) | tag(0=Ed25519) | 31 zero bytes
    let raw = x"010000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = bcs::EOutOfRange)]
fun create_multisig_inner_secp256k1_key_too_short_aborts() {
    // 1 signer with Secp256k1 tag but only 32 key bytes (33 required); BCS reader exhausts
    // on the 33rd peel inside the inner key loop.
    // Layout: vec_len(1) | tag(1=Secp256k1) | 32 zero bytes
    let raw = x"01010000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = bcs::EOutOfRange)]
fun create_multisig_missing_weight_byte_aborts() {
    // 1 signer with a complete Ed25519 key (32 bytes) but no weight byte following it.
    // Layout: vec_len(1) | tag(0=Ed25519) | 32 zero bytes
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = bcs::EOutOfRange)]
fun create_multisig_incomplete_threshold_aborts() {
    // 1 signer with a complete Ed25519 key and weight byte, but only 1 of the 2 threshold
    // bytes present; peel_u16 exhausts on the second byte.
    // Layout: vec_len(1) | tag(0=Ed25519) | 32 zero bytes | weight(1) | 1 threshold byte
    let raw = x"010000000000000000000000000000000000000000000000000000000000000000000101";
    public_key::create(signature_scheme::multisig(), raw);
}

// === from_prefixed_bytes — happy paths ===

// Same key material used throughout; addresses cross-checked against the Rust node.
const ED25519_PK: vector<u8> =
    x"00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
const SECP256K1_PK: vector<u8> =
    x"0102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
const SECP256R1_PK: vector<u8> =
    x"020227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
const PASSKEY_PK: vector<u8> =
    x"060227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
// 1-of-1 Ed25519 MultiSig: [0x03] | vec_len(1) | tag(0) | 32-byte key | weight(1) | threshold_le16(1)
const MULTISIG_PK: vector<u8> =
    x"030100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100";
// Mixed 1-of-2 MultiSig: Ed25519 (weight=1) + Secp256k1 (weight=1), threshold=1.
// [0x03] | vec_len(2) | tag(0) | 32-byte ed25519 key | weight(1) | tag(1) | 33-byte secp256k1 key | weight(1) | threshold_le16(1)
const MULTISIG_MIXED_PK: vector<u8> =
    x"030200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100";

#[test]
fun from_prefixed_bytes_ed25519() {
    let pk = public_key::from_prefixed_bytes(ED25519_PK);
    assert_eq(pk.scheme(), signature_scheme::ed25519());
    assert_ref_eq(
        pk.raw_bytes(),
        &x"cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88",
    );
}

#[test]
fun from_prefixed_bytes_secp256k1() {
    let pk = public_key::from_prefixed_bytes(SECP256K1_PK);
    assert_eq(pk.scheme(), signature_scheme::secp256k1());
    assert_ref_eq(
        pk.raw_bytes(),
        &x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c",
    );
}

#[test]
fun from_prefixed_bytes_secp256r1() {
    let pk = public_key::from_prefixed_bytes(SECP256R1_PK);
    assert_eq(pk.scheme(), signature_scheme::secp256r1());
    assert_ref_eq(
        pk.raw_bytes(),
        &x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a",
    );
}

#[test]
fun from_prefixed_bytes_passkey() {
    let pk = public_key::from_prefixed_bytes(PASSKEY_PK);
    assert_eq(pk.scheme(), signature_scheme::passkey());
    assert_ref_eq(
        pk.raw_bytes(),
        &x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a",
    );
}

#[test]
fun from_prefixed_bytes_multisig() {
    let pk = public_key::from_prefixed_bytes(MULTISIG_PK);
    assert_eq(pk.scheme(), signature_scheme::multisig());
    assert_ref_eq(
        pk.raw_bytes(),
        &x"0100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100",
    );
}

// === from_prefixed_bytes — failure paths ===

#[test]
#[expected_failure(abort_code = iota::public_key::EPublicKeyBytesEmpty)]
fun from_prefixed_bytes_empty_aborts() {
    public_key::from_prefixed_bytes(x"");
}

#[test]
#[expected_failure(abort_code = iota::signature_scheme::EUnknownScheme)]
fun from_prefixed_bytes_unknown_flag_aborts() {
    // 0xff is not a recognized scheme flag
    public_key::from_prefixed_bytes(
        x"ffcc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88",
    );
}

// === to_iota_address — cross-checked against Rust-computed vectors ===

// Expected IOTA addresses computed independently by the Rust node (Blake2b256 via fastcrypto).
const ED25519_ADDR: address = @0xcef6bafea1d59edb73ff5ec9e8aa58354796e1b572b695d64237ce9c15a34a03;
const SECP256K1_ADDR: address = @0x2fecbdf2652b089c64d127158d388621fdbbd156533fbcca5a0082aa0d2939fa;
const SECP256R1_ADDR: address = @0x318f591092f10b67a81963954fb9539ea3919444417726be4e1b95ce44fe2fc0;
const PASSKEY_ADDR: address = @0xa2f90cd2552d45ab5ba157dacf19597e2018108c6a80e4d7a4a5680d1542a7e8;
const MULTISIG_ADDR: address = @0x1cc23b51b2e3c8641eea35b29114a53ad7a76643dcb2763d12290a7b83cac525;
const MULTISIG_MIXED_ADDR: address =
    @0x2e6c30799340fef9d382542ff0cad8e2a20f766da8b71a25c2443eda658104e4;

#[test]
fun to_iota_address_vectors() {
    assert_eq(public_key::from_prefixed_bytes(ED25519_PK).to_iota_address(), ED25519_ADDR);
    assert_eq(public_key::from_prefixed_bytes(SECP256K1_PK).to_iota_address(), SECP256K1_ADDR);
    assert_eq(public_key::from_prefixed_bytes(SECP256R1_PK).to_iota_address(), SECP256R1_ADDR);
    assert_eq(public_key::from_prefixed_bytes(PASSKEY_PK).to_iota_address(), PASSKEY_ADDR);
    // 1-of-1 Ed25519 MultiSig: Ed25519 members have no scheme-flag prefix in the hash input.
    assert_eq(public_key::from_prefixed_bytes(MULTISIG_PK).to_iota_address(), MULTISIG_ADDR);
    // Mixed Ed25519 + Secp256k1: pins the per-scheme flag behavior for both member types.
    assert_eq(public_key::from_prefixed_bytes(MULTISIG_MIXED_PK).to_iota_address(), MULTISIG_MIXED_ADDR);
}
