// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::account;

use iota::dynamic_field;
use iota::package_metadata::PackageMetadataV1;
use std::ascii;
use std::type_name;

#[error(code = 0)]
const EAuthenticatorInfoV1AlreadyAttached: vector<u8> =
    b"An `AuthenticatorInfoV1` instance is already attached to the account.";
#[error(code = 1)]
const EAuthenticatorInfoV1NotAttached: vector<u8> =
    b"'AuthenticatorInfoV1' is not attached to the account.";
#[error(code = 2)]
const EAuthenticatorInfoNotCompatibleWithAccount: vector<u8> =
    b"The provided `AuthenticatorInfoV1` is not compatible with the account type.";

/// Dynamic field key, where the system will look for a potential
/// authenticate function.
public struct AuthenticatorInfoV1Key has copy, drop, store {}

/// Represents a validated authenticate function.
#[allow(unused_field)]
public struct AuthenticatorInfoV1<phantom Account: key> has copy, drop, store {
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
}

/// Create an "AuthenticatorInfoV1" using an `authenticate` function defined outside of this version of the package
///
/// The referred `package`, `module_name`, `function_name` can refer to any valid `authenticate` function,
/// regardless of package dependencies or versions.
/// For example package A has two versions V1 and V2. V2 of package A may refer to an `authenticate`
/// function defined in V1. Or it can refer to any package B with an appropriate `authenticate` function
/// even if package A does not have a dependency on package B.
/// In fact package A may have a dependency on package B version 1, but can still refer to an `authenticate`
/// function defined in package B version 2.
/// Referring to an `authenticate` function with `create_auth_info_v1` is a strictly runtime dependency and
/// it does not collide with any compile time restrictions.
///
/// This function cannot be used in `move unit tests` as there is no mechanism to refer to the package being tested.
public fun create_auth_info_v1<Account: key>(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1<Account> {
    let authenticator_metadata = package_metadata
        .modules_metadata_v1(
            &module_name,
        )
        .authenticator_metadata_v1(&function_name);

    assert!(
        type_name::get<Account>() == authenticator_metadata.account_type(),
        EAuthenticatorInfoNotCompatibleWithAccount,
    );
    AuthenticatorInfoV1 {
        package: package_metadata.storage_id(),
        module_name,
        function_name,
    }
}

/// Create an account as a mutable shared object with the provided `authenticator`.
/// The `authenticator` instance will be added to the account as a dynamic field specified by the `AuthenticatorInfoV1Key` name.
/// This function has custom rules performed by the IOTA Move bytecode verifier that ensures
/// that `Account` is an object defined in the module where `create_account_v1` is invoked.
public fun create_account_v1<Account: key>(
    mut account: Account,
    authenticator: AuthenticatorInfoV1<Account>,
) {
    attach_auth_info_v1(&mut account, authenticator);

    create_account_v1_impl(account);
}

/// Create an account as an immutable object with the provided `authenticator`.
/// The `authenticator` instance will be added to the account as a dynamic field specified by the `AuthenticatorInfoV1Key` name.
/// This function has custom rules performed by the IOTA Move bytecode verifier that ensures
/// that `Account` is an object defined in the module where `create_immutable_account_v1` is invoked.
public fun create_immutable_account_v1<Account: key>(
    mut account: Account,
    authenticator: AuthenticatorInfoV1<Account>,
) {
    attach_auth_info_v1(&mut account, authenticator);

    create_immutable_account_v1_impl(account);
}

/// Rotate the account-related authenticator.
/// The `authenticator` instance will replace the account dynamic field specified by the `AuthenticatorInfoV1Key` name.
/// This function has custom rules performed by the IOTA Move bytecode verifier that ensures
/// that `Account` is an object defined in the module where `rotate_auth_info_v1` is invoked.
public fun rotate_auth_info_v1<Account: key>(
    account: &mut Account,
    authenticator: AuthenticatorInfoV1<Account>,
): AuthenticatorInfoV1<Account> {
    let account_id = borrow_account_uid_mut(account);

    assert!(has_auth_info_v1(account_id), EAuthenticatorInfoV1NotAttached);

    let name = auth_info_v1_key();

    let previous_authenticator_info = dynamic_field::remove(account_id, name);
    dynamic_field::add(account_id, name, authenticator);
    previous_authenticator_info
}

/// Borrow the account-related authenticator.
/// The dynamic field specified by the `AuthenticatorInfoV1Key` name will be returned.
public fun borrow_auth_info_v1<Account: key>(account_id: &UID): &AuthenticatorInfoV1<Account> {
    assert!(has_auth_info_v1(account_id), EAuthenticatorInfoV1NotAttached);
    dynamic_field::borrow(account_id, auth_info_v1_key())
}

/// Check if an authenticator is attached. If a dynamic field with the `AuthenticatorInfoV1Key` name exists.
public fun has_auth_info_v1(account_id: &UID): bool {
    dynamic_field::exists_(account_id, auth_info_v1_key())
}

fun auth_info_v1_key(): AuthenticatorInfoV1Key {
    AuthenticatorInfoV1Key {}
}

/// Add `authenticator` as a dynamic field to `account`.
/// This function must be called only from the account functions protected by the compiler
/// from being called outside the `Account` module.
fun attach_auth_info_v1<Account: key>(
    account: &mut Account,
    authenticator: AuthenticatorInfoV1<Account>,
) {
    let account_id = borrow_account_uid_mut(account);

    assert!(!has_auth_info_v1(account_id), EAuthenticatorInfoV1AlreadyAttached);

    dynamic_field::add(account_id, auth_info_v1_key(), authenticator);
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

/// Create an `AuthenticatorInfoV1` instance for testing, skipping validation.
#[test_only]
public fun create_auth_info_v1_for_testing<Account: key>(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1<Account> {
    AuthenticatorInfoV1 { package: package.to_id(), module_name, function_name }
}
