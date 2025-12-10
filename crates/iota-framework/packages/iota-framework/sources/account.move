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
const EAuthenticatorInfoV1CompatibilityNotProven: vector<u8> =
    b"An `AuthenticatorInfoV1` instance is not verified to be attached to the account.";
#[error(code = 3)]
const EAuthenticatorInfoNotCompatibileWithAccount: vector<u8> =
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

/// Represents a proof of compatibility between `AuthenticatorInfoV1` and an account.
public struct AuthenticatorInfoV1CompatibilityProof<phantom Account: key> has drop {
    account_id: ID,
    authenticator: AuthenticatorInfoV1<Account>,
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
        EAuthenticatorInfoNotCompatibileWithAccount,
    );
    AuthenticatorInfoV1 {
        package: package_metadata.storage_id(),
        module_name,
        function_name,
    }
}

/// Checks that the provided `authenticator` is compatible with the given `account`.
/// Returns a proof that can be used to attach or rotate the `authenticator` to the `account`.
public fun check_auth_info_v1_compatibility<Account: key>(
    account: &Account,
    authenticator: AuthenticatorInfoV1<Account>,
): AuthenticatorInfoV1CompatibilityProof<Account> {
    AuthenticatorInfoV1CompatibilityProof {
        account_id: object::id(account),
        authenticator,
    }
}

/// Attach the `authenticator` instance to the account. It uses a `AuthenticatorInfoV1CompatibilityProof` to obtain that instance.
/// It will be added as a dynamic field specified by the `AuthenticatorInfoV1Key` name.
public fun attach_auth_info_v1<Account: key>(
    account_id: &mut UID,
    proof: AuthenticatorInfoV1CompatibilityProof<Account>,
) {
    assert!(account_id.as_inner() == proof.account_id, EAuthenticatorInfoV1CompatibilityNotProven);
    assert!(!has_auth_info_v1(account_id), EAuthenticatorInfoV1AlreadyAttached);

    dynamic_field::add(account_id, auth_info_v1_key(), proof.authenticator);
}

/// Rotate the account-related authenticator.
/// The `authenticator` instance will replace the account dynamic field specified by the `AuthenticatorInfoV1Key` name;
/// It uses a `AuthenticatorInfoV1CompatibilityProof` to obtain the new instance.
public fun rotate_auth_info_v1<Account: key>(
    account_id: &mut UID,
    proof: AuthenticatorInfoV1CompatibilityProof<Account>,
): AuthenticatorInfoV1<Account> {
    assert!(account_id.as_inner() == proof.account_id, EAuthenticatorInfoV1CompatibilityNotProven);
    assert!(has_auth_info_v1(account_id), EAuthenticatorInfoV1NotAttached);

    let name = auth_info_v1_key();

    let previous_authenticator_info = dynamic_field::remove<_, AuthenticatorInfoV1<Account>>(
        account_id,
        name,
    );
    dynamic_field::add(account_id, name, proof.authenticator);
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

/// Creates an `AuthenticatorInfoV1` instance for testing, skipping validation.
#[test_only]
public fun create_auth_info_v1_for_testing<Account: key>(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1<Account> {
    AuthenticatorInfoV1 { package: package.to_id(), module_name, function_name }
}
