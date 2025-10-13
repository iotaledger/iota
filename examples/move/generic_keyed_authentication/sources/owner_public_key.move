// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module generic_keyed_authentication::owner_public_key;

use iota::dynamic_field;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hex::decode;

// Common functionality for constructing signature based authentication logic for abstract accounts.
// These tools have protection for the values they manage, but impose no other access restrictions.
// It is the sole responsibility of the account developer to ensure that only the right sender has
// access to any logic provided by these functions.

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

// Attach a public key data to the account.
public fun attach(account_id: &mut UID, public_key: vector<u8>) {
    assert!(!has(account_id), EPublicKeyAttached);

    dynamic_field::add(account_id, OwnerPublicKey {}, public_key);
}

// Detach public key data from the account, disabling keyed based authentication for the
// account.
public fun detach(account_id: &mut UID): vector<u8> {
    assert!(has(account_id), EPublicKeyMissing);

    dynamic_field::remove(account_id, OwnerPublicKey {})
}

// Rotate the stored public key for the account.
public fun rotate(account_id: &mut UID, public_key: vector<u8>): vector<u8> {
    assert!(has(account_id), EPublicKeyMissing);

    let prev_public_key = dynamic_field::remove(account_id, OwnerPublicKey {});
    dynamic_field::add(account_id, OwnerPublicKey {}, public_key);
    prev_public_key
}

// Run the Ed25519 authenticator for the given account, signature and message.
//
// The account must have a stored public key, against which the message will be checked using
// the given signature.
// The signature is expected to be hex::encode-ed and it will be decoded internally.
public fun authenticate_ed25519(account_id: &UID, signature: vector<u8>, message: &vector<u8>) {
    assert!(has(account_id), EPublicKeyMissing);

    assert!(
        ed25519::ed25519_verify(&decode(signature), borrow(account_id), message),
        EEd25519VerificationFailed,
    );
}

// Run the Secp256k1 authenticator for the given account, signature and message.
//
// The account must have a stored public key, against which the message will be checked using
// the given signature.
// The signature is expected to be hex::encode-ed and it will be decoded internally.
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

// Run the Secp256r1 authenticator for the given account, signature and message.
//
// The account must have a stored public key, against which the message will be checked using
// the given signature.
// The signature is expected to be hex::encode-ed and it will be decoded internally.
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

// === Public-View Functions ===

// Check if the account contains the required public key.
public fun has(account_id: &UID): bool {
    dynamic_field::exists_(account_id, OwnerPublicKey {})
}

// Borrow the stored public key from the account.
public fun borrow(account_id: &UID): &vector<u8> {
    dynamic_field::borrow(account_id, OwnerPublicKey {})
}

// === Admin Functions ===

// === Public-Package Functions ===

// === Private Functions ===

// === Test Functions ===
