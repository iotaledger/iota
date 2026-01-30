// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iotaccount::keyed_iotaccount;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hex::decode;
use iotaccount::iotaccount::{Self, IOTAccount, ensure_tx_sender_is_account};

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

/// Creates a new `IOTAccount`  as a shared object with the given authenticator.
///
/// `authenticator` is expected to have a signature like the following:
///
/// public fun authenticate(self: &IOTAccount, signature: vector<u8>, _: &AuthContext, _: &TxContext) { ... }
///
/// to allow to verify the `signature` parameter against the public key stored in the account.
///
/// There are several ready-made authenticators available in this module:
/// - `authenticate_ed25519`
/// - `authenticate_secp256k1`
/// - `authenticate_secp256r1`
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    iotaccount::builder(authenticator, ctx)
        .add_dynamic_field(OwnerPublicKey {}, public_key)
        .build();
}

/// Ed25519 signature authenticator.
#[authenticator]
public fun authenticate_ed25519(
    account: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(account, ctx);

    // Check the signature.
    assert!(
        ed25519::ed25519_verify(&decode(signature), borrow_public_key(account), ctx.digest()),
        EEd25519VerificationFailed,
    );
}

/// Secp256k1 signature authenticator.
#[authenticator]
public fun authenticate_secp256k1(
    account: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(account, ctx);

    // Check the signature.
    assert!(
        ecdsa_k1::secp256k1_verify(&decode(signature), borrow_public_key(account), ctx.digest(), 0),
        ESecp256k1VerificationFailed,
    );
}

/// Secp256r1 signature authenticator.
#[authenticator]
public fun authenticate_secp256r1(
    account: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(account, ctx);

    // Check the signature.
    assert!(
        ecdsa_r1::secp256r1_verify(&decode(signature), borrow_public_key(account), ctx.digest(), 0),
        ESecp256r1VerificationFailed,
    );
}

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    account: &mut IOTAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &TxContext,
) {
    // Update the account owner public key dynamic field. It is expected that the field already exists.
    account.rotate_field(OwnerPublicKey {}, public_key, ctx);

    // Update the account authenticator dynamic field. It is expected that the field already exists.
    account.rotate_auth_function_ref_v1(authenticator, ctx);
}

// === View Functions ===

/// An utility function to borrow the account-related public key.
public fun borrow_public_key(account: &IOTAccount): &vector<u8> {
    account.borrow_field(OwnerPublicKey {})
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
