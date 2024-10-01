// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use checked::*;
use move_core_types::{
    annotated_value::MoveStructLayout,
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    balance::Balance,
    base_types::{MoveObjectType, ObjectID, SequenceNumber},
    coin::Coin,
    id::UID,
    object::MoveObject,
};

pub const SMR_MODULE_NAME: &IdentStr = ident_str!("smr");
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

    /// Rust version of the Move 0x2::coin::Coin<0x2::smr::SMR> type
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct SmrCoin(pub Coin);

    impl SmrCoin {
        pub fn new(id: ObjectID, value: u64) -> Self {
            Self(Coin::new(UID::new(id), value))
        }

        pub fn value(&self) -> u64 {
            self.0.value()
        }

        pub fn type_() -> StructTag {
            Coin::type_(TypeTag::Struct(Box::new(SMR::type_())))
        }

        /// Return `true` if `s` is the type of a smr coin (i.e.,
        /// 0x2::coin::Coin<0x2::smr::SMR>)
        pub fn is_smr_coin(s: &StructTag) -> bool {
            Coin::is_coin(s) && s.type_params.len() == 1 && SMR::is_smr_type(&s.type_params[0])
        }

        /// Return `true` if `s` is the type of a gas balance (i.e.,
        /// 0x2::balance::Balance<0x2::smr::SMR>)
        pub fn is_smr_balance(s: &StructTag) -> bool {
            Balance::is_balance(s)
                && s.type_params.len() == 1
                && SMR::is_smr_type(&s.type_params[0])
        }

        pub fn id(&self) -> &ObjectID {
            self.0.id()
        }

        pub fn to_bcs_bytes(&self) -> Vec<u8> {
            bcs::to_bytes(&self).unwrap()
        }

        pub fn to_object(&self, version: SequenceNumber) -> MoveObject {
            MoveObject::new_coin(
                MoveObjectType::from(SmrCoin::type_()),
                version,
                *self.id(),
                self.value(),
            )
        }

        pub fn layout() -> MoveStructLayout {
            Coin::layout(TypeTag::Struct(Box::new(SMR::type_())))
        }

        #[cfg(any(feature = "test-utils", test))]
        pub fn new_for_testing(value: u64) -> Self {
            Self::new(ObjectID::random(), value)
        }

        #[cfg(any(feature = "test-utils", test))]
        pub fn new_for_testing_with_id(id: ObjectID, value: u64) -> Self {
            Self::new(id, value)
        }
    }
}
