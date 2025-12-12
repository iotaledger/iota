// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_2::types::{Address, IdentifierRef, StructTag, TypeTag};
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize};

use crate::{
    base_types::{ObjectId, Version},
    id::ID,
};

const TRANSFER_MODULE_NAME: &IdentifierRef = IdentifierRef::const_new("transfer");
const RECEIVING_STRUCT_NAME: &IdentifierRef = IdentifierRef::const_new("Receiving");

pub const RESOLVED_RECEIVING_STRUCT: (&AccountAddress, &IdentStr, &IdentStr) = (
    &AccountAddress::new(Address::FRAMEWORK.into_bytes()),
    ident_str!(TRANSFER_MODULE_NAME.as_str()),
    ident_str!(RECEIVING_STRUCT_NAME.as_str()),
);

/// Rust version of the Move iota::transfer::Receiving type
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Receiving {
    pub id: ID,
    pub version: Version,
}

impl Receiving {
    pub fn new(id: ObjectId, version: Version) -> Self {
        Self {
            id: ID::new(id),
            version,
        }
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(self).expect("Value representation is owned and should always serialize")
    }

    pub fn struct_tag() -> StructTag {
        StructTag {
            address: Address::FRAMEWORK,
            module: TRANSFER_MODULE_NAME.to_owned(),
            name: RECEIVING_STRUCT_NAME.to_owned(),
            // TODO: this should really include the type parameters eventually when we add type
            // parameters to the other polymorphic types like this.
            type_params: vec![],
        }
    }

    pub fn type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(Self::struct_tag()))
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
