// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Built-in authenticator support for `SmartAccount`: a public key attached to the account,
/// checked by the built-in authenticator for IOTA's standard signature schemes (Ed25519,
/// Secp256k1, Secp256r1, MultiSig, Passkey).
///
/// The built-in authenticator uses the scheme of the attached public key, so the key can be
/// rotated to any supported scheme without touching the authenticator.
///
/// The functions of this module keep a public key attached whenever the account's authenticator
/// is the built-in one:
/// - `rotate_to_builtin_auth_v1` aborts while no key is attached;
/// - the key cannot be detached while the built-in authenticator is attached.
///
/// `smart_account::builder_v1` and `smart_account::rotate_auth_function_ref_v1` accept any
/// authenticator, so they can still set the built-in one on an account without a key, which then
/// can't send any transaction. Use this module's `builder_v1` and `rotate_to_builtin_auth_v1`
/// instead.
///
/// Claiming an existing address through the `ClaimAccount` transaction kind drives the private
/// `claim_account_v1` below.
module iota::smart_account_builtin_auth;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::builtin_authenticator_functions;
use iota::public_key::PublicKey;
use iota::smart_account::{Self, SmartAccount, SmartAccountBuilder};

// === Errors ===

#[error(code = 0)]
const EBuiltinAuthAttached: vector<u8> =
    b"The public key cannot be detached while the built-in authenticator is attached.";
#[error(code = 1)]
const EPublicKeyMissing: vector<u8> =
    b"The built-in authenticator needs a public key attached to the account.";

// === Public Functions ===

/// Creates a `SmartAccountBuilder` for a new account backed by the built-in authenticator, with
/// `public_key` attached.
///
/// Finish it with `smart_account::build_v1`.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event on success.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun builder_v1(public_key: PublicKey, ctx: &mut TxContext): SmartAccountBuilder {
    let mut builder = smart_account::builder_v1(builtin_auth_function_ref_v1(), ctx);
    builtin_authenticator_functions::attach_public_key(builder.builder_uid_mut(), public_key);

    builder
}

// === View Functions ===

/// Returns `true` if and only if the account has a public key attached.
public fun has_public_key(account: &SmartAccount): bool {
    builtin_authenticator_functions::has_public_key(account.uid())
}

/// Borrows the public key attached to the account.
///
/// Aborts if no public key is attached.
public fun borrow_public_key(account: &SmartAccount): &PublicKey {
    builtin_authenticator_functions::borrow_public_key(account.uid())
}

/// Returns `true` if and only if the account's authenticator is the built-in one.
public fun has_builtin_auth(account: &SmartAccount): bool {
    builtin_authenticator_functions::is_builtin_authenticator(
        account.borrow_auth_function_ref_v1(),
    )
}

// === Admin Functions ===

/// Attaches `public_key` to the account. The authenticator is unchanged.
///
/// To also switch to the built-in authenticator, call `rotate_to_builtin_auth_v1` afterwards.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if a public key is already attached.
public fun attach_public_key(account: &mut SmartAccount, public_key: PublicKey, ctx: &TxContext) {
    account.ensure_tx_sender_is_smart_account(ctx);

    builtin_authenticator_functions::attach_public_key(account.uid_mut(), public_key);
}

/// Detaches and returns the public key attached to the account.
///
/// Use this after rotating the account to a custom authenticator.
///
/// Emits a `builtin_authenticator_functions::PublicKeyDetached` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the account's authenticator is the built-in one.
/// Aborts if no public key is attached.
public fun detach_public_key(account: &mut SmartAccount, ctx: &TxContext): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(!has_builtin_auth(account), EBuiltinAuthAttached);

    builtin_authenticator_functions::detach_public_key(account.uid_mut())
}

/// Replaces the attached public key with `public_key`, of any supported scheme, and returns the
/// previous key. The authenticator is unchanged.
///
/// Emits a `builtin_authenticator_functions::PublicKeyRotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is attached.
public fun rotate_public_key(
    account: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);

    builtin_authenticator_functions::rotate_public_key(account.uid_mut(), public_key)
}

/// Rotates the account's authenticator to the built-in one, which checks signatures against the
/// attached public key, and returns the previous authenticator.
///
/// Emits an `account::AuthenticatorFunctionRefV1Rotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if no public key is attached.
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun rotate_to_builtin_auth_v1(
    account: &mut SmartAccount,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_public_key(account), EPublicKeyMissing);

    account.rotate_auth_function_ref_v1(builtin_auth_function_ref_v1(), ctx)
}

/// Returns the built-in authenticator ref for `SmartAccount`.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun builtin_auth_function_ref_v1(): AuthenticatorFunctionRefV1<SmartAccount> {
    builtin_authenticator_functions::builtin_authenticator_function_ref_v1()
}

// === Private Functions ===

/// Claims the sender's address and creates a `SmartAccount` at it, backed by the built-in
/// authenticator with `public_key` attached.
///
/// This is the whole `ClaimAccount` pipeline. It is **private on purpose**: the account object
/// it creates has an ID equal to a signature-derivable address, so `ClaimAccount` must stay the
/// one and only way such an object can come into existence. A private function is reachable from
/// the node's own PTB, which runs in an execution mode that bypasses visibility, and from nowhere
/// else — not from a user PTB, and not from another package, which could otherwise wrap a public
/// entry point. See the `iota::clock::consensus_commit_prologue` function for the same idiom.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event and an
/// `account::MutableAccountCreated` event.
///
/// Aborts if `public_key` does not derive the sender's address.
#[allow(unused_function)]
fun claim_account_v1(public_key: PublicKey, ctx: &TxContext) {
    let mut builder = smart_account::new_claim_builder(
        public_key,
        builtin_auth_function_ref_v1(),
        ctx,
    );
    builtin_authenticator_functions::attach_public_key(builder.builder_uid_mut(), public_key);

    builder.build_v1();
}

// === Test Functions ===

/// Test-only entry to `claim_account_v1`, which is private so that only the node's
/// `ClaimAccount` pipeline can reach it.
#[test_only]
public fun claim_account_v1_for_testing(public_key: PublicKey, ctx: &TxContext) {
    claim_account_v1(public_key, ctx)
}
