// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An extension for `SmartAccount` that limits the authenticators the account can rotate to.
///
/// It attaches a rotation rule, `AllowedAuthenticatorsRule`, whose config is the list of allowed
/// authenticators: with `with_allowed_authenticators` while building an account, or with
/// `attach_allowed_authenticators` on an account that already exists. It works next to other
/// rules, such as `smart_account_builtin_auth::BuiltinAuthRule`; every rotation then needs a
/// receipt from each of them:
///
/// ```move
/// let mut request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
///     &account,
///     new_authenticator,
///     ctx,
/// );
/// smart_account_builtin_auth::approve_auth_rotation(&account, &mut request);
/// allowed_authenticators::approve_auth_rotation(&account, &mut request);
/// smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1(&mut account, request, ctx);
/// ```
///
/// `rotate_auth_function_ref_v1` does this in one call when the only other rule, if any, is
/// `BuiltinAuthRule`.
///
/// The list can only shrink (`disallow_authenticator`) and the rule can't be removed, so whoever
/// controls the account's current authenticator can't widen it. Include the built-in authenticator
/// in the list for the account to be able to rotate back to it.
module smart_account_allowed_authenticators::allowed_authenticators;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::smart_account::{SmartAccount, SmartAccountBuilder};
use iota::smart_account_builtin_auth::{Self, BuiltinAuthRule};
use iota::smart_account_rotation_rules::{Self, AuthRotationRequest};
use iota::vec_set::{Self, VecSet};

// === Errors ===

#[error(code = 0)]
const EAuthenticatorNotAllowed: vector<u8> =
    b"The authenticator is not in the account's list of allowed authenticators.";
#[error(code = 1)]
const ERequestForAnotherAccount: vector<u8> = b"The rotation request was made for another account.";

// === Structs ===

/// The rotation rule of this extension.
public struct AllowedAuthenticatorsRule has drop {}

/// The config of `AllowedAuthenticatorsRule`: the authenticators the account can rotate to.
public struct AllowedAuthenticators has drop, store {
    authenticators: VecSet<AuthenticatorFunctionRefV1<SmartAccount>>,
}

// === SmartAccountBuilder Functions ===

/// Attaches the list `allowed` to the account being built.
///
/// Aborts if `allowed` contains the same authenticator twice.
public fun with_allowed_authenticators(
    builder: SmartAccountBuilder,
    allowed: vector<AuthenticatorFunctionRefV1<SmartAccount>>,
): SmartAccountBuilder {
    smart_account_rotation_rules::with_auth_rotation_rule(
        builder,
        AllowedAuthenticatorsRule {},
        new_config(allowed),
    )
}

// === Public Functions ===

/// Adds the receipt of `AllowedAuthenticatorsRule` to a rotation request on `account`.
///
/// Aborts if `request` was made for another account.
/// Aborts if the request rotates to an authenticator that is not in the list.
public fun approve_auth_rotation(account: &SmartAccount, request: &mut AuthRotationRequest) {
    assert!(request.request_account_id() == object::id(account), ERequestForAnotherAccount);
    assert!(is_allowed(account, request.request_authenticator()), EAuthenticatorNotAllowed);

    request.add_auth_rotation_receipt(AllowedAuthenticatorsRule {});
}

// === View Functions ===

/// Returns `true` if and only if the account can rotate to `authenticator`.
///
/// Aborts if the list is not attached to the account.
public fun is_allowed(
    account: &SmartAccount,
    authenticator: &AuthenticatorFunctionRefV1<SmartAccount>,
): bool {
    config(account).authenticators.contains(authenticator)
}

/// Returns the authenticators the account can rotate to.
///
/// Aborts if the list is not attached to the account.
public fun allowed_authenticators(
    account: &SmartAccount,
): vector<AuthenticatorFunctionRefV1<SmartAccount>> {
    *config(account).authenticators.keys()
}

// === Admin Functions ===

/// Attaches the list `allowed` to an existing account. The account's current authenticator keeps
/// working until it is rotated, even if it is not in the list.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the list is already attached.
/// Aborts if `allowed` contains the same authenticator twice.
public fun attach_allowed_authenticators(
    account: &mut SmartAccount,
    allowed: vector<AuthenticatorFunctionRefV1<SmartAccount>>,
    ctx: &TxContext,
) {
    smart_account_rotation_rules::add_auth_rotation_rule(
        account,
        AllowedAuthenticatorsRule {},
        new_config(allowed),
        ctx,
    );
}

/// Rotates the account's authenticator to `authenticator`, approving the rotation for
/// `AllowedAuthenticatorsRule` and, if attached, `BuiltinAuthRule`, and returns the previous
/// authenticator.
///
/// For an account with no other rotation rules. With others attached, use
/// `smart_account_rotation_rules::request_auth_function_ref_rotation_v1`, each rule's approval,
/// then `smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1`.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `authenticator` is not in the list.
/// Aborts if `BuiltinAuthRule` refuses the rotation.
/// Aborts if another rotation rule is attached.
public fun rotate_auth_function_ref_v1(
    account: &mut SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    let mut request = smart_account_rotation_rules::request_auth_function_ref_rotation_v1(
        account,
        authenticator,
        ctx,
    );
    if (smart_account_rotation_rules::has_auth_rotation_rule<BuiltinAuthRule>(account)) {
        smart_account_builtin_auth::approve_auth_rotation(account, &mut request);
    };
    approve_auth_rotation(account, &mut request);
    smart_account_rotation_rules::confirm_auth_function_ref_rotation_v1(account, request, ctx)
}

/// Removes `authenticator` from the list. The account's current authenticator keeps working
/// until it is rotated.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the list is not attached, or `authenticator` is not in it.
public fun disallow_authenticator(
    account: &mut SmartAccount,
    authenticator: &AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
) {
    let config: &mut AllowedAuthenticators =
        smart_account_rotation_rules::borrow_auth_rotation_rule_config_mut(
            account,
            AllowedAuthenticatorsRule {},
            ctx,
        );
    config.authenticators.remove(authenticator);
}

// === Private Functions ===

fun new_config(allowed: vector<AuthenticatorFunctionRefV1<SmartAccount>>): AllowedAuthenticators {
    AllowedAuthenticators { authenticators: vec_set::from_keys(allowed) }
}

fun config(account: &SmartAccount): &AllowedAuthenticators {
    smart_account_rotation_rules::borrow_auth_rotation_rule_config(
        account,
        AllowedAuthenticatorsRule {},
    )
}
