// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    base_types::ObjectID,
    error::IotaError,
    object::{Data, Object},
};

pub const ACCOUNT_MODULE_NAME: &IdentStr = ident_str!("account");
pub const AUTHENTICATOR_INFO_V1_STRUCT_NAME: &IdentStr = ident_str!("AuthenticatorInfoV1");
pub const AUTHENTICATOR_INFO_V1_KEY_STRUCT_NAME: &IdentStr = ident_str!("AuthenticatorInfoV1Key");

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorInfoV1 {
    pub package: ObjectID,
    pub module: String,
    pub function: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorInfoV1Key {
    // This field is required to make a Rust struct compatible with an empty Move one.
    // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
    // allowed in the bytecode.
    dummy_field: bool,
}

impl AuthenticatorInfoV1 {
    pub fn type_(type_param: StructTag) -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: ACCOUNT_MODULE_NAME.to_owned(),
            name: AUTHENTICATOR_INFO_V1_STRUCT_NAME.to_owned(),
            type_params: vec![TypeTag::Struct(Box::new(type_param))],
        }
    }

    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AuthenticatorInfoV1 object: {err}"),
        })
    }

    pub fn is_authenticator_info_v1(tag: &StructTag) -> bool {
        tag.address == IOTA_FRAMEWORK_ADDRESS
            && tag.module.as_ident_str() == ACCOUNT_MODULE_NAME
            && tag.name.as_ident_str() == AUTHENTICATOR_INFO_V1_STRUCT_NAME
    }
}

impl TryFrom<Object> for AuthenticatorInfoV1 {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_authenticator_info_v1() {
                    return AuthenticatorInfoV1::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a AuthenticatorInfoV1: {object:?}"),
        })
    }
}

impl AuthenticatorInfoV1Key {
    pub fn tag() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: ACCOUNT_MODULE_NAME.to_owned(),
            name: AUTHENTICATOR_INFO_V1_KEY_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }
}
