// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module key_pair_based::account;

use iota_account::iota_account::{Self, IOTAccount};

use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::ecdsa_k1;
use iota::ecdsa_r1;
use iota::ed25519;
use iota::hex::decode;

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"The user who signed the transaction is not the account.";

#[error(code = 10)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 11)]
const ESecp256k1VerificationFailed: vector<u8> = b"Secp256k1 authenticator verification failed.";
#[error(code = 12)]
const ESecp256r1VerificationFailed: vector<u8> = b"Secp256r1 authenticator verification failed.";

/// A dynamic field key for the account owner public key.
public struct OwnerPublicKey has copy, drop, store {}

// --------------------------------------- Creation ---------------------------------------

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
public fun create(public_key: vector<u8>, authenticator: AuthenticatorInfoV1, ctx: &mut TxContext) {
    iota_account::builder(ctx)
        .add_reserved_dynamic_field(OwnerPublicKey{}, public_key)
        .add_authenticator(authenticator)
        .share();
}

public fun clear(self: &mut IOTAccount, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    self.remove_field<_, vector<u8>>(OwnerPublicKey{}, ctx);
}

// --------------------------------------- Authentication ---------------------------------------

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    self: &mut IOTAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1,
    ctx: &TxContext
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Update the account owner public key dynamic field. It is expected that the field already exists.
    let owner_public_key = OwnerPublicKey{};

    self.rotate_reserved_field(owner_public_key, public_key, ctx);
    self.rotate_reserved_field(account::authenticator_df_name(), authenticator, ctx);
}

// --------------------------------------- Authenticators ---------------------------------------

/// Ed25519 signature authenticator.
public fun authenticate_ed25519(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(
        ed25519::ed25519_verify(&decode(signature), borrow_public_key(self), ctx.digest()),
        EEd25519VerificationFailed
    );
}

/// Secp256k1 signature authenticator.
public fun authenticate_secp256k1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(
        ecdsa_k1::secp256k1_verify(&decode(signature), borrow_public_key(self), ctx.digest(), 0),
        ESecp256k1VerificationFailed
    );
}

/// Secp256r1 signature authenticator.
public fun authenticate_secp256r1(
    self: &IOTAccount,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Check the signature.
    assert!(
        ecdsa_r1::secp256r1_verify(&decode(signature), borrow_public_key(self), ctx.digest(), 0),
        ESecp256r1VerificationFailed
    );
}

// --------------------------------------- Utilities ---------------------------------------

/// An utility function to borrow the account-related public key.
fun borrow_public_key(self: &IOTAccount): &vector<u8> {
    self.borrow_field(OwnerPublicKey{})
}

/// Checks that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &IOTAccount, ctx: &TxContext) {
    assert!(self.addr() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// --------------------------------------- Test Utilities ---------------------------------------

#[test_only]
public fun create_owner_public_key_for_testing(): OwnerPublicKey {
    OwnerPublicKey{}
}
