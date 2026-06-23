// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A typed cryptographic public key.
///
/// A `PublicKey` stores the signature scheme and the raw key material.
///
/// Raw-byte lengths by scheme:
///
/// | Scheme     | Flag | Raw-byte length       |
/// |------------|------|-----------------------|
/// | Ed25519    | 0x00 | 32 bytes              |
/// | Secp256k1  | 0x01 | 33 bytes (compressed) |
/// | Secp256r1  | 0x02 | 33 bytes (compressed) |
/// | MultiSig   | 0x03 | variable (BCS)        |
/// | Passkey    | 0x06 | 33 bytes (compressed) |
///
/// `PublicKey` values are constructed exclusively through `create`. Once created,
/// the inner fields are immutable.
///
/// **Validation scope**: `create` checks that the scheme is recognized, that the raw bytes form
/// a valid curve point (which implicitly rejects wrong-length inputs), and — for MultiSig — that
/// the BCS structure is well-formed (signer count, weights, threshold) and each member's key
/// is a valid curve point.
module iota::public_key;

use iota::address as iota_address;
use iota::bcs;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hash;
use iota::signature_scheme::{Self, SignatureScheme};

// === Errors ===

#[error(code = 0)]
const EPublicKeyBytesEmpty: vector<u8> = b"Public key bytes are empty.";
#[error(code = 1)]
const EUnknownPublicKeyScheme: vector<u8> = b"Unknown public key scheme.";
#[error(code = 2)]
const EInvalidPublicKeyBytes: vector<u8> = b"Public key bytes are not a valid point on the curve.";

#[error(code = 10)]
const EMultiSigEmptySigners: vector<u8> = b"MultiSig public key must have at least one signer.";
#[error(code = 11)]
const EMultiSigTooManySigners: vector<u8> = b"MultiSig signer count exceeds the maximum of 10.";
#[error(code = 12)]
const EMultiSigZeroThreshold: vector<u8> = b"MultiSig threshold must be greater than zero.";
#[error(code = 13)]
const EMultiSigWeightBelowThreshold: vector<u8> =
    b"MultiSig total weight is less than the threshold.";
#[error(code = 14)]
const EMultiSigTrailingBytes: vector<u8> =
    b"MultiSig public key bytes contain unexpected trailing data.";

// === Constants ===

/// Raw-byte length for Ed25519 public keys: 32 bytes.
const ED25519_PUBLIC_KEY_LENGTH: u64 = 32;
/// Raw-byte length for secp256k1, secp256r1, and Passkey public keys:
/// 33-byte compressed point.
const SECP256_PUBLIC_KEY_LENGTH: u64 = 33;

/// Maximum number of signers in a MultiSig public key.
const MAX_MULTISIG_SIGNERS: u64 = 10;

/// BCS variant indices of Rust's `iota_types::crypto::PublicKey` enum, used when
/// deserializing inner public keys inside a MultiSig payload.
const MULTISIG_KEY_TAG_ED25519: u32 = 0;
const MULTISIG_KEY_TAG_SECP256K1: u32 = 1;
const MULTISIG_KEY_TAG_SECP256R1: u32 = 2;
// Variant 3 = ZkLoginDeprecated (unit variant, no key bytes; not allowed in MultiSig).
const MULTISIG_KEY_TAG_PASSKEY: u32 = 4;

// === Structs ===

/// A validated public key consisting of a typed `scheme` and raw key material.
public struct PublicKey has copy, drop, store {
    /// The signature scheme identifying the key algorithm.
    scheme: SignatureScheme,
    /// Raw key material without the scheme flag prefix.
    raw_bytes: vector<u8>,
}

// === Public Functions ===

