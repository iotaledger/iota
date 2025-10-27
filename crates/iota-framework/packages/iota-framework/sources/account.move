// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::account;

use iota::dynamic_field;
use iota::package_metadata::PackageMetadataV1;
use std::ascii;

#[error(code = 0)]
const EAuthenticatorInfoV1AlreadyAttached: vector<u8> =
    b"An `AuthenticatorInfoV1` instance is already attached to the account.";
#[error(code = 1)]
const EAuthenticatorInfoV1NotAttached: vector<u8> =
    b"'AuthenticatorInfoV1' is not attached to the account.";
#[error(code = 2)]
const EFunctionIsNotAuthenticator: vector<u8> =
    b"The specified function is not an 'authenticator' function.";
#[error(code = 3)]
const EUnexpectedAuthenticatorVersion: vector<u8> = b"Unexpected 'authenticator' function version.";

/// Dynamic field key, where the system will look for a potential
/// authenticate function.
public struct AuthenticatorInfoV1Key has copy, drop, store {}

public struct AuthenticatorInfoV1 has copy, drop, store {
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
/// Refiring to an `authenticate` function with `create_auth_info_v1` is a strictly runtime dependency and
/// it does not collide with any compile time restrictions.
///
/// This function cannot be used in `move unit tests` as there is no mechanism to refer to the package being tested.
public fun create_auth_info_v1(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1 {
    check_auth_info_v1(package, module_name.as_bytes(), function_name.as_bytes());
    AuthenticatorInfoV1 {
        package: object::id_from_address(package),
        module_name,
        function_name,
    }
}

public fun create_auth_info_v1_package(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1 {
    let module_handle = package_metadata.find_module_handle(module_name);
    let function_handle = package_metadata.find_function_handle(module_handle, function_name);

    assert!(function_handle.authenticator_version().is_some(), EFunctionIsNotAuthenticator);
    let authenticator_version = function_handle.authenticator_version().extract();

    assert!(authenticator_version == 1, EUnexpectedAuthenticatorVersion);

    AuthenticatorInfoV1 { package: package_metadata.package_id(), module_name, function_name }
}

/// Attach the `authenticator` instance to the account.
/// It will be added as a dynamic field specified by the `AuthenticatorInfoV1Key` name.
public fun attach_auth_info_v1(account_id: &mut UID, authenticator: AuthenticatorInfoV1) {
    assert!(!has_auth_info_v1(account_id), EAuthenticatorInfoV1AlreadyAttached);
    dynamic_field::add(account_id, auth_info_v1_key(), authenticator);
}

/// Rotate the account-related authenticator.
/// The `authenticator` instance will replace the account dynamic field specified by the `AuthenticatorInfoV1Key` name;
/// the previous value will be returned.
public fun rotate_auth_info_v1(
    account_id: &mut UID,
    authenticator: AuthenticatorInfoV1,
): AuthenticatorInfoV1 {
    assert!(has_auth_info_v1(account_id), EAuthenticatorInfoV1NotAttached);

    let name = auth_info_v1_key();

    let previous_authenticator_info = dynamic_field::remove(account_id, name);
    dynamic_field::add(account_id, name, authenticator);
    previous_authenticator_info
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

native fun check_auth_info_v1(
    package: address,
    module_name: &vector<u8>,
    function_name: &vector<u8>,
);

/// Creates an `AuthenticatorInfoV1` instance for testing, skipping validation.
#[test_only]
public fun create_auth_info_v1_for_testing(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorInfoV1 {
    AuthenticatorInfoV1 { package: package.to_id(), module_name, function_name }
}
