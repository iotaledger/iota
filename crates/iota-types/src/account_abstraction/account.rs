// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};

use crate::IOTA_FRAMEWORK_ADDRESS;

pub const ACCOUNT_MODULE_NAME: &IdentStr = ident_str!("account");
pub const AUTHENTICATOR_FUNCTION_REF_V1_KEY_STRUCT_NAME: &IdentStr =
    ident_str!("AuthenticatorFunctionRefV1Key");

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorFunctionRefV1Key {
    // This field is required to make a Rust struct compatible with an empty Move one.
    // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
    // allowed in the bytecode.
    dummy_field: bool,
}

impl AuthenticatorFunctionRefV1Key {
    pub fn tag() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: ACCOUNT_MODULE_NAME.to_owned(),
            name: AUTHENTICATOR_FUNCTION_REF_V1_KEY_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }
}
