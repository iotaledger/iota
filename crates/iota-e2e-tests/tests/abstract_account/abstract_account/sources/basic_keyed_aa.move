// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::basic_keyed_aa;

use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hex::decode;

// === Errors ===

#[error(code = 0)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 1)]
const ESecp256k1VerificationFailed: vector<u8> = b"Secp256k1 authenticator verification failed.";
#[error(code = 2)]
const ESecp256r1VerificationFailed: vector<u8> = b"Secp256r1 authenticator verification failed.";

// === Constants ===

/// A dynamic field key for the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

// === Structs ===

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    signature: &vector<u8>,
    public_key: &vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    assert!(
        ed25519::ed25519_verify(&decode(*signature), public_key, ctx.digest()),
        EEd25519VerificationFailed,
    );
}

/// Secp256k1 signature authenticator.
public fun authenticate_secp256k1(
    signature: &vector<u8>,
    public_key: &vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    assert!(
        ecdsa_k1::secp256k1_verify(&decode(*signature), public_key, ctx.digest(), 0),
        ESecp256k1VerificationFailed,
    );
}

/// Secp256r1 signature authenticator.
public fun authenticate_secp256r1(
    signature: &vector<u8>,
    public_key: &vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    assert!(
        ecdsa_r1::secp256r1_verify(&decode(*signature), public_key, ctx.digest(), 0),
        ESecp256r1VerificationFailed,
    );
}

// === View Functions ===

// === Admin Functions ===

// === Package Functions ===

public(package) fun owner_public_key(): OwnerPublicKey {
    OwnerPublicKey {}
}

// === Private Functions ===

// === Test Functions ===
