// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Identifier, StructTag};
use serde::{Deserialize, Serialize};

use crate::base_types::IotaAddress;

pub const ACCOUNT_MODULE_NAME: Identifier = Identifier::from_static("account");
pub const AUTHENTICATOR_FUNCTION_REF_V1_KEY_STRUCT_NAME: Identifier =
    Identifier::from_static("AuthenticatorFunctionRefV1Key");

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorFunctionRefV1Key {
    // This field is required to make a Rust struct compatible with an empty Move one.
    // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
    // allowed in the bytecode.
    dummy_field: bool,
}

impl AuthenticatorFunctionRefV1Key {
    pub fn tag() -> StructTag {
        StructTag::new(
            IotaAddress::FRAMEWORK,
            ACCOUNT_MODULE_NAME,
            AUTHENTICATOR_FUNCTION_REF_V1_KEY_STRUCT_NAME,
            Vec::new(),
        )
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }
}
