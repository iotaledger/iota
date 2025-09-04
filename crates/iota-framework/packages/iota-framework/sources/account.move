module iota::account;

use std::ascii;

/// Dynamic field key, where the system will look for a potential
/// authenticate function.
#[allow(unused_const)]
const AUTHENTICATOR_ID: vector<u8> = b"IOTA_AUTHENTICATION";

#[allow(unused_field)]
public struct AuthenticatorInfoV1 has store {
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
/// The only scenario which cannot be handled by this function is that of referring to an `authenticate`
/// function from the current version of the package. Simply because the current package address won't be know
/// before publishing, thus the user cannot specify it in code.
/// If an `authenticate` function should be used from the current version of the package please use
/// `create_auth_info_self_v1` instead.
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

public fun drop_auth_info_v1(auth_info: AuthenticatorInfoV1) {
    let AuthenticatorInfoV1 { .. } = auth_info;
}
