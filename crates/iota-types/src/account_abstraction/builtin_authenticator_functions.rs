// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, Identifier, StructTag};
use serde::{Deserialize, Serialize};

pub const BUILTIN_AUTHENTICATOR_FUNCTIONS_MODULE_NAME: Identifier =
    Identifier::from_static("builtin_authenticator_functions");

pub const PUBLIC_KEY_FIELD_NAME_STRUCT_NAME: Identifier =
    Identifier::from_static("PublicKeyFieldName");

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
