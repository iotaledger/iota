// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Built-in authenticator support for `SmartAccount`: the built-in authenticator for IOTA's
/// standard signature schemes (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey), checking
/// signatures against the account's public key.
///
/// This is the only module that attaches, rotates or detaches a `SmartAccount`'s public key, and
/// it keeps the key and the built-in authenticator together: an account has both or neither.
///
/// - `builder_v1` and the `ClaimAccount` pipeline create an account with both;
/// - `rotate_to_builtin_auth_v1` attaches a key and sets the built-in authenticator in one call;
/// - `rotate_to_custom_auth_v1` sets a custom authenticator and detaches the key in one call;
/// - `rotate_public_key` replaces the key, with one of any supported scheme, while the built-in
///   authenticator is set. The built-in authenticator uses the scheme of the attached key, so it
///   stays as it is.
///
/// `smart_account::build_v1` and `smart_account::rotate_auth_function_ref_v1` check the same
/// pairing, so no `SmartAccount` can have the built-in authenticator without the key, or the key
/// without the built-in authenticator.
///
/// Claiming an existing address through the `ClaimAccount` transaction kind drives the private
/// `claim_account_v1` below.
module iota::smart_account_builtin_auth;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::builtin_authenticator_functions::{Self, builtin_authenticator_function_ref_v1};
use iota::public_key::PublicKey;
use iota::smart_account::{Self, SmartAccount, SmartAccountBuilder};

// === Errors ===

#[error(code = 0)]
const EBuiltinAuthAlreadySet: vector<u8> =
    b"The built-in authenticator is already set; use `rotate_public_key` to replace its key.";
#[error(code = 1)]
const EBuiltinAuthNotSet: vector<u8> =
    b"The built-in authenticator is not set; use `rotate_to_builtin_auth_v1`.";
#[error(code = 2)]
const EAuthenticatorIsBuiltin: vector<u8> =
    b"Rotate to the built-in authenticator with `rotate_to_builtin_auth_v1`.";

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
    let builder = smart_account::builder_v1(builtin_authenticator_function_ref_v1(), ctx);
    with_public_key(builder, public_key)
}

// === View Functions ===

/// Returns `true` if and only if the account's authenticator is the built-in one.
public fun has_builtin_auth(account: &SmartAccount): bool {
    builtin_authenticator_functions::is_builtin_authenticator(
        account.borrow_auth_function_ref_v1(),
    )
}

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

// === Admin Functions ===

/// Attaches `public_key` to the account and rotates its authenticator to the built-in one, which
/// checks signatures against that key. Returns the previous authenticator.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event and an
/// `account::AuthenticatorFunctionRefV1Rotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the built-in authenticator is already set; use `rotate_public_key` to replace its
/// key.
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun rotate_to_builtin_auth_v1(
    account: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(!has_builtin_auth(account), EBuiltinAuthAlreadySet);

    builtin_authenticator_functions::attach_public_key(account.uid_mut(), public_key);
    account.rotate_auth_function_ref_v1(builtin_authenticator_function_ref_v1(), ctx)
}

/// Rotates the account's authenticator to the custom `authenticator`, detaching the public key if
/// one is attached, and returns that key.
///
/// Emits an `account::AuthenticatorFunctionRefV1Rotated` event, and a
/// `builtin_authenticator_functions::PublicKeyDetached` event if a key was attached, on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `authenticator` is the built-in one; use `rotate_to_builtin_auth_v1`.
public fun rotate_to_custom_auth_v1(
    account: &mut SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): Option<PublicKey> {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(
        !builtin_authenticator_functions::is_builtin_authenticator(&authenticator),
        EAuthenticatorIsBuiltin,
    );

    let public_key = if (has_public_key(account)) {
        option::some(builtin_authenticator_functions::detach_public_key(account.uid_mut()))
    } else {
        option::none()
    };
    account.rotate_auth_function_ref_v1(authenticator, ctx);
    public_key
}

/// Replaces the account's public key with `public_key`, of any supported scheme, and returns the
/// previous key. The built-in authenticator stays as it is and checks signatures against the new
/// key.
///
/// Emits a `builtin_authenticator_functions::PublicKeyRotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the built-in authenticator is not set; use `rotate_to_builtin_auth_v1`.
public fun rotate_public_key(
    account: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_builtin_auth(account), EBuiltinAuthNotSet);

    builtin_authenticator_functions::rotate_public_key(account.uid_mut(), public_key)
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
    let builder = smart_account::new_claim_builder(
        public_key,
        builtin_authenticator_function_ref_v1(),
        ctx,
    );
    with_public_key(builder, public_key).build_v1();
}

/// Attaches `public_key` to the account being built.
fun with_public_key(mut builder: SmartAccountBuilder, public_key: PublicKey): SmartAccountBuilder {
    builtin_authenticator_functions::attach_public_key(builder.borrow_uid_mut(), public_key);
    builder
}

// === Test Functions ===

/// Test-only entry to `claim_account_v1`, which is private so that only the node's
/// `ClaimAccount` pipeline can reach it.
#[test_only]
public fun claim_account_v1_for_testing(public_key: PublicKey, ctx: &TxContext) {
    claim_account_v1(public_key, ctx)
}
