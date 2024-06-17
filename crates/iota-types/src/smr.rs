// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};

pub use checked::*;

use crate::IOTA_FRAMEWORK_ADDRESS;

pub const SMR_MODULE_NAME: &IdentStr = ident_str!("iota");
pub const SMR_STRUCT_NAME: &IdentStr = ident_str!("SMR");

#[iota_macros::with_checked_arithmetic]
mod checked {
    use super::*;

    pub struct SMR {}
    impl SMR {
        pub fn type_() -> StructTag {
            StructTag {
                address: IOTA_FRAMEWORK_ADDRESS,
                name: SMR_STRUCT_NAME.to_owned(),
                module: SMR_MODULE_NAME.to_owned(),
                type_params: Vec::new(),
            }
        }

        pub fn type_tag() -> TypeTag {
            TypeTag::Struct(Box::new(Self::type_()))
        }

        pub fn is_smr(other: &StructTag) -> bool {
            &Self::type_() == other
        }

        pub fn is_smr_type(other: &TypeTag) -> bool {
            match other {
                TypeTag::Struct(s) => Self::is_smr(s),
                _ => false,
            }
        }
    }
}
