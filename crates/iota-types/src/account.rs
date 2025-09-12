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

pub const AUTHENTICATOR_DF_NAME: &str = "IOTA_AUTHENTICATION";

pub const AUTHENTICATOR_INFO_MODULE_NAME: &IdentStr = ident_str!("account");
pub const AUTHENTICATOR_INFO_V1_STRUCT_NAME: &IdentStr = ident_str!("AuthenticatorInfoV1");

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AuthenticatorInfoV1 {
    pub package: ObjectID,
    pub module: String,
    pub function: String,
}

impl AuthenticatorInfoV1 {
    pub fn tag() -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: AUTHENTICATOR_INFO_MODULE_NAME.to_owned(),
            name: AUTHENTICATOR_INFO_V1_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AuthenticatorInfoV1 object: {err}"),
        })
    }

    pub fn is_authenticator_info_v1(tag: &StructTag) -> bool {
        tag.address == IOTA_FRAMEWORK_ADDRESS
            && tag.module.as_ident_str() == AUTHENTICATOR_INFO_MODULE_NAME
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

pub fn authenticator_df_name_type_tag() -> TypeTag {
    TypeTag::Vector(Box::new(TypeTag::U8))
}

pub fn authenticator_df_name_as_bcs_bytes() -> Vec<u8> {
    bcs::to_bytes(&AUTHENTICATOR_DF_NAME).expect(
        "authenticator dynamic field name serialization is expected to be finished without any errors",
    )
}
