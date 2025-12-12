// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use iota_sdk_2::types::{Address, IdentifierRef, StructTag, TypeTag};
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
    identifier::IdentStr,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    MoveTypeTagTrait, base_types::ObjectId, iota_sdk_types_conversions::struct_tag_sdk_to_core,
};

pub const OBJECT_MODULE_NAME_STR: &str = "object";
pub const OBJECT_MODULE_NAME: &IdentifierRef = IdentifierRef::const_new(OBJECT_MODULE_NAME_STR);
pub const UID_STRUCT_NAME: &IdentifierRef = IdentifierRef::const_new("UID");
pub const ID_STRUCT_NAME: &IdentifierRef = IdentifierRef::const_new("ID");
pub const RESOLVED_IOTA_ID: (&AccountAddress, &IdentStr, &IdentStr) = (
    &AccountAddress::new(Address::FRAMEWORK.into_bytes()),
    ident_str!(OBJECT_MODULE_NAME.as_str()),
    ident_str!(ID_STRUCT_NAME.as_str()),
);

/// Rust version of the Move iota::object::Info type
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Eq, PartialEq)]
pub struct UID {
    pub id: ID,
}

/// Rust version of the Move iota::object::ID type
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone, Eq, PartialEq)]
#[serde(transparent)]
pub struct ID {
    pub bytes: ObjectId,
}

impl UID {
    pub fn new(bytes: ObjectId) -> Self {
        Self {
            id: { ID::new(bytes) },
        }
    }

    pub fn type_() -> StructTag {
        StructTag {
            address: Address::FRAMEWORK,
            module: OBJECT_MODULE_NAME.to_owned(),
            name: UID_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn object_id(&self) -> &ObjectId {
        &self.id.bytes
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    pub fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&Self::type_()),
            fields: vec![MoveFieldLayout::new(
                ident_str!("id").to_owned(),
                MoveTypeLayout::Struct(Box::new(ID::layout())),
            )],
        }
    }
}

impl fmt::Display for UID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.id.fmt(f)
    }
}

impl ID {
    pub fn new(object_id: ObjectId) -> Self {
        Self { bytes: object_id }
    }

    pub fn type_() -> StructTag {
        StructTag {
            address: Address::FRAMEWORK,
            module: OBJECT_MODULE_NAME.to_owned(),
            name: ID_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&Self::type_()),
            fields: vec![MoveFieldLayout::new(
                ident_str!("bytes").to_owned(),
                MoveTypeLayout::Address,
            )],
        }
    }
}

impl fmt::Display for ID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.bytes.fmt(f)
    }
}

impl MoveTypeTagTrait for ID {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(Self::type_()))
    }
}
