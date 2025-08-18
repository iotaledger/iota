
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

public fun create_auth_info_v1(package: address, module_name: ascii::String, function_name: ascii::String): AuthenticatorInfoV1 {
    create_auth_info_v1_impl(package, module_name.as_bytes(), function_name.as_bytes()) 
}

native fun create_auth_info_v1_impl(package: address, module_name: &vector<u8>, function_name: &vector<u8>): AuthenticatorInfoV1;

public fun drop_auth_info_v1(auth_info: AuthenticatorInfoV1) {
  let AuthenticatorInfoV1 {..} = auth_info;
}