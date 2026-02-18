// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module defines a set of authenticators for `IOTAccount` that use public key cryptography to verify
/// the authenticity of the transaction sender.
///
/// It allows to set a public key as field of the account, such that the authenticators of this module can
/// authenticate the account by verifying a signature.
/// The public key schemes implemented in this module are:
/// - ed25519 -> `ed25519_IOTAccount_authenticator`
/// - secp256k1 -> `secp256k1_IOTAccount_authenticator`
/// - secp256r1 -> `secp256r1_IOTAccount_authenticator`
module iotaccount::public_key_authenticators;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iotaccount::iotaccount::{Self, IOTAccount, IOTAccountBuilder};

// === Errors ===

#[error(code = 0)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 1)]
const ESecp256k1VerificationFailed: vector<u8> = b"Secp256k1 authenticator verification failed.";
#[error(code = 2)]
const ESecp256r1VerificationFailed: vector<u8> = b"Secp256r1 authenticator verification failed.";

// === Constants ===

// === Structs ===

/// A dynamic field key for the account owner public key.
public struct PublicKeyField has copy, drop, store {}

// === Account Helpers ===

/// Creates a new `IOTAccount` as a shared object with the given authenticator.
///
/// It sets a public key as field of the account and the given authenticator, which will be used to
/// authenticate the account.
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    iotaccount::builder(authenticator, ctx).with_field(PublicKeyField {}, public_key).build();
}

/// Creates a new `IOTAccount` as a shared object with the given authenticator.
///
/// It sets a public key as field of the account, the given authenticator which will be used to
/// authenticate the account and the admin address.
public fun create_with_admin(
    public_key: vector<u8>,
    admin: address,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    iotaccount::builder(authenticator, ctx)
        .with_field(PublicKeyField {}, public_key)
        .with_admin(admin)
        .build();
}

/// Attach a PublicKey as a dynamic field to the account being built.
public fun with_public_key(self: IOTAccountBuilder, public_key: vector<u8>): IOTAccountBuilder {
    self.with_field(PublicKeyField {}, public_key)
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
    account.rotate_field(PublicKeyField {}, public_key, ctx);

    // Update the account authenticator dynamic field. It is expected that the field already exists.
    account.rotate_auth_function_ref_v1(authenticator, ctx);
}

// Attach a public key to the account with the provided `public_key`.
// It fails if the account already has a public key attached.
// Only the account itself can call this function.
public fun add_public_key(
    account: &mut IOTAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &TxContext,
) {
    // Update the account owner public key dynamic field. It is expected that the field does not exist.
    account.add_field(PublicKeyField {}, public_key, ctx);

    // Update the account authenticator dynamic field. It is expected that the field already exists.
    account.rotate_auth_function_ref_v1(authenticator, ctx);
}

// === Authenticators ===

/// Ed25519 signature authenticator for `IOTAccount`.
#[authenticator]
public fun ed25519_IOTAccount_authenticator(
    account: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    authenticate_ed25519(account, signature, ctx);
}

/// Secp256k1 signature authenticator for `IOTAccount`.
#[authenticator]
public fun secp256k1_IOTAccount_authenticator(
    account: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    authenticate_secp256k1(account, signature, ctx);
}

/// Secp256r1 signature authenticator for `IOTAccount`.
#[authenticator]
public fun secp256r1_IOTAccount_authenticator(
    account: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    authenticate_secp256r1(account, signature, ctx);
}

// === Public Authenticators Helpers ===

/// Ed25519 signature authenticator helper.
public fun authenticate_ed25519(account: &IOTAccount, signature: vector<u8>, ctx: &TxContext) {
    assert!(
        ed25519::ed25519_verify(&signature, borrow_public_key(account), ctx.digest()),
        EEd25519VerificationFailed,
    );
}

/// Secp256k1 signature authenticator helper.
public fun authenticate_secp256k1(account: &IOTAccount, signature: vector<u8>, ctx: &TxContext) {
    assert!(
        ecdsa_k1::secp256k1_verify(&signature, borrow_public_key(account), ctx.digest(), 0),
        ESecp256k1VerificationFailed,
    );
}

/// Secp256r1 signature authenticator helper.
public fun authenticate_secp256r1(account: &IOTAccount, signature: vector<u8>, ctx: &TxContext) {
    assert!(
        ecdsa_r1::secp256r1_verify(&signature, borrow_public_key(account), ctx.digest(), 0),
        ESecp256r1VerificationFailed,
    );
}

// === View Functions ===

/// An utility function to check if the account has a public key set.
public fun has_public_key(account: &IOTAccount): bool {
    account.has_field(PublicKeyField {})
}

/// An utility function to borrow the account-related public key.
public fun borrow_public_key(account: &IOTAccount): &vector<u8> {
    account.borrow_field(PublicKeyField {})
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
