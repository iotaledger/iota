// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, Identifier, StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    account_abstraction::authenticator_function::AuthenticatorFunctionRefV1,
};

pub const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME: Identifier =
    Identifier::from_static("builtin_authenticator_functions");

pub const PUBLIC_KEY_FIELD_NAME_STRUCT_NAME: Identifier =
    Identifier::from_static("PublicKeyFieldName");

pub const BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME: &str = "builtin_authenticator_v1";

/// The `#[error]` code of `builtin_authenticator_functions::EInvalidSignature`,
/// the abort raised when the built-in authenticator rejects a signature.
pub const INVALID_SIGNATURE_ERROR_CODE: u64 = 20;

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct PublicKeyFieldName {
    // This field is required to make a Rust struct compatible with an empty Move one.
    // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
    // allowed in the bytecode.
    dummy_field: bool,
}

impl PublicKeyFieldName {
    pub fn tag() -> StructTag {
        StructTag::new(
            Address::FRAMEWORK,
            BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME,
            PUBLIC_KEY_FIELD_NAME_STRUCT_NAME,
            Vec::new(),
        )
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).expect("PublicKeyFieldName is always BCS-serializable")
    }
}

/// Returns an authentication function that references the built-in
/// authenticator.
pub fn builtin_authenticator_function_ref_v1() -> AuthenticatorFunctionRefV1 {
    AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME.to_string(),
        function: BUILTIN_AUTHENTICATOR_FUNCTION_V1_NAME.to_string(),
    }
}

/// Returns true if `authenticator_function_ref` references the built-in
/// authenticator.
pub fn is_builtin_authenticator_function_ref(
    authenticator_function_ref: &AuthenticatorFunctionRefV1,
) -> bool {
    *authenticator_function_ref == builtin_authenticator_function_ref_v1()
}

#[cfg(test)]
#[path = "../unit_tests/account_abstraction/builtin_authenticator_functions_tests.rs"]
mod builtin_authenticator_functions_tests;
