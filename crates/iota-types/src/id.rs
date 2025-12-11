// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use iota_sdk_2::types::Address;
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{MoveTypeTagTrait, base_types::ObjectId};

pub const OBJECT_MODULE_NAME_STR: &str = "object";
pub const OBJECT_MODULE_NAME: &IdentStr = ident_str!(OBJECT_MODULE_NAME_STR);
pub const UID_STRUCT_NAME: &IdentStr = ident_str!("UID");
pub const ID_STRUCT_NAME: &IdentStr = ident_str!("ID");
pub const RESOLVED_IOTA_ID: (&AccountAddress, &IdentStr, &IdentStr) = (
    &AccountAddress::new(Address::FRAMEWORK.into_bytes()),
    OBJECT_MODULE_NAME,
    ID_STRUCT_NAME,
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
            address: AccountAddress::new(Address::FRAMEWORK.into_bytes()),
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
            type_: Self::type_(),
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
            address: AccountAddress::new(Address::FRAMEWORK.into_bytes()),
            module: OBJECT_MODULE_NAME.to_owned(),
            name: ID_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    pub fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: Self::type_(),
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
