// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module defines a set of authenticators helpers that use public key cryptography to implement
/// authenticators.
///
/// It allows to set a public key as field of an account, such that any authenticator can authenticate
/// the account by verifying a signature against the public key.
/// The public key schemes implemented in this module are:
/// - ed25519
/// - secp256k1
/// - secp256r1
module public_key_authentication::public_key_authentication;

use iota::dynamic_field as df;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;

// === Errors ===

#[error(code = 0)]
const EPublicKeyAlreadyAttached: vector<u8> = b"Public key already attached.";
#[error(code = 1)]
const EPublicKeyMissing: vector<u8> = b"Public key missing.";
#[error(code = 2)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 3)]
const ESecp256k1VerificationFailed: vector<u8> = b"Secp256k1 authenticator verification failed.";
#[error(code = 4)]
const ESecp256r1VerificationFailed: vector<u8> = b"Secp256r1 authenticator verification failed.";

// === Constants ===

// === Structs ===

/// A dynamic field name for the account owner public key.
public struct PublicKeyFieldName has copy, drop, store {}

// === Public Functions ===

/// Attach public key data to the account with the provided `public_key`.
public fun attach_public_key(account_id: &mut UID, public_key: vector<u8>) {
    assert!(!has_public_key(account_id), EPublicKeyAlreadyAttached);

    df::add(account_id, PublicKeyFieldName {}, public_key)
}

/// Detach public key data from the account.
public fun detach_public_key(account_id: &mut UID): vector<u8> {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    df::remove(account_id, PublicKeyFieldName {})
}

/// Update the public key attached to the account.
public fun rotate_public_key(account_id: &mut UID, public_key: vector<u8>): vector<u8> {
    assert!(has_public_key(account_id), EPublicKeyMissing);

    let prev_public_key = df::remove(account_id, PublicKeyFieldName {});
    df::add(account_id, PublicKeyFieldName {}, public_key);
    prev_public_key
}

// === Public Authenticators Helpers ===

/// Ed25519 signature authenticator helper.
public fun authenticate_ed25519(account_id: &UID, signature: vector<u8>, ctx: &TxContext) {
    assert!(has_public_key(account_id), EPublicKeyMissing);
    assert!(
        ed25519::ed25519_verify(&signature, borrow_public_key(account_id), ctx.digest()),
        EEd25519VerificationFailed,
    );
}

/// Secp256k1 signature authenticator helper.
public fun authenticate_secp256k1(account_id: &UID, signature: vector<u8>, ctx: &TxContext) {
    assert!(has_public_key(account_id), EPublicKeyMissing);
    assert!(
        ecdsa_k1::secp256k1_verify(&signature, borrow_public_key(account_id), ctx.digest(), 0),
        ESecp256k1VerificationFailed,
    );
}

/// Secp256r1 signature authenticator helper.
public fun authenticate_secp256r1(account_id: &UID, signature: vector<u8>, ctx: &TxContext) {
    assert!(has_public_key(account_id), EPublicKeyMissing);
    assert!(
        ecdsa_r1::secp256r1_verify(&signature, borrow_public_key(account_id), ctx.digest(), 0),
        ESecp256r1VerificationFailed,
    );
}

// === View Functions ===

/// An utility function to check if the account has a public key set.
public fun has_public_key(account_id: &UID): bool {
    df::exists_(account_id, PublicKeyFieldName {})
}

/// An utility function to borrow the account-related public key.
public fun borrow_public_key(account_id: &UID): &vector<u8> {
    df::borrow(account_id, PublicKeyFieldName {})
}

// === Admin Functions ===

// === Package Functions ===

/// An utility function to construct the dynamic field name for the public key field.
public(package) fun public_key_field_name(): PublicKeyFieldName {
    PublicKeyFieldName {}
}

// === Private Functions ===

// === Test Functions ===
