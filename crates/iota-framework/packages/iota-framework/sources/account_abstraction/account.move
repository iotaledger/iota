// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::account;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field;

#[error(code = 0)]
const EAuthenticatorFunctionRefV1AlreadyAttached: vector<u8> =
    b"An `AuthenticatorFunctionRefV1` instance is already attached to the account.";
#[error(code = 1)]
const EAuthenticatorFunctionRefV1NotAttached: vector<u8> =
    b"'AuthenticatorFunctionRefV1' is not attached to the account.";

/// Dynamic field key, where the system will look for a potential
/// authenticate function.
public struct AuthenticatorFunctionRefV1Key has copy, drop, store {}

/// Create an account as a mutable shared object with the provided `authenticator`.
/// The `authenticator` instance will be added to the account as a dynamic field specified by the `AuthenticatorFunctionRefV1Key` name.
/// This function has custom rules performed by the IOTA Move bytecode verifier that ensures
/// that `Account` is an object defined in the module where `create_account_v1` is invoked.
public fun create_account_v1<Account: key>(
    mut account: Account,
    authenticator: AuthenticatorFunctionRefV1<Account>,
) {
    attach_auth_function_ref_v1(&mut account, authenticator);

    create_account_v1_impl(account);
}

/// Create an account as an immutable object with the provided `authenticator`.
/// The `authenticator` instance will be added to the account as a dynamic field specified by the `AuthenticatorFunctionRefV1Key` name.
/// This function has custom rules performed by the IOTA Move bytecode verifier that ensures
/// that `Account` is an object defined in the module where `create_immutable_account_v1` is invoked.
public fun create_immutable_account_v1<Account: key>(
    mut account: Account,
    authenticator: AuthenticatorFunctionRefV1<Account>,
) {
    attach_auth_function_ref_v1(&mut account, authenticator);

    create_immutable_account_v1_impl(account);
}

/// Rotate the account-related authenticator.
/// The `authenticator` instance will replace the account dynamic field specified by the `AuthenticatorFunctionRefV1Key` name.
/// This function has custom rules performed by the IOTA Move bytecode verifier that ensures
/// that `Account` is an object defined in the module where `rotate_auth_function_ref_v1` is invoked.
public fun rotate_auth_function_ref_v1<Account: key>(
    account: &mut Account,
    authenticator: AuthenticatorFunctionRefV1<Account>,
): AuthenticatorFunctionRefV1<Account> {
    let account_id = borrow_account_uid_mut(account);

    assert!(has_auth_function_ref_v1(account_id), EAuthenticatorFunctionRefV1NotAttached);

    let name = auth_function_ref_v1_key();

    let previous_authenticator_function_ref = dynamic_field::remove(account_id, name);
    dynamic_field::add(account_id, name, authenticator);
    previous_authenticator_function_ref
}

/// Borrow the account-related authenticator.
/// The dynamic field specified by the `AuthenticatorFunctionRefV1Key` name will be returned.
public fun borrow_auth_function_ref_v1<Account: key>(
    account_id: &UID,
): &AuthenticatorFunctionRefV1<Account> {
    assert!(has_auth_function_ref_v1(account_id), EAuthenticatorFunctionRefV1NotAttached);
    dynamic_field::borrow(account_id, auth_function_ref_v1_key())
}

/// Check if an authenticator is attached. If a dynamic field with the `AuthenticatorFunctionRefV1Key` name exists.
public fun has_auth_function_ref_v1(account_id: &UID): bool {
    dynamic_field::exists_(account_id, auth_function_ref_v1_key())
}

fun auth_function_ref_v1_key(): AuthenticatorFunctionRefV1Key {
    AuthenticatorFunctionRefV1Key {}
}

/// Add `authenticator` as a dynamic field to `account`.
///
/// IMPORTANT: This function is allowed to be called only by the functions that the IOTA Move bytecode verifier
/// prevents from being invoked outside the module where `Account` is declared.
fun attach_auth_function_ref_v1<Account: key>(
    account: &mut Account,
    authenticator: AuthenticatorFunctionRefV1<Account>,
) {
    let account_id = borrow_account_uid_mut(account);

    assert!(!has_auth_function_ref_v1(account_id), EAuthenticatorFunctionRefV1AlreadyAttached);
    dynamic_field::add(account_id, auth_function_ref_v1_key(), authenticator);
}

/// Borrow the account `UID` mutably.
///
/// IMPORTANT: This function is allowed to be called only by the functions that the IOTA Move bytecode verifier
/// prevents from being invoked outside the module where `Account` is declared.
native fun borrow_account_uid_mut<Account: key>(account: &mut Account): &mut UID;

/// Turn `account` into a mutable shared object.
///
/// IMPORTANT: This function is allowed to be called only by the functions that the IOTA Move bytecode verifier
/// prevents from being invoked outside the module where `Account` is declared.
native fun create_account_v1_impl<Account: key>(account: Account);

/// Turn `account` into an immutable object.
///
/// IMPORTANT: This function is allowed to be called only by the functions that the IOTA Move bytecode verifier
/// prevents from being invoked outside the module where `Account` is declared.
native fun create_immutable_account_v1_impl<Account: key>(account: Account);
