// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module generic_keyed_authentication::owner_public_key;

use iota::dynamic_field;
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
#[error(code = 3)]
const EPublicKeyAttached: vector<u8> = b"Public key already attached.";
#[error(code = 4)]
const EPublicKeyMissing: vector<u8> = b"Public key missing.";

// === Constants ===

// === Structs ===

/// A dynamic field key for the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

// === Public-View Functions ===

// === Admin Functions ===

// === Public-Package Functions ===

// === Private Functions ===

// === Test Functions ===

public fun attach(account_id: &mut UID, public_key: vector<u8>) {
    assert!(!has(account_id), EPublicKeyAttached);

    dynamic_field::add(account_id, OwnerPublicKey {}, public_key);
}

public fun detach(account_id: &mut UID): vector<u8> {
    assert!(has(account_id), EPublicKeyMissing);

    dynamic_field::remove(account_id, OwnerPublicKey {})
}

public fun rotate(account_id: &mut UID, public_key: vector<u8>): vector<u8> {
    assert!(has(account_id), EPublicKeyMissing);

    let prev_public_key = dynamic_field::remove(account_id, OwnerPublicKey {});
    dynamic_field::add(account_id, OwnerPublicKey {}, public_key);
    prev_public_key
}

public fun has(account_id: &UID): bool {
    dynamic_field::exists_(account_id, OwnerPublicKey {})
}

public fun borrow(account_id: &UID): &vector<u8> {
    dynamic_field::borrow(account_id, OwnerPublicKey {})
}

public fun authenticate_ed25519_signature(
    account_id: &UID,
    signature: vector<u8>,
    message: &vector<u8>,
) {
    assert!(has(account_id), EPublicKeyMissing);

    assert!(
        ed25519::ed25519_verify(&decode(signature), borrow(account_id), message),
        EEd25519VerificationFailed,
    );
}

/// Secp256k1 signature authenticator.
public fun authenticate_secp256k1(account_id: &UID, signature: vector<u8>, message: &vector<u8>) {
    assert!(has(account_id), EPublicKeyMissing);

    // Check the signature.
    assert!(
        ecdsa_k1::secp256k1_verify(
            &decode(signature),
            borrow(account_id),
            message,
            0,
        ),
        ESecp256k1VerificationFailed,
    );
}

/// Secp256r1 signature authenticator.
public fun authenticate_secp256r1(account_id: &UID, signature: vector<u8>, message: &vector<u8>) {
    assert!(has(account_id), EPublicKeyMissing);

    // Check the signature.
    assert!(
        ecdsa_r1::secp256r1_verify(
            &decode(signature),
            borrow(account_id),
            message,
            0,
        ),
        ESecp256r1VerificationFailed,
    );
}