/// Constructs a `PublicKey` from a `scheme`-prefixed byte vector.
///
/// The first byte of `prefixed_bytes` is the scheme flag; the remaining bytes are the raw
/// key material. Byte lengths after stripping the flag:
/// 32 bytes for Ed25519, 33 bytes (compressed) for Secp256k1 / Secp256r1 / Passkey,
/// BCS-encoded `MultiSigPublicKey` for MultiSig (1–10 signers, threshold > 0, total weight ≥ threshold).
///
/// Aborts if `prefixed_bytes` is empty, if the flag byte is not a recognized scheme, if the
/// remaining bytes are not a valid curve point, or if a MultiSig payload fails structural validation.
public fun from_prefixed_bytes(mut prefixed_bytes: vector<u8>): PublicKey {
    assert!(!prefixed_bytes.is_empty(), EPublicKeyBytesEmpty);
    let flag = prefixed_bytes.remove(0);
    create(signature_scheme::from_flag(flag), prefixed_bytes)
}

/// Constructs a `PublicKey` from an explicit `scheme` and raw key `raw_bytes`.
///
/// `raw_bytes` must be the raw key material **without** the scheme flag prefix:
/// 32 bytes for Ed25519, 33 bytes (compressed) for Secp256k1 / Secp256r1 / Passkey,
/// and a valid BCS-encoded `MultiSigPublicKey` for MultiSig (1–10 signers, threshold > 0,
/// total weight ≥ threshold).
///
/// Aborts if `raw_bytes` is empty, if `scheme` is not a recognized scheme, if the bytes do not
/// form a valid curve point, or if a MultiSig payload fails structural validation or contains
/// an invalid member key.
public fun create(scheme: SignatureScheme, raw_bytes: vector<u8>): PublicKey {
    assert!(!raw_bytes.is_empty(), EPublicKeyBytesEmpty);

    validate_public_key(scheme, &raw_bytes);

    PublicKey { scheme, raw_bytes }
}

// === View Functions ===

/// Returns the `SignatureScheme` that identifies this key's signature algorithm.
public fun scheme(self: &PublicKey): SignatureScheme {
    self.scheme
}

/// Returns a reference to the raw key bytes (without the scheme flag prefix).
public fun raw_bytes(self: &PublicKey): &vector<u8> {
    &self.raw_bytes
}

