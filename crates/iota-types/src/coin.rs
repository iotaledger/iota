// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Identifier, StructTag, TypeTag};
use move_core_types::{
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
};
use serde::{Deserialize, Serialize};

use crate::{
    balance::{Balance, Supply},
    base_types::ObjectID,
    error::{ExecutionError, ExecutionErrorKind, IotaError},
    id::UID,
    iota_sdk_types_conversions::struct_tag_sdk_to_core,
    object::{Data, Object},
};

pub const COIN_JOIN_FUNC_NAME: Identifier = Identifier::from_static("join");

pub const PAY_SPLIT_N_FUNC_NAME: Identifier = Identifier::from_static("divide_and_keep");
pub const PAY_SPLIT_VEC_FUNC_NAME: Identifier = Identifier::from_static("split_vec");

// Rust version of the Move iota::coin::Coin type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Coin {
    pub id: UID,
    pub balance: Balance,
}

impl Coin {
    pub fn new(id: ObjectID, value: u64) -> Self {
        Self {
            id: UID::new(id),
            balance: Balance::new(value),
        }
    }

    /// Create a coin from BCS bytes
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, bcs::Error> {
        bcs::from_bytes(content)
    }

    /// If the given object is a Coin, deserialize its contents and extract the
    /// balance Ok(Some(u64)). If it's not a Coin, return Ok(None).
    /// The cost is 2 comparisons if not a coin, and deserialization if its a
    /// Coin.
    pub fn extract_balance_if_coin(object: &Object) -> Result<Option<u64>, bcs::Error> {
        let Data::Move(obj) = &object.data else {
            return Ok(None);
        };
        let Some(_) = obj.type_().coin_type_opt() else {
            return Ok(None);
        };

        let coin = Self::from_bcs_bytes(obj.contents())?;
        Ok(Some(coin.value()))
    }

    pub fn id(&self) -> &ObjectID {
        self.id.object_id()
    }

    pub fn value(&self) -> u64 {
        self.balance.value()
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    pub fn layout(type_param: TypeTag) -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&StructTag::new_coin(type_param.clone())),
            fields: vec![
                MoveFieldLayout::new(
                    ident_str!("id").to_owned(),
                    MoveTypeLayout::Struct(Box::new(UID::layout())),
                ),
                MoveFieldLayout::new(
                    ident_str!("balance").to_owned(),
                    MoveTypeLayout::Struct(Box::new(Balance::layout(type_param))),
                ),
            ],
        }
    }

    /// Add balance to this coin, erroring if the new total balance exceeds the
    /// maximum
    pub fn add(&mut self, balance: Balance) -> Result<(), ExecutionError> {
        let Some(new_value) = self.value().checked_add(balance.value()) else {
            return Err(ExecutionError::from_kind(
                ExecutionErrorKind::CoinBalanceOverflow,
            ));
        };
        self.balance = Balance::new(new_value);
        Ok(())
    }

    // Split amount out of this coin to a new coin.
    // Related coin objects need to be updated in temporary_store to persist the
    // changes, including creating the coin object related to the newly created
    // coin.
    pub fn split(&mut self, amount: u64, new_coin_id: ObjectID) -> Result<Coin, ExecutionError> {
        self.balance.withdraw(amount)?;
        Ok(Coin::new(new_coin_id, amount))
    }
}

// Rust version of the Move iota::coin::TreasuryCap type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct TreasuryCap {
    pub id: UID,
    pub total_supply: Supply,
}

impl TreasuryCap {
    /// Create a TreasuryCap from BCS bytes
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize TreasuryCap object: {err}"),
        })
    }

    /// Checks if the provided type is `TreasuryCap<T>`, returning the type T if
    /// so.
    pub fn is_treasury_with_coin_type(other: &StructTag) -> Option<&StructTag> {
        if other.is_treasury_cap() {
            match other.type_params().first() {
                Some(TypeTag::Struct(coin_type)) => Some(coin_type),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl TryFrom<Object> for TreasuryCap {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_treasury_cap() {
                    return TreasuryCap::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a TreasuryCap: {object:?}"),
        })
    }
}

// Rust version of the Move iota::coin::CoinMetadata type
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct CoinMetadata {
    pub id: UID,
    /// Number of decimal places the coin uses.
    pub decimals: u8,
    /// Name for the token
    pub name: String,
    /// Symbol for the token
    pub symbol: String,
    /// Description of the token
    pub description: String,
    /// URL for the token logo
    pub icon_url: Option<String>,
}

impl CoinMetadata {
    /// Create a coin from BCS bytes
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize CoinMetadata object: {err}"),
        })
    }

    /// Checks if the provided type is `CoinMetadata<T>`, returning the type T
    /// if so.
    pub fn is_coin_metadata_with_coin_type(other: &StructTag) -> Option<&StructTag> {
        if other.is_coin_metadata() {
            match other.type_params().first() {
                Some(TypeTag::Struct(coin_type)) => Some(coin_type),
                _ => None,
            }
        } else {
            None
        }
    }
}

impl TryFrom<Object> for CoinMetadata {
    type Error = IotaError;
    fn try_from(object: Object) -> Result<Self, Self::Error> {
        TryFrom::try_from(&object)
    }
}

impl TryFrom<&Object> for CoinMetadata {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_coin_metadata() {
                    return CoinMetadata::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a CoinMetadata: {object:?}"),
        })
    }
}
