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
/// the BCS structure is well-formed (signer count, weights, threshold), each member's key is a
/// valid curve point, every signer weight is non-zero, and no signer is duplicated. These
/// MultiSig rules match the canonical Rust verifier, so a key accepted here is guaranteed to be
/// authenticatable rather than bricking the account at verification time.
module iota::public_key;

use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::multisig;
use iota::signature_scheme::{Self, SignatureScheme};

// === Errors ===

#[error(code = 0)]
const EPublicKeyBytesEmpty: vector<u8> = b"Public key bytes are empty.";
#[error(code = 1)]
const EUnknownPublicKeyScheme: vector<u8> = b"Unknown public key scheme.";
#[error(code = 2)]
const EInvalidPublicKeyBytes: vector<u8> = b"Public key bytes are not a valid point on the curve.";

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
/// BCS-encoded `MultiSigPublicKey` for MultiSig (1–10 distinct signers, each weight > 0,
/// threshold > 0, total weight ≥ threshold).
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
/// and a valid BCS-encoded `MultiSigPublicKey` for MultiSig (1–10 distinct signers, each
/// weight > 0, threshold > 0, total weight ≥ threshold).
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

/// Derives the IOTA address for this public key, mirroring the node's `IotaAddress::from`.
///
/// See `to_iota_address_impl` for the exact per-scheme derivation rules.
public fun to_iota_address(self: &PublicKey): address {
    to_iota_address_impl(self.scheme.flag(), &self.raw_bytes)
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

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
        assert!(multisig::multisig_validate_pubkey(raw_bytes), EInvalidPublicKeyBytes);
    } else {
        abort EUnknownPublicKeyScheme
    }
}

/// Derives the IOTA address for a `flag`-typed public key with raw key material `raw_bytes`
/// (the key bytes **without** the scheme flag prefix), mirroring the node's address derivation:
///   Ed25519:   Blake2b256(pk)
///   Secp256k1: Blake2b256(0x01 || pk)
///   Secp256r1: Blake2b256(0x02 || pk)
///   MultiSig:  Blake2b256(0x03 || threshold_le16 || member*) where each Ed25519 member
///              contributes `pk || weight` and each other member contributes
///              `scheme_flag || pk || weight` (Ed25519 omits its flag — IOTA legacy rule)
///   Passkey:   Blake2b256(0x06 || pk)
///
/// Aborts if `flag` is not a recognized scheme or `raw_bytes` is not a valid public key for it.
native fun to_iota_address_impl(flag: u8, raw_bytes: &vector<u8>): address;

// === Test Functions ===

#[test_only]
public fun derive_address_for_testing(prefixed_bytes: &vector<u8>): address {
    from_prefixed_bytes(*prefixed_bytes).to_iota_address()
}
