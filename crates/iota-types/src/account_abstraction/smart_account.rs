// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, Identifier, MoveStruct, ObjectId, Owner, StructTag, Version};
use serde::{Deserialize, Serialize};

use crate::{
    digests::TransactionDigest,
    id::UID,
    object::{OBJECT_START_VERSION, Object},
};

pub const SMART_ACCOUNT_MODULE_NAME: Identifier = Identifier::from_static("smart_account");
pub const SMART_ACCOUNT_STRUCT_NAME: Identifier = Identifier::from_static("SmartAccount");

const IMPLICIT_SMART_ACCOUNT_OBJECT_VERSION: Version = OBJECT_START_VERSION;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct SmartAccount {
    pub id: UID,
}

impl SmartAccount {
    pub fn new(id: ObjectId) -> Self {
        Self { id: UID::new(id) }
    }

    // Return the data needed for creating a synthetic implicit smart account
    pub fn to_synthetic_implicit_account_object(id: ObjectId) -> Object {
        Self::new(id).to_object()
    }

    pub fn tag() -> StructTag {
        StructTag::new(
            Address::FRAMEWORK,
            SMART_ACCOUNT_MODULE_NAME,
            SMART_ACCOUNT_STRUCT_NAME,
            Vec::new(),
        )
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    fn to_object(&self) -> Object {
        let move_struct = MoveStruct::new(
            Self::tag().into(),
            IMPLICIT_SMART_ACCOUNT_OBJECT_VERSION,
            bcs::to_bytes(self).expect("should serialize a SmartAccount into bytes"),
        )
        .expect("should fail move struct size limits");

        let owner = Owner::Shared(move_struct.version());
        Object::new_move(move_struct, owner, TransactionDigest::GENESIS_MARKER)
    }
}
