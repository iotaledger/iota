// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module abstract_account::delayed_abstract_account_keyed;

use abstract_account::basic_keyed_aa;
use abstract_account::delayed_abstract_account::{Self, DelayedAbstractAccount};
use iota::authenticator_function::AuthenticatorFunctionRefV1;

// === Errors ===

// === Constants ===

// === Structs ===

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Creates a new `DelayedAbstractAccount`  as a shared object with the given authenticator.
///
/// `authenticator` is expected to have a signature like the following:
///
/// public fun authenticate(self: &DelayedAbstractAccount, signature: vector<u8>, _: &AuthContext, _: &TxContext) { ... }
///
/// to allow to verify the `signature` parameter against the public key stored in the account.
///
/// There are several ready-made authenticators available in this module:
/// - `authenticate_ed25519`
/// - `authenticate_secp256k1`
/// - `authenticate_secp256r1`
public fun create(
    mut self: DelayedAbstractAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<DelayedAbstractAccount>,
    ctx: &mut TxContext,
) {
    self.add_field(basic_keyed_aa::owner_public_key(), public_key, ctx);
    delayed_abstract_account::build(self, authenticator);
}

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    account: &mut DelayedAbstractAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<DelayedAbstractAccount>,
    ctx: &TxContext,
) {
    // Update the account owner public key dynamic field. It is expected that the field already exists.
    account.replace_field(basic_keyed_aa::owner_public_key(), public_key, ctx);

    // Update the account authenticator dynamic field. It is expected that the field already exists.
    account.rotate_auth_function_ref_v1(authenticator, ctx);
}

/// Ed25519 signature authenticator.
#[authenticator]
public fun authenticate_ed25519(
    account: &DelayedAbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    basic_keyed_aa::authenticate_ed25519(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Secp256k1 signature authenticator.
#[authenticator]
public fun authenticate_secp256k1(
    account: &DelayedAbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    basic_keyed_aa::authenticate_secp256k1(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Secp256r1 signature authenticator.
#[authenticator]
public fun authenticate_secp256r1(
    account: &DelayedAbstractAccount,
    signature: vector<u8>,
    actx: &AuthContext,
    ctx: &TxContext,
) {
    // Check the signature.
    basic_keyed_aa::authenticate_secp256r1(
        &signature,
        borrow_public_key(account),
        actx,
        ctx,
    );
}

/// Free access, do nothing.
#[authenticator]
public fun authenticate_free_access(_: &DelayedAbstractAccount, _: &AuthContext, _: &TxContext) {}

// === View Functions ===

/// An utility function to borrow the account-related public key.
public fun borrow_public_key(account: &DelayedAbstractAccount): &vector<u8> {
    account.borrow_field(basic_keyed_aa::owner_public_key())
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
