// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt;

use iota_sdk_types::{ObjectId, StructTag, TypeTag};
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
    identifier::IdentStr,
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS, MoveTypeTagTrait, iota_sdk_types_conversions::struct_tag_sdk_to_core,
};

pub const RESOLVED_IOTA_ID: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!("object"),
    ident_str!("ID"),
);

/// Rust version of the Move iota::object::Info type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct UID {
    pub id: ID,
}

/// Rust version of the Move iota::object::ID type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
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

    pub fn object_id(&self) -> &ObjectId {
        &self.id.bytes
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    pub fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&StructTag::new_uid()),
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

    pub fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&StructTag::new_id()),
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
        TypeTag::Struct(Box::new(StructTag::new_id()))
    }
}
