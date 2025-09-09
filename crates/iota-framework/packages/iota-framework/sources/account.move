// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota::account;

use std::ascii;

/// Dynamic field key, where the system will look for a potential
/// authenticate function.
const AUTHENTICATOR_DF_NAME: vector<u8> = b"IOTA_AUTHENTICATION";

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
    create_auth_info_v1_impl(package, module_name.as_bytes(), function_name.as_bytes())
}

native fun create_auth_info_v1_impl(
    package: address,
    module_name: &vector<u8>,
    function_name: &vector<u8>,
): AuthenticatorInfoV1;

/// Returns the dynamic field name where the system will look for an authenticate function.
public fun authenticator_df_name(): vector<u8> {
    AUTHENTICATOR_DF_NAME
}
