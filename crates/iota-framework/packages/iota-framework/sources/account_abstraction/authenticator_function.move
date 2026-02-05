// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::authenticator_function;

use iota::package_metadata::PackageMetadataV1;
use std::ascii;
use std::type_name;

#[error(code = 0)]
const EAuthenticatorFunctionRefV1NotCompatibleWithAccount: vector<u8> =
    b"The provided `AuthenticatorFunctionRefV1` is not compatible with the account type.";

/// Represents a validated authenticate function.
#[allow(unused_field)]
public struct AuthenticatorFunctionRefV1<phantom Account: key> has copy, drop, store {
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
}

/// Create an "AuthenticatorFunctionRefV1" using an `authenticate` function defined outside of this version of the package
///
/// The referred `package`, `module_name`, `function_name` can refer to any valid `authenticate` function,
/// regardless of package dependencies or versions.
/// For example package A has two versions V1 and V2. V2 of package A may refer to an `authenticate`
/// function defined in V1. Or it can refer to any package B with an appropriate `authenticate` function
/// even if package A does not have a dependency on package B.
/// In fact package A may have a dependency on package B version 1, but can still refer to an `authenticate`
/// function defined in package B version 2.
/// Referring to an `authenticate` function with `create_auth_function_ref_v1` is a strictly runtime dependency and
/// it does not collide with any compile time restrictions.
///
/// This function cannot be used in `move unit tests` as there is no mechanism to refer to the package being tested.
public fun create_auth_function_ref_v1<Account: key>(
    package_metadata: &PackageMetadataV1,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorFunctionRefV1<Account> {
    let authenticator_metadata = package_metadata
        .modules_metadata_v1(
            &module_name,
        )
        .authenticator_metadata_v1(&function_name);

    assert!(
        type_name::get<Account>() == authenticator_metadata.account_type(),
        EAuthenticatorFunctionRefV1NotCompatibleWithAccount,
    );
    AuthenticatorFunctionRefV1 {
        package: package_metadata.storage_id(),
        module_name,
        function_name,
    }
}

/// Return the storage ID of the package represented by `AuthenticatorFunctionRefV1`.
public fun package<Account: key>(self: &AuthenticatorFunctionRefV1<Account>): ID {
    self.package
}

/// Return the name of the module represented by `AuthenticatorFunctionRefV1`.
public fun module_name<Account: key>(self: &AuthenticatorFunctionRefV1<Account>): &ascii::String {
    &self.module_name
}

/// Return the name of the function represented by `AuthenticatorFunctionRefV1`.
public fun function_name<Account: key>(self: &AuthenticatorFunctionRefV1<Account>): &ascii::String {
    &self.function_name
}

/// Create an `AuthenticatorFunctionRefV1` instance for testing, skipping validation.
#[test_only]
public fun create_auth_function_ref_v1_for_testing<Account: key>(
    package: address,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorFunctionRefV1<Account> {
    AuthenticatorFunctionRefV1 { package: package.to_id(), module_name, function_name }
}
