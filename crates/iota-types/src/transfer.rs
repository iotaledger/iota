// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{ObjectId, StructTag, TypeTag};
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize};

use crate::{IOTA_FRAMEWORK_ADDRESS, base_types::SequenceNumber, id::ID};

pub const RESOLVED_RECEIVING_STRUCT: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!("transfer"),
    ident_str!("Receiving"),
);

/// Rust version of the Move iota::transfer::Receiving type
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Receiving {
    pub id: ID,
    pub version: SequenceNumber,
}

impl Receiving {
    pub fn new(id: ObjectId, version: SequenceNumber) -> Self {
        Self {
            id: ID::new(id),
            version,
        }
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("Value representation is owned and should always serialize")
    }

    pub fn struct_tag(value_type: TypeTag) -> StructTag {
        StructTag::new_transfer_receiving(value_type)
    }

    pub fn type_tag(value_type: TypeTag) -> TypeTag {
        TypeTag::Struct(Box::new(Self::struct_tag(value_type)))
    }

    pub fn is_receiving(view: &CompiledModule, s: &SignatureToken) -> bool {
        use SignatureToken as S;
        match s {
            S::MutableReference(inner) | S::Reference(inner) => Self::is_receiving(view, inner),
            S::DatatypeInstantiation(inst) => {
                let (idx, type_args) = &**inst;
                let struct_tag = resolve_struct(view, *idx);
                struct_tag == RESOLVED_RECEIVING_STRUCT && type_args.len() == 1
            }
            _ => false,
        }
    }
}
