// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::account;

use iota::address::from_ascii_bytes;
use iota::object::id_from_address;
use iota::types;
use std::ascii;
use std::type_name;

/// Error code for non-OTW structures during the authenticator info creation.
const ENotAuthenticateOneTimeWitness: u64 = 0;
/// Error code empty function names during the authenticator info creation.
const EAuthFnNameIsEmpty: u64 = 1;

/// Dynamic field key, where the system will look for a potential
/// authenticate function.
const AUTHENTICATOR_DF_NAME: vector<u8> = b"IOTA_AUTHENTICATION";

// The length of the prefix used for AOTW
const AOTW_PREFIX_LEN: u64 = 5; //AUTH_

#[allow(unused_field)]
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

public fun create_auth_info_v1_fotw<AOTW: drop>(): AuthenticatorInfoV1 {
    // Verify that the type is an AOTW
    assert!(types::is_authenticate_one_time_witness<AOTW>(), ENotAuthenticateOneTimeWitness);

    let type_name = type_name::get_with_original_ids<AOTW>();
    let package = id_from_address(from_ascii_bytes(type_name.get_address().as_bytes()));
    let module_name = type_name.get_module();
    let struct_name = type_name.get_struct();

    // Remove the AOTW prefix and convert to lowercase
    let function_name = struct_name.substring(AOTW_PREFIX_LEN, struct_name.length()).to_lowercase();
    assert!(!function_name.is_empty(), EAuthFnNameIsEmpty);

    AuthenticatorInfoV1 {
        package,
        module_name,
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