/// Derives the IOTA address for this public key, mirroring Rust `IotaAddress::from(&PublicKey)`:
///   Ed25519:   Blake2b256(pubkey)
///   Secp256k1: Blake2b256([0x01] || pubkey)
///   Secp256r1: Blake2b256([0x02] || pubkey)
///   MultiSig:  Blake2b256([0x03] || threshold_le16 || member*) where each Ed25519 member
///              contributes `pk || weight` and each other member contributes
///              `scheme_flag || pk || weight` (Ed25519 has no flag prefix — IOTA legacy rule)
///   Passkey:   Blake2b256([0x06] || pubkey)
public fun to_iota_address(self: &PublicKey): address {
    let scheme = self.scheme;
    let raw = self.raw_bytes;
    let data = if (scheme == signature_scheme::ed25519()) {
        raw
    } else if (scheme == signature_scheme::multisig()) {
        multisig_to_hash_input(raw)
    } else {
        let mut v = vector[scheme.flag()];
        v.append(raw);
        v
    };
    iota_address::from_bytes(hash::blake2b256(&data))
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

/// Creates the bytes used to derive an address from a multisig PublicKey.
/// Result: `flag_MultiSig || threshold_le16 || member_1 || ... || member_n`
/// where each Ed25519 member is `pk || weight` and each other member is
/// `scheme_flag || pk || weight`. Ed25519 omits the flag prefix, mirroring
/// `SignatureScheme::update_hasher_with_flag` in the Rust node.
fun multisig_to_hash_input(mut raw_bytes: vector<u8>): vector<u8> {
    let threshold_high = raw_bytes.pop_back();
    let threshold_low = raw_bytes.pop_back();
    let mut bcs = bcs::new(raw_bytes);
    let num_signers = bcs.peel_vec_length();
    // flag_MultiSig || threshold_le16 || member*
    let mut data = vector[signature_scheme::multisig().flag(), threshold_low, threshold_high];
    num_signers.do!(|_| {
        let tag = bcs.peel_enum_tag();
        // Ed25519 follows the IOTA legacy address rule: no scheme flag in hash input.
        let key_len = if (tag == MULTISIG_KEY_TAG_ED25519) {
            ED25519_PUBLIC_KEY_LENGTH
        } else if (tag == MULTISIG_KEY_TAG_SECP256K1) {
            data.push_back(signature_scheme::secp256k1().flag());
            SECP256_PUBLIC_KEY_LENGTH
        } else if (tag == MULTISIG_KEY_TAG_SECP256R1) {
            data.push_back(signature_scheme::secp256r1().flag());
            SECP256_PUBLIC_KEY_LENGTH
        } else if (tag == MULTISIG_KEY_TAG_PASSKEY) {
            data.push_back(signature_scheme::passkey().flag());
            SECP256_PUBLIC_KEY_LENGTH
        } else abort EUnknownPublicKeyScheme;
        key_len.do!(|_| {
            data.push_back(bcs.peel_u8());
        });
        data.push_back(bcs.peel_u8());
    });
    data
}

fun validate_public_key(scheme: SignatureScheme, raw_bytes: &vector<u8>) {
    if (scheme == signature_scheme::ed25519()) {
        assert!(ed25519::ed25519_validate_pubkey(raw_bytes), EInvalidPublicKeyBytes);
    } else if (scheme == signature_scheme::secp256k1()) {
        assert!(ecdsa_k1::secp256k1_validate_pubkey(raw_bytes), EInvalidPublicKeyBytes);
    } else if (
        scheme == signature_scheme::secp256r1() ||
        scheme == signature_scheme::passkey()
    ) {
        assert!(ecdsa_r1::secp256r1_validate_pubkey(raw_bytes), EInvalidPublicKeyBytes);
    } else if (scheme == signature_scheme::multisig()) {
        validate_multisig_public_key(raw_bytes);
    } else {
        abort EUnknownPublicKeyScheme
    }
}

fun validate_multisig_public_key(raw_bytes: &vector<u8>) {
    let mut bcs = bcs::new(*raw_bytes);

    let num_signers = bcs.peel_vec_length();

    assert!(num_signers >= 1, EMultiSigEmptySigners);
    assert!(num_signers <= MAX_MULTISIG_SIGNERS, EMultiSigTooManySigners);

    let mut total_weight = 0;
    let mut i = 0;

    while (i < num_signers) {
        let tag = bcs.peel_enum_tag();

        let key_len = if (tag == MULTISIG_KEY_TAG_ED25519) {
            ED25519_PUBLIC_KEY_LENGTH
        } else if (tag == MULTISIG_KEY_TAG_SECP256K1 ||
            tag == MULTISIG_KEY_TAG_SECP256R1 ||
            tag == MULTISIG_KEY_TAG_PASSKEY) {
            SECP256_PUBLIC_KEY_LENGTH
        } else {
            abort EUnknownPublicKeyScheme
        };

        let mut key_bytes = vector[];
        let mut j = 0;
        while (j < key_len) {
            key_bytes.push_back(bcs.peel_u8());
            j = j + 1;
        };

        if (tag == MULTISIG_KEY_TAG_ED25519) {
            assert!(ed25519::ed25519_validate_pubkey(&key_bytes), EInvalidPublicKeyBytes);
        } else if (tag == MULTISIG_KEY_TAG_SECP256K1) {
            assert!(ecdsa_k1::secp256k1_validate_pubkey(&key_bytes), EInvalidPublicKeyBytes);
        } else {
            // SECP256R1 and PASSKEY both use the secp256r1 curve
            assert!(ecdsa_r1::secp256r1_validate_pubkey(&key_bytes), EInvalidPublicKeyBytes);
        };

        let weight = bcs.peel_u8() as u64;

        total_weight = total_weight + weight;
        i = i + 1;
    };

    let threshold = bcs.peel_u16() as u64;

    assert!(threshold > 0, EMultiSigZeroThreshold);
    assert!(total_weight >= threshold, EMultiSigWeightBelowThreshold);
    assert!(bcs.into_remainder_bytes().is_empty(), EMultiSigTrailingBytes);
}

// === Test Functions ===

#[test_only]
public fun derive_address_for_testing(prefixed_bytes: &vector<u8>): address {
    from_prefixed_bytes(*prefixed_bytes).to_iota_address()
}
