// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    convert::{TryFrom, TryInto},
    fmt::{Display, Formatter},
};

use move_core_types::{
    annotated_value::MoveStructLayout,
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    balance::{Balance, Supply},
    base_types::{ObjectID, SequenceNumber},
    coin::{Coin, TreasuryCap},
    error::{ExecutionError, ExecutionErrorKind},
    id::UID,
    object::{Data, MoveObject, Object},
};

/// The number of Nanos per IOTA token
pub const NANOS_PER_IOTA: u64 = 1_000_000_000;

/// Total supply in IOTA at genesis, after the migration from a Stardust ledger,
/// before any inflation mechanism
pub const STARDUST_TOTAL_SUPPLY_IOTA: u64 = 4_600_000_000;

// Note: cannot use checked arithmetic here since `const unwrap` is still
// unstable.
/// Total supply at genesis denominated in Nanos, after the migration from a
/// Stardust ledger, before any inflation mechanism
pub const STARDUST_TOTAL_SUPPLY_NANOS: u64 = STARDUST_TOTAL_SUPPLY_IOTA * NANOS_PER_IOTA;

pub const GAS_MODULE_NAME: &IdentStr = ident_str!("iota");
pub const GAS_STRUCT_NAME: &IdentStr = ident_str!("IOTA");
pub const GAS_TREASURY_CAP_STRUCT_NAME: &IdentStr = ident_str!("IotaTreasuryCap");

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
mod checked {
    use super::*;

    pub struct GAS {}
    impl GAS {
        pub fn type_() -> StructTag {
            StructTag {
                address: IOTA_FRAMEWORK_ADDRESS,
                name: GAS_STRUCT_NAME.to_owned(),
                module: GAS_MODULE_NAME.to_owned(),
                type_params: Vec::new(),
            }
        }

        pub fn type_tag() -> TypeTag {
            TypeTag::Struct(Box::new(Self::type_()))
        }

        pub fn is_gas(other: &StructTag) -> bool {
            &Self::type_() == other
        }

        pub fn is_gas_type(other: &TypeTag) -> bool {
            match other {
                TypeTag::Struct(s) => Self::is_gas(s),
                _ => false,
            }
        }
    }

    /// Rust version of the Move iota::coin::Coin<Iota::iota::IOTA> type
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct GasCoin(pub Coin);

    impl GasCoin {
        pub fn new(id: ObjectID, value: u64) -> Self {
            Self(Coin::new(UID::new(id), value))
        }

        pub fn value(&self) -> u64 {
            self.0.value()
        }

        pub fn type_() -> StructTag {
            Coin::type_(TypeTag::Struct(Box::new(GAS::type_())))
        }

        /// Return `true` if `s` is the type of a gas coin (i.e.,
        /// 0x2::coin::Coin<0x2::iota::IOTA>)
        pub fn is_gas_coin(s: &StructTag) -> bool {
            Coin::is_coin(s) && s.type_params.len() == 1 && GAS::is_gas_type(&s.type_params[0])
        }

        /// Return `true` if `s` is the type of a gas balance (i.e.,
        /// 0x2::balance::Balance<0x2::iota::IOTA>)
        pub fn is_gas_balance(s: &StructTag) -> bool {
            Balance::is_balance(s)
                && s.type_params.len() == 1
                && GAS::is_gas_type(&s.type_params[0])
        }

        pub fn id(&self) -> &ObjectID {
            self.0.id()
        }

        pub fn to_bcs_bytes(&self) -> Vec<u8> {
            bcs::to_bytes(&self).unwrap()
        }

        pub fn to_object(&self, version: SequenceNumber) -> MoveObject {
            MoveObject::new_gas_coin(version, *self.id(), self.value())
        }

        pub fn layout() -> MoveStructLayout {
            Coin::layout(TypeTag::Struct(Box::new(GAS::type_())))
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

    impl TryFrom<&MoveObject> for GasCoin {
        type Error = ExecutionError;

        fn try_from(value: &MoveObject) -> Result<GasCoin, ExecutionError> {
            if !value.type_().is_gas_coin() {
                return Err(ExecutionError::new_with_source(
                    ExecutionErrorKind::InvalidGasObject,
                    format!("Gas object type is not a gas coin: {}", value.type_()),
                ));
            }
            let gas_coin: GasCoin = bcs::from_bytes(value.contents()).map_err(|err| {
                ExecutionError::new_with_source(
                    ExecutionErrorKind::InvalidGasObject,
                    format!("Unable to deserialize gas object: {:?}", err),
                )
            })?;
            Ok(gas_coin)
        }
    }

    impl TryFrom<&Object> for GasCoin {
        type Error = ExecutionError;

        fn try_from(value: &Object) -> Result<GasCoin, ExecutionError> {
            match &value.data {
                Data::Move(obj) => obj.try_into(),
                Data::Package(_) => Err(ExecutionError::new_with_source(
                    ExecutionErrorKind::InvalidGasObject,
                    format!("Gas object type is not a gas coin: {:?}", value),
                )),
            }
        }
    }

    impl Display for GasCoin {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Coin {{ id: {}, value: {} }}", self.id(), self.value())
        }
    }

    // Rust version of the IotaTreasuryCap type
    #[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
    pub struct IotaTreasuryCap {
        pub inner: TreasuryCap,
    }

    impl IotaTreasuryCap {
        pub fn type_() -> StructTag {
            StructTag {
                address: IOTA_FRAMEWORK_ADDRESS,
                module: GAS_MODULE_NAME.to_owned(),
                name: GAS_TREASURY_CAP_STRUCT_NAME.to_owned(),
                type_params: Vec::new(),
            }
        }

        /// Returns the `TreasuryCap<IOTA>` object ID.
        pub fn id(&self) -> &ObjectID {
            self.inner.id.object_id()
        }

        /// Returns the total `Supply` of `Coin<IOTA>`.
        pub fn total_supply(&self) -> &Supply {
            &self.inner.total_supply
        }
    }
}
