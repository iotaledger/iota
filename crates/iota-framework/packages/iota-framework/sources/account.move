// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::account;

use iota::object::id_from_bytes;
use iota::types;
use std::ascii;
use std::type_name;

/// Error code for non-OTW structures when publishing authenticate functions.
const ENotOneTimeWitness: u64 = 0;

/// Dynamic field key, where the system will look for a potential
/// authenticate function.
const AUTHENTICATOR_DF_NAME: vector<u8> = b"IOTA_AUTHENTICATION";

#[allow(unused_field)]
public struct AuthenticatorInfoV1 has copy, drop, store {
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
}

/// A record to mark the existence of a unique type, ensuring only one instance per type.
public struct AuthenticateRegistry has key {
    id: UID,
    package: ascii::String,
    module_name: ascii::String,
    function_names: vector<vector<u8>>,
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

native fun check_auth_info_v1(
    package: address,
    module_name: &vector<u8>,
    function_name: &vector<u8>,
);

/// Returns the dynamic field name where the system will look for an authenticate function.
public fun authenticator_df_name(): vector<u8> {
    AUTHENTICATOR_DF_NAME
}

/// Public function to public a new registry of authenticate functions.
/// The `is_one_time_witness` function ensures that this function
/// can only be called once for a specific `T`.
public fun publish_authenticate_registry<OTW: drop>(
    witness: &OTW,
    function_names: vector<vector<u8>>,
    ctx: &mut TxContext,
) {
    // Verify that the type is an OTW
    assert!(types::is_one_time_witness(witness), ENotOneTimeWitness);

    let type_name = type_name::get_with_original_ids<OTW>();

    // Share the record globally
    transfer::freeze_object(AuthenticateRegistry {
        id: object::new(ctx),
        package: type_name.get_address(),
        module_name: type_name.get_module(),
        function_names,
    });
}

public fun create_auth_info_v2(
    registry: &AuthenticateRegistry,
    function_name: ascii::String,
): AuthenticatorInfoV1 {
    assert!(registry.function_names.contains(function_name.as_bytes()), 1);
    AuthenticatorInfoV1 {
        package: id_from_bytes(registry.package.into_bytes()),
        module_name: registry.module_name,
        function_name,
    }
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
