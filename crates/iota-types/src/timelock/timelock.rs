// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_FRAMEWORK_ADDRESS,
    balance::Balance,
    base_types::ObjectID,
    error::IotaError,
    gas_coin::GasCoin,
    id::UID,
    object::{Data, Object},
};

pub const TIMELOCK_MODULE_NAME: &IdentStr = ident_str!("timelock");
pub const TIMELOCK_STRUCT_NAME: &IdentStr = ident_str!("TimeLock");

/// All basic outputs whose IDs start with this prefix represent vested rewards
/// that were created during the stardust upgrade on IOTA mainnet.
pub const VESTED_REWARD_ID_PREFIX: &str =
    "0xb191c4bc825ac6983789e50545d5ef07a1d293a98ad974fc9498cb18";

/// Rust version of the Move stardust::TimeLock type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct TimeLock<T> {
    id: UID,
    /// The locked object.
    locked: T,
    /// This is the epoch time stamp of when the lock expires.
    expiration_timestamp_ms: u64,
    /// Timelock related label.
    label: Option<String>,
}

impl<T> TimeLock<T> {
    /// Constructor.
    pub fn new(id: UID, locked: T, expiration_timestamp_ms: u64, label: Option<String>) -> Self {
        Self {
            id,
            locked,
            expiration_timestamp_ms,
            label,
        }
    }

    /// Get the TimeLock's `type`.
    pub fn type_(type_param: TypeTag) -> StructTag {
        StructTag {
            address: IOTA_FRAMEWORK_ADDRESS,
            module: TIMELOCK_MODULE_NAME.to_owned(),
            name: TIMELOCK_STRUCT_NAME.to_owned(),
            type_params: vec![type_param],
        }
    }

    /// Get the TimeLock's `id`.
    pub fn id(&self) -> &ObjectID {
        self.id.object_id()
    }

    /// Get the TimeLock's `locked` object.
    pub fn locked(&self) -> &T {
        &self.locked
    }

    /// Get the TimeLock's `expiration_timestamp_ms`.
    pub fn expiration_timestamp_ms(&self) -> u64 {
        self.expiration_timestamp_ms
    }

    /// Get the TimeLock's `label``.
    pub fn label(&self) -> &Option<String> {
        &self.label
    }
}

impl<'de, T> TimeLock<T>
where
    T: Serialize + Deserialize<'de>,
{
    /// Create a `TimeLock` from BCS bytes.
    pub fn from_bcs_bytes(content: &'de [u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize TimeLock object: {err:?}"),
        })
    }

    /// Serialize a `TimeLock` as a `Vec<u8>` of BCS.
    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }
}

/// Is this other StructTag representing a TimeLock?
pub fn is_timelock(other: &StructTag) -> bool {
    other.address == IOTA_FRAMEWORK_ADDRESS
        && other.module.as_ident_str() == TIMELOCK_MODULE_NAME
        && other.name.as_ident_str() == TIMELOCK_STRUCT_NAME
}

/// Is this other StructTag representing a `TimeLock<Balance<T>>`?
pub fn is_timelocked_balance(other: &StructTag) -> bool {
    if !is_timelock(other) {
        return false;
    }

    if other.type_params.len() != 1 {
        return false;
    }

    match &other.type_params[0] {
        TypeTag::Struct(tag) => Balance::is_balance(tag),
        _ => false,
    }
}

/// Is this other StructTag representing a `TimeLock<Balance<IOTA>>`?
pub fn is_timelocked_gas_balance(other: &StructTag) -> bool {
    if !is_timelock(other) {
        return false;
    }

    if other.type_params.len() != 1 {
        return false;
    }

    match &other.type_params[0] {
        TypeTag::Struct(tag) => GasCoin::is_gas_balance(tag),
        _ => false,
    }
}

impl<'de, T> TryFrom<&'de Object> for TimeLock<T>
where
    T: Serialize + Deserialize<'de>,
{
    type Error = IotaError;

    fn try_from(object: &'de Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_timelock() {
                    return TimeLock::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a TimeLock: {object:?}"),
        })
    }
}
