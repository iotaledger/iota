// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A validated, typed cryptographic public key.
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
/// `PublicKey` values are constructed exclusively through `create`, which validates the scheme
/// and the raw-byte length. Once created, the inner fields are immutable.
module iota::public_key;

use iota::signature_scheme::{Self, SignatureScheme};

// === Errors ===

#[error(code = 0)]
const EPublicKeyBytesEmpty: vector<u8> = b"Public key bytes are empty.";
#[error(code = 1)]
const EUnknownPublicKeyScheme: vector<u8> = b"Unknown public key scheme.";
#[error(code = 2)]
const EInvalidPublicKeyLength: vector<u8> = b"Invalid public key length for the given scheme.";

// === Constants ===

/// Raw-byte length for Ed25519 public keys: 32 bytes.
const ED25519_PUBLIC_KEY_LENGTH: u64 = 32;
/// Raw-byte length for secp256k1, secp256r1, and Passkey public keys:
/// 33-byte compressed point.
const SECP256_PUBLIC_KEY_LENGTH: u64 = 33;

// === Structs ===

/// A validated public key consisting of a typed `scheme` and raw key material.
public struct PublicKey has copy, drop, store {
    /// The signature scheme identifying the key algorithm.
    scheme: SignatureScheme,
    /// Raw key material without the scheme flag prefix.
    raw_bytes: vector<u8>,
}

// === Public Functions ===

/// Constructs a `PublicKey` from an explicit `scheme` and raw key `raw_bytes`.
///
/// `raw_bytes` must be the raw key material **without** the scheme flag prefix:
/// 32 bytes for Ed25519, 33 bytes (compressed) for Secp256k1 / Secp256r1 / Passkey,
/// and at least 1 byte of BCS-encoded payload for MultiSig.
///
/// Aborts if `raw_bytes` is empty, if `scheme` is not a recognized scheme, or if the
/// byte length does not match the scheme.
public fun create(scheme: SignatureScheme, raw_bytes: vector<u8>): PublicKey {
    assert!(!raw_bytes.is_empty(), EPublicKeyBytesEmpty);

    validate_length(scheme, raw_bytes.length());

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

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

fun validate_length(scheme: SignatureScheme, len: u64) {
    if (scheme == signature_scheme::ed25519()) {
        assert!(len == ED25519_PUBLIC_KEY_LENGTH, EInvalidPublicKeyLength);
    } else if (
        scheme == signature_scheme::secp256k1() ||
        scheme == signature_scheme::secp256r1() ||
        scheme == signature_scheme::passkey()
    ) {
        assert!(len == SECP256_PUBLIC_KEY_LENGTH, EInvalidPublicKeyLength);
    } else if (scheme == signature_scheme::multisig()) {
        // MultiSig key payload is BCS-encoded and variable in length.
        assert!(len > 0, EInvalidPublicKeyLength);
    } else {
        abort EUnknownPublicKeyScheme
    }
}

// === Test Functions ===
