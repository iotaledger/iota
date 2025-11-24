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
public struct AuthenticatorInfoV1 has copy, drop, store {
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
}

/// Represents a proof of compatibility between `AuthenticatorInfoV1` and an account.
public struct AuthenticatorInfoV1CompatibilityProof has drop {
    account_id: ID,
    authenticator: AuthenticatorInfoV1,
}

/// Checks that the provided `authenticator` is compatible with the given `account`.
/// Returns a proof that can be used to attach or rotate the `authenticator` to the `account`.
public fun check_auth_info_v1_compatibility<Account: key>(
    account: &Account,
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1CompatibilityProof {
    let authenticator_metadata = package_metadata
        .modules_metadata_v1(
            &module_name,
        )
        .authenticator_metadata_v1(&function_name);

    let account_type_name = type_name::get<Account>();
    assert!(
        account_type_name == authenticator_metadata.account_type(),
        EAuthenticatorInfoNotCompatibileWithAccount,
    );
    AuthenticatorInfoV1CompatibilityProof {
        account_id: object::id(account),
        authenticator: AuthenticatorInfoV1 {
            package: package_metadata.storage_id(),
            module_name,
            function_name,
        },
    }
}

/// Attach the `authenticator` instance to the account. It uses a `AuthenticatorInfoV1CompatibilityProof` to obtain that instance.
/// It will be added as a dynamic field specified by the `AuthenticatorInfoV1Key` name.
public fun attach_auth_info_v1(account_id: &mut UID, proof: AuthenticatorInfoV1CompatibilityProof) {
    assert!(account_id.as_inner() == proof.account_id, EAuthenticatorInfoV1CompatibilityNotProven);
    assert!(!has_auth_info_v1(account_id), EAuthenticatorInfoV1AlreadyAttached);

    dynamic_field::add(account_id, auth_info_v1_key(), proof.authenticator);
}

/// Rotate the account-related authenticator.
/// The `authenticator` instance will replace the account dynamic field specified by the `AuthenticatorInfoV1Key` name;
/// It uses a `AuthenticatorInfoV1CompatibilityProof` to obtain the new instance.
public fun rotate_auth_info_v1(account_id: &mut UID, proof: AuthenticatorInfoV1CompatibilityProof) {
    assert!(account_id.as_inner() == proof.account_id, EAuthenticatorInfoV1CompatibilityNotProven);
    assert!(has_auth_info_v1(account_id), EAuthenticatorInfoV1NotAttached);

    let name = auth_info_v1_key();

    dynamic_field::remove<_, AuthenticatorInfoV1>(account_id, name);
    dynamic_field::add(account_id, name, proof.authenticator);
}

/// Borrow the account-related authenticator.
/// The dynamic field specified by the `AuthenticatorInfoV1Key` name will be returned.
public fun borrow_auth_info_v1(account_id: &UID): &AuthenticatorInfoV1 {
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
public fun create_auth_info_v1_for_testing(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1 {
    AuthenticatorInfoV1 { package: package.to_id(), module_name, function_name }
}
