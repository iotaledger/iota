// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Built-in authenticator support for `SmartAccount`: the built-in authenticator for IOTA's
/// standard signature schemes (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey), checking
/// signatures against the account's public key.
///
/// The public key is managed with `iota::smart_account_public_key`, as for any other
/// authenticator. The built-in authenticator uses the scheme of the attached key, so the key can
/// be rotated to any supported scheme without touching the authenticator.
///
/// `builder_v1` and `rotate_to_builtin_auth_v1` only set the built-in authenticator together with
/// a public key. `smart_account::builder_v1` and `smart_account::rotate_auth_function_ref_v1`
/// accept any authenticator, and `smart_account_public_key::detach_public_key` does not look at
/// the authenticator, so they can still leave an account with the built-in authenticator and no
/// key, which then can't send any transaction.
///
/// Claiming an existing address through the `ClaimAccount` transaction kind drives the private
/// `claim_account_v1` below.
module iota::smart_account_builtin_auth;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::builtin_authenticator_functions;
use iota::public_key::PublicKey;
use iota::smart_account::{Self, SmartAccount, SmartAccountBuilder};
use iota::smart_account_public_key;

// === Errors ===

#[error(code = 0)]
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
    let builder = smart_account::builder_v1(builtin_auth_function_ref_v1(), ctx);
    smart_account_public_key::with_public_key(builder, public_key)
}

// === View Functions ===

/// Returns `true` if and only if the account's authenticator is the built-in one.
public fun has_builtin_auth(account: &SmartAccount): bool {
    builtin_authenticator_functions::is_builtin_authenticator(
        account.borrow_auth_function_ref_v1(),
    )
}

// === Admin Functions ===

/// Rotates the account's authenticator to the built-in one, which checks signatures against the
/// attached public key, and returns the previous authenticator.
///
/// Attach a key first with `smart_account_public_key::attach_public_key` if none is attached.
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
    assert!(smart_account_public_key::has_public_key(account), EPublicKeyMissing);

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
    let builder = smart_account::new_claim_builder(
        public_key,
        builtin_auth_function_ref_v1(),
        ctx,
    );
    smart_account_public_key::with_public_key(builder, public_key).build_v1();
}

// === Test Functions ===

/// Test-only entry to `claim_account_v1`, which is private so that only the node's
/// `ClaimAccount` pipeline can reach it.
#[test_only]
public fun claim_account_v1_for_testing(public_key: PublicKey, ctx: &TxContext) {
    claim_account_v1(public_key, ctx)
}
