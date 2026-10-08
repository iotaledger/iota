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
/// `builder_v1`, `ClaimAccount` and `rotate_to_builtin_auth_v1` also attach this module's rotation
/// rule `BuiltinAuthRule` (see `iota::smart_account_rotation_rules`). From then on
/// `smart_account::rotate_auth_function_ref_v1` aborts, and every rotation needs this rule's
/// receipt (`approve_auth_rotation`), which it refuses for a rotation that would separate the key
/// from the built-in authenticator: to the built-in authenticator while no key is attached, or to
/// a custom one while the key is attached. So the account always has both or neither.
///
/// The rotation functions approve and rotate in one call when `BuiltinAuthRule` is the only rule.
/// With other rules attached, make the request with
/// `smart_account_rotation_rules::request_auth_function_ref_rotation_v1`, add the other rules'
/// receipts, and pass it to `rotate_to_builtin_auth_with_request_v1` or
/// `rotate_to_custom_auth_with_request_v1`.
///
/// `smart_account::builder_v1` accepts any authenticator, so an account built with it that never
/// used this module can still get the built-in authenticator without a key, which then can't
/// send any transaction.
///
/// Claiming an existing address through the `ClaimAccount` transaction kind drives the private
/// `claim_account_v1` below.
module iota::smart_account_builtin_auth;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::builtin_authenticator_functions::{Self, builtin_authenticator_function_ref_v1};
use iota::public_key::PublicKey;
use iota::smart_account::{Self, SmartAccount, SmartAccountBuilder};
use iota::smart_account_rotation_rules::{Self, AuthRotationRequest};

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
#[error(code = 3)]
const EAuthenticatorIsNotBuiltin: vector<u8> =
    b"Rotate to a custom authenticator with `rotate_to_custom_auth_v1`.";
#[error(code = 4)]
const EAuthRotationRequestForAnotherAccount: vector<u8> =
    b"The rotation request was made for another account.";
#[error(code = 5)]
const EPublicKeyMissing: vector<u8> =
    b"The built-in authenticator needs a public key attached to the account.";
#[error(code = 6)]
const EPublicKeyAttached: vector<u8> =
    b"A custom authenticator can't be set while the public key is attached.";

// === Structs ===

/// The rotation rule this module attaches to the accounts it manages.
public struct BuiltinAuthRule has drop {}

/// The configuration stored with `BuiltinAuthRule`. The rule needs none.
public struct BuiltinAuthRuleConfig has drop, store {}

// === Public Functions ===

/// Creates a `SmartAccountBuilder` for a new account backed by the built-in authenticator, with
/// `public_key` attached and `BuiltinAuthRule` attached.
///
/// Finish it with `smart_account::build_v1`.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event and a
/// `smart_account_rotation_rules::AuthRotationRuleAdded` event on success.
///
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun builder_v1(public_key: PublicKey, ctx: &mut TxContext): SmartAccountBuilder {
    let builder = smart_account::builder_v1(builtin_authenticator_function_ref_v1(), ctx);
    with_builtin_auth_rule(with_public_key(builder, public_key))
}

/// Adds the receipt of `BuiltinAuthRule` to a rotation request on `account`.
///
/// Called by `rotate_to_builtin_auth_with_request_v1` and `rotate_to_custom_auth_with_request_v1`
/// after they attach or detach the key; call it directly to rotate between custom authenticators.
///
/// Aborts if `request` was made for another account.
/// Aborts if the request rotates to the built-in authenticator and no public key is attached.
/// Aborts if the request rotates to a custom authenticator and a public key is attached.
public fun approve_auth_rotation(account: &SmartAccount, request: &mut AuthRotationRequest) {
    assert!(
        request.request_account_id() == object::id(account),
        EAuthRotationRequestForAnotherAccount,
    );
    let to_builtin_auth = builtin_authenticator_functions::is_builtin_authenticator(
        request.request_authenticator(),
    );
    if (to_builtin_auth) {
        assert!(has_public_key(account), EPublicKeyMissing);
    } else {
        assert!(!has_public_key(account), EPublicKeyAttached);
    };

    request.add_auth_rotation_receipt(BuiltinAuthRule {});
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
/// For an account whose only rotation rule, if any, is `BuiltinAuthRule`; with other rules, use
/// `rotate_to_builtin_auth_with_request_v1`.
///
/// See `rotate_to_builtin_auth_with_request_v1` for the events and aborts.
public fun rotate_to_builtin_auth_v1(
    account: &mut SmartAccount,
    public_key: PublicKey,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    let request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
        account,
        builtin_authenticator_function_ref_v1(),
        ctx,
    );
    rotate_to_builtin_auth_with_request_v1(account, request, public_key, ctx)
}

