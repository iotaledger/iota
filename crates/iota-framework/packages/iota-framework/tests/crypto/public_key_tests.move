// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::public_key_tests;

use iota::public_key::{Self, PublicKey};
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
    let raw = x"02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
    let public_key = public_key::create(signature_scheme::secp256k1(), raw);

    assert_eq(public_key.scheme(), signature_scheme::secp256k1());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_secp256r1_key() {
    let raw = x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
    let public_key = public_key::create(signature_scheme::secp256r1(), raw);

    assert_eq(public_key.scheme(), signature_scheme::secp256r1());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_passkey_key() {
    let raw = x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
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
    // BCS-encoded MultiSigPublicKey: 1-of-1 with one Secp256k1 signer, weight=1, threshold=1.
    // Layout: vec_len(1) | tag(1=Secp256k1) | key(33B) | weight(1) | threshold_le(1)
    let raw = x"010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100";
    let public_key = public_key::create(signature_scheme::multisig(), raw);

    assert_eq(public_key.scheme(), signature_scheme::multisig());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_multisig_key_multiple_signers() {
    // BCS-encoded MultiSigPublicKey: 1-of-2 with a Secp256k1 and a Secp256r1 signer, each
    // weight=1, threshold=1. Layout: vec_len(2) | Secp256k1-entry | Secp256r1-entry | threshold_le(1)
    // Uses the same key material as the MULTISIG_SECP256K1_SECP256R1_PK constant (minus the 0x03
    // scheme prefix).
    let raw =
        x"020102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c01020227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a010100";
    let public_key = public_key::create(signature_scheme::multisig(), raw);

    assert_eq(public_key.scheme(), signature_scheme::multisig());
    assert_ref_eq(public_key.raw_bytes(), &raw);
}

#[test]
fun create_multisig_key_secp256k1_member_not_on_curve() {
    // Multisig with a Secp256k1 member whose bytes are all-zero (prefix 0x00 is invalid). Member
    // keys are not checked against their curves, so the key is created.
    // vec_len(1) | tag(1=Secp256k1) | 33 zero bytes | weight(1) | threshold_le(1)
    let raw = x"0101000000000000000000000000000000000000000000000000000000000000000000010100";
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
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_ed25519_too_short_aborts() {
    // 1 raw byte — ed25519 requires 32
    public_key::create(signature_scheme::ed25519(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_ed25519_too_long_aborts() {
    // 33 raw bytes — one too many for ed25519 (requires 32)
    let raw = x"000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::ed25519(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_secp256k1_too_short_aborts() {
    // 1 raw byte — secp256k1 requires 33
    public_key::create(signature_scheme::secp256k1(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_secp256k1_too_long_aborts() {
    // 34 raw bytes — one too many for secp256k1 (requires 33)
    let raw = x"0000000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::secp256k1(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_secp256r1_too_short_aborts() {
    // 1 raw byte — secp256r1 requires 33
    public_key::create(signature_scheme::secp256r1(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_secp256r1_too_long_aborts() {
    // 34 raw bytes — one too many for secp256r1 (requires 33)
    let raw = x"0000000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::secp256r1(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_passkey_too_short_aborts() {
    // 1 raw byte — passkey requires 33
    public_key::create(signature_scheme::passkey(), x"00");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_passkey_too_long_aborts() {
    // 34 raw bytes — one too many for passkey (requires 33)
    let raw = x"0000000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::passkey(), raw);
}

// === Failure: MultiSig structural validation ===

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_empty_signers_aborts() {
    // vec_len(0) | threshold_le16(1) — deserializes to a committee with no signers, which is
    // rejected (at least one signer is required).
    public_key::create(signature_scheme::multisig(), x"000100");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_too_many_signers_aborts() {
    // BCS-encoded MultiSigPublicKey with 11 Ed25519 signers (max is 10), each weight=1,
    // threshold=1. vec_len=11 (0x0b), followed by 11 × (tag=0 | 32 zero bytes | weight=1)
    let raw =
        x"0b00000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010100";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_zero_threshold_aborts() {
    // Same as the valid 1-of-1 Ed25519 layout but threshold_le = 0x0000
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000010000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_weight_below_threshold_aborts() {
    // 1 signer with weight=1, threshold=2 — total weight (1) < threshold (2)
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000010200";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_zero_weight_signer_aborts() {
    // The two-scheme 1-of-2 committee with the Secp256k1 member's weight set to 0. Total weight
    // (1, from the Secp256r1 member) still meets the threshold, so only the zero-weight rule is
    // violated. Zero-weight members are rejected.
    let raw =
        x"020102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c00020227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a010100";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_mixed_ed25519_signers_aborts() {
    // 1-of-2 with an Ed25519 and a Secp256k1 signer, each weight=1, threshold=1. Ed25519 members
    // carry no scheme flag in the address preimage, so two different committees mixing Ed25519
    // with other schemes can derive the same address; such committees are rejected.
    // Layout: vec_len(2) | Ed25519-entry | Secp256k1-entry | threshold_le(1)
    let raw =
        x"0200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_duplicate_signer_aborts() {
    // 2 identical Ed25519 signers (same 32 zero bytes), each weight=1, threshold=1. Duplicate
    // member keys are rejected.
    // Layout: vec_len(2) | Ed25519-entry | Ed25519-entry (same key) | threshold_le(1)
    let raw =
        x"0200000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000010100";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_trailing_bytes_aborts() {
    // Valid 1-of-1 Ed25519 payload followed by a spurious trailing byte (0xff)
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000010100ff";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_unknown_sub_key_scheme_aborts() {
    // 1 signer using BCS enum tag 3 (ZkLoginDeprecated — no key bytes, not allowed), weight=1,
    // threshold=1
    let raw = x"0103010100";
    public_key::create(signature_scheme::multisig(), raw);
}

// === Failure: MultiSig truncated / malformed BCS ===

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_truncated_signer_count_aborts() {
    // Byte 0x80 has the variable-length integer continuation bit set, so the signer count is
    // incomplete and deserialization fails.
    public_key::create(signature_scheme::multisig(), x"80");
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_inner_ed25519_key_too_short_aborts() {
    // 1 signer with Ed25519 tag but only 31 key bytes (32 required).
    // Layout: vec_len(1) | tag(0=Ed25519) | 31 zero bytes
    let raw = x"010000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_inner_secp256k1_key_too_short_aborts() {
    // 1 signer with Secp256k1 tag but only 32 key bytes (33 required).
    // Layout: vec_len(1) | tag(1=Secp256k1) | 32 zero bytes
    let raw = x"01010000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_missing_weight_byte_aborts() {
    // 1 signer with a complete Ed25519 key (32 bytes) but no weight byte following it.
    // Layout: vec_len(1) | tag(0=Ed25519) | 32 zero bytes
    let raw = x"01000000000000000000000000000000000000000000000000000000000000000000";
    public_key::create(signature_scheme::multisig(), raw);
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_multisig_incomplete_threshold_aborts() {
    // 1 signer with a complete Ed25519 key and weight byte, but only 1 of the 2 threshold
    // bytes present.
    // Layout: vec_len(1) | tag(0=Ed25519) | 32 zero bytes | weight(1) | 1 threshold byte
    let raw = x"010000000000000000000000000000000000000000000000000000000000000000000101";
    public_key::create(signature_scheme::multisig(), raw);
}

// === Failure: curve point validation ===

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_ed25519_invalid_curve_point_aborts() {
    // y = 2 in little-endian: x² = (4-1)/(4d+1) mod p is not a quadratic residue,
    // so no valid x exists for this y value — not a valid Ed25519 point.
    public_key::create(
        signature_scheme::ed25519(),
        x"0200000000000000000000000000000000000000000000000000000000000000",
    );
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_secp256k1_invalid_curve_point_aborts() {
    // Prefix byte 0x00 is not a valid compressed-point prefix (must be 0x02 or 0x03).
    public_key::create(
        signature_scheme::secp256k1(),
        x"000000000000000000000000000000000000000000000000000000000000000000",
    );
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_secp256r1_invalid_curve_point_aborts() {
    public_key::create(
        signature_scheme::secp256r1(),
        x"000000000000000000000000000000000000000000000000000000000000000000",
    );
}

#[test]
#[expected_failure(abort_code = iota::public_key::EInvalidPublicKeyBytes)]
fun create_passkey_invalid_curve_point_aborts() {
    public_key::create(
        signature_scheme::passkey(),
        x"000000000000000000000000000000000000000000000000000000000000000000",
    );
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
// 1-of-2 MultiSig: Secp256k1 (weight=1) + Secp256r1 (weight=1), threshold=1.
// [0x03] | vec_len(2) | tag(1) | 33-byte secp256k1 key | weight(1) | tag(2) | 33-byte secp256r1 key | weight(1) | threshold_le16(1)
const MULTISIG_SECP256K1_SECP256R1_PK: vector<u8> =
    x"03020102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c01020227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a010100";

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
const MULTISIG_SECP256K1_SECP256R1_ADDR: address =
    @0x16cc5f64810dd2ca27d4bf4a884ffd3570800349238b403c072fd6ca6a9a4c97;

#[test]
fun to_iota_address_vectors() {
    assert_eq(public_key::from_prefixed_bytes(ED25519_PK).to_iota_address(), ED25519_ADDR);
    assert_eq(public_key::from_prefixed_bytes(SECP256K1_PK).to_iota_address(), SECP256K1_ADDR);
    assert_eq(public_key::from_prefixed_bytes(SECP256R1_PK).to_iota_address(), SECP256R1_ADDR);
    assert_eq(public_key::from_prefixed_bytes(PASSKEY_PK).to_iota_address(), PASSKEY_ADDR);
    // 1-of-1 Ed25519 MultiSig: Ed25519 members have no scheme-flag prefix in the hash input.
    assert_eq(public_key::from_prefixed_bytes(MULTISIG_PK).to_iota_address(), MULTISIG_ADDR);
    // Secp256k1 + Secp256r1: pins the scheme-flag prefix of both flagged member types.
    assert_eq(
        public_key::from_prefixed_bytes(MULTISIG_SECP256K1_SECP256R1_PK).to_iota_address(),
        MULTISIG_SECP256K1_SECP256R1_ADDR,
    );
}

// === verify_personal_message_signature ===

// For each scheme, a public key and its signature of `PM_MESSAGE` as an IOTA personal message, in
// `UserSignature` wire format (`UserSignature::to_bytes`). The values are fixed, not generated by
// the test, so that a change in the bytes the SDK signs or the node decodes makes these tests fail.
// They were generated with `iota-sdk-crypto` as follows:
// - keys: `random_with` on one `rand::rngs::StdRng::from_seed([0; 32])`, in this order: Ed25519,
//   the other Ed25519, Secp256k1, Secp256r1, Passkey (a Secp256r1 key), then the Secp256k1 and the
//   Secp256r1 member of the MultiSig committee;
// - Ed25519, Secp256k1 and Secp256r1 signatures: `IotaSigner::sign_personal_message` of
//   `PersonalMessage(PM_MESSAGE)`;
// - Passkey signature: the Passkey key signs `authenticatorData || sha256(clientDataJSON)`, with
//   `authenticatorData = 0xab` and `clientDataJSON =
//   {"type":"webauthn.get","challenge":"<base64url of the signing digest>","origin":"https://iota.org","crossOrigin":false}`,
//   as `passkey_signature` does in the `builtin_authenticator_functions` tests of `iota-types`;
// - MultiSig signature: the Secp256k1 member's signature of the signing digest, aggregated with
//   `MultisigAggregatedSignature::new`.
const PM_MESSAGE: vector<u8> = b"a personal message";
const PM_OTHER_MESSAGE: vector<u8> = b"another personal message";
const PM_ED25519_PUBLIC_KEY: vector<u8> =
    x"b9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
const PM_ED25519_SIGNATURE: vector<u8> =
    x"0086edf5916d3fbc37293e074ff92ec93320918395514480a969f1a47ee6d23d5df0d3fe65ddd80e98b81f70b6d51172aa6c2e5f3f02d24c152883f265f135770bb9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
const PM_OTHER_ED25519_PUBLIC_KEY: vector<u8> =
    x"aef3f4a4b8eca1dfc343361bf8e436bd42de9259c04b8314eb8e2054dd6e82ab";
// The Ed25519 key signs the bytes of `PM_MESSAGE` with the transaction intent.
const PM_ED25519_TRANSACTION_INTENT_SIGNATURE: vector<u8> =
    x"004edd7b3eb8c39dbb9f66ccb67d54bd35fdbfe94ba9de657694ee83ad0a6e3b19225320a350ba32936435a0509427ab085eff93866ecfb26831addec35f88130cb9c6ee1630ef3e711144a648db06bbb2284f7274cfbee53ffcee503cc1a49200";
const PM_SECP256K1_PUBLIC_KEY: vector<u8> =
    x"03688466442a134ee312299bafb37058e385c98dd6005eaaf0f538f533efe5f91f";
const PM_SECP256K1_SIGNATURE: vector<u8> =
    x"014981d5621ea4e7db372f5148a1357a49e7a57c6ac10739279a71a0c5a02efe2f0373160b70d509e35e06befb2a12c9abebfbd10420db24fd073c01e1e2a215d703688466442a134ee312299bafb37058e385c98dd6005eaaf0f538f533efe5f91f";
const PM_SECP256R1_PUBLIC_KEY: vector<u8> =
    x"0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
const PM_SECP256R1_SIGNATURE: vector<u8> =
    x"02c8405e30af49dffeb8646f5eb4fd1a2eab44a50ae6868dde8a7b789a4a24daac69681a91832b7a5bfee556a26e5423cc68b0230e18a1e40ec011b91fbcba641b0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";
const PM_PASSKEY_PUBLIC_KEY: vector<u8> =
    x"0298526d75fb0bb1c30b68eb6908dc8ee10036b2ebf75ba267ae35c740778036f3";
// The challenge in `clientDataJSON` is the signing digest of the personal message.
const PM_PASSKEY_SIGNATURE: vector<u8> =
    x"0601ab81017b2274797065223a22776562617574686e2e676574222c226368616c6c656e6765223a226a464f77734a66325077625a36386d64675031527941393358316e55473930714c646c7753325f494b4b49222c226f726967696e223a2268747470733a2f2f696f74612e6f7267222c2263726f73734f726967696e223a66616c73657d6202afd3e627d60b6d58a37d04b3e0dbb2f225d6e3ae8d852ed12d62466114e7feb045be9be529f5ff61341f69ba8086ed03f7534cefa1137a689bfe870b55df22350298526d75fb0bb1c30b68eb6908dc8ee10036b2ebf75ba267ae35c740778036f3";
// A 1-of-2 committee of a Secp256k1 and a Secp256r1 member, signed by the Secp256k1 member.
const PM_MULTISIG_PUBLIC_KEY: vector<u8> =
    x"0201029a857208e280d56d008894e7088a4e059cccc28efed3cab1c06b0cfbe3df3526010203921eb1f7a86f64513dd89e479f137d63685809668443d53cfa529ec8a808c6bf010100";
const PM_MULTISIG_SIGNATURE: vector<u8> =
    x"030101a842d6417618a108e4c7d22c7912ff9098cd3607f025584ca679fb0313571806042273132a3cac39e876de4a7f8e067578fdf0cb5adf1989cff3c9e9dcbee02301000201029a857208e280d56d008894e7088a4e059cccc28efed3cab1c06b0cfbe3df3526010203921eb1f7a86f64513dd89e479f137d63685809668443d53cfa529ec8a808c6bf010100";

#[test]
fun verify_personal_message_signature_ed25519() {
    let public_key = public_key::create(signature_scheme::ed25519(), PM_ED25519_PUBLIC_KEY);

    assert_eq(verify_pm_message(&public_key, PM_ED25519_SIGNATURE), true);
}

#[test]
fun verify_personal_message_signature_secp256k1() {
    let public_key = public_key::create(signature_scheme::secp256k1(), PM_SECP256K1_PUBLIC_KEY);

    assert_eq(verify_pm_message(&public_key, PM_SECP256K1_SIGNATURE), true);
}

#[test]
fun verify_personal_message_signature_secp256r1() {
    let public_key = public_key::create(signature_scheme::secp256r1(), PM_SECP256R1_PUBLIC_KEY);

    assert_eq(verify_pm_message(&public_key, PM_SECP256R1_SIGNATURE), true);
}

#[test]
fun verify_personal_message_signature_passkey() {
    let public_key = public_key::create(signature_scheme::passkey(), PM_PASSKEY_PUBLIC_KEY);

    assert_eq(verify_pm_message(&public_key, PM_PASSKEY_SIGNATURE), true);
}

#[test]
fun verify_personal_message_signature_rejects_another_key() {
    let other_public_key = public_key::create(
        signature_scheme::ed25519(),
        PM_OTHER_ED25519_PUBLIC_KEY,
    );

    assert_eq(verify_pm_message(&other_public_key, PM_ED25519_SIGNATURE), false);
}

#[test]
fun verify_personal_message_signature_rejects_another_message() {
    let public_key = public_key::create(signature_scheme::ed25519(), PM_ED25519_PUBLIC_KEY);

    let signature = PM_ED25519_SIGNATURE;
    let other_message = PM_OTHER_MESSAGE;
    assert_eq(
        public_key.verify_personal_message_signature(&signature, &other_message),
        false,
    );
}

#[test]
fun verify_personal_message_signature_rejects_transaction_intent_signature() {
    let public_key = public_key::create(signature_scheme::ed25519(), PM_ED25519_PUBLIC_KEY);

    // The same key signs the same bytes of `PM_MESSAGE`, but with the transaction intent instead
    // of the personal message intent: a check that also accepted the transaction intent would
    // return `true` here and pass every other test.
    assert_eq(
        verify_pm_message(&public_key, PM_ED25519_TRANSACTION_INTENT_SIGNATURE),
        false,
    );
}

#[test]
fun verify_personal_message_signature_rejects_another_scheme() {
    // The signature is Ed25519 but the public key is Secp256k1.
    let public_key = public_key::create(signature_scheme::secp256k1(), PM_SECP256K1_PUBLIC_KEY);

    assert_eq(verify_pm_message(&public_key, PM_ED25519_SIGNATURE), false);
}

#[test]
fun verify_personal_message_signature_rejects_malformed_bytes() {
    let public_key = public_key::create(signature_scheme::ed25519(), PM_ED25519_PUBLIC_KEY);

    assert_eq(verify_pm_message(&public_key, vector[]), false);
    assert_eq(verify_pm_message(&public_key, x"abcdef"), false);
    // The Ed25519 signature without its last byte.
    let mut truncated = PM_ED25519_SIGNATURE;
    truncated.pop_back();
    assert_eq(verify_pm_message(&public_key, truncated), false);
}

#[test]
fun verify_personal_message_signature_rejects_multisig_key() {
    let public_key = public_key::create(signature_scheme::multisig(), PM_MULTISIG_PUBLIC_KEY);

    assert_eq(verify_pm_message(&public_key, PM_MULTISIG_SIGNATURE), false);
}

/// Checks `signature` as a signature of `PM_MESSAGE` by `public_key`.
fun verify_pm_message(public_key: &PublicKey, signature: vector<u8>): bool {
    let message = PM_MESSAGE;
    public_key.verify_personal_message_signature(&signature, &message)
}