/// Attaches `public_key` to the account and rotates its authenticator to the built-in one with
/// `request`, which must already carry the receipts of every other rule attached to the account.
/// Attaches `BuiltinAuthRule` first if it is not attached yet. Returns the previous
/// authenticator.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event and an
/// `account::AuthenticatorFunctionRefV1Rotated` event on success, and a
/// `smart_account_rotation_rules::AuthRotationRuleAdded` event if it attaches `BuiltinAuthRule`.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `request` does not rotate to the built-in authenticator.
/// Aborts if the built-in authenticator is already set; use `rotate_public_key` to replace its
/// key.
/// Aborts if a public key is already attached.
/// Aborts if `request` is not approved by every rule attached to the account (see
/// `smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1`).
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun rotate_to_builtin_auth_with_request_v1(
    account: &mut SmartAccount,
    mut request: AuthRotationRequest,
    public_key: PublicKey,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(
        builtin_authenticator_functions::is_builtin_authenticator(request.request_authenticator()),
        EAuthenticatorIsNotBuiltin,
    );
    assert!(!has_builtin_auth(account), EBuiltinAuthAlreadySet);

    if (!smart_account_rotation_rules::has_auth_rotation_rule<BuiltinAuthRule>(account)) {
        smart_account_rotation_rules::add_auth_rotation_rule(
            account,
            BuiltinAuthRule {},
            BuiltinAuthRuleConfig {},
            ctx,
        );
    };
    builtin_authenticator_functions::attach_public_key(account.uid_mut(), public_key);
    approve_auth_rotation(account, &mut request);
    smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1(account, request, ctx)
}

/// Rotates the account's authenticator to the custom `authenticator`, detaching the public key if
/// one is attached, and returns that key.
///
/// For an account whose only rotation rule, if any, is `BuiltinAuthRule`; with other rules, use
/// `rotate_to_custom_auth_with_request_v1`.
///
/// See `rotate_to_custom_auth_with_request_v1` for the events and aborts.
public fun rotate_to_custom_auth_v1(
    account: &mut SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): Option<PublicKey> {
    let request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
        account,
        authenticator,
        ctx,
    );
    rotate_to_custom_auth_with_request_v1(account, request, ctx)
}

/// Rotates the account's authenticator to the custom authenticator of `request`, which must
/// already carry the receipts of every other rule attached to the account, detaching the public
/// key if one is attached. Returns that key.
///
/// Emits an `account::AuthenticatorFunctionRefV1Rotated` event, and a
/// `builtin_authenticator_functions::PublicKeyDetached` event if a key was attached, on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `request` rotates to the built-in authenticator; use `rotate_to_builtin_auth_v1`.
/// Aborts if `request` is not approved by every rule attached to the account (see
/// `smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1`).
public fun rotate_to_custom_auth_with_request_v1(
    account: &mut SmartAccount,
    mut request: AuthRotationRequest,
    ctx: &TxContext,
): Option<PublicKey> {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(
        !builtin_authenticator_functions::is_builtin_authenticator(request.request_authenticator()),
        EAuthenticatorIsBuiltin,
    );

    let public_key = if (has_public_key(account)) {
        option::some(builtin_authenticator_functions::detach_public_key(account.uid_mut()))
    } else {
        option::none()
    };
    if (smart_account_rotation_rules::has_auth_rotation_rule<BuiltinAuthRule>(account)) {
        approve_auth_rotation(account, &mut request);
    };
    smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1(account, request, ctx);
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
/// Aborts if no public key is attached.
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
/// authenticator with `public_key` attached, and with `BuiltinAuthRule` attached.
///
/// This is the whole `ClaimAccount` pipeline. It is **private on purpose**: the account object
/// it creates has an ID equal to a signature-derivable address, so `ClaimAccount` must stay the
/// one and only way such an object can come into existence. A private function is reachable from
/// the node's own PTB, which runs in an execution mode that bypasses visibility, and from nowhere
/// else — not from a user PTB, and not from another package, which could otherwise wrap a public
/// entry point. See the `iota::clock::consensus_commit_prologue` function for the same idiom.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event, a
/// `smart_account_rotation_rules::AuthRotationRuleAdded` event and an
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
    with_builtin_auth_rule(with_public_key(builder, public_key)).build_v1();
}

/// Attaches `public_key` to the account being built.
fun with_public_key(mut builder: SmartAccountBuilder, public_key: PublicKey): SmartAccountBuilder {
    builtin_authenticator_functions::attach_public_key(builder.borrow_uid_mut(), public_key);
    builder
}

/// Attaches `BuiltinAuthRule` to the account being built.
fun with_builtin_auth_rule(builder: SmartAccountBuilder): SmartAccountBuilder {
    smart_account_rotation_rules::with_auth_rotation_rule(
        builder,
        BuiltinAuthRule {},
        BuiltinAuthRuleConfig {},
    )
}

// === Test Functions ===

/// Test-only entry to `claim_account_v1`, which is private so that only the node's
/// `ClaimAccount` pipeline can reach it.
#[test_only]
public fun claim_account_v1_for_testing(public_key: PublicKey, ctx: &TxContext) {
    claim_account_v1(public_key, ctx)
}
