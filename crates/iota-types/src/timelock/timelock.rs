// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    // annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
    identifier::IdentStr,
    language_storage::{StructTag, TypeTag},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{balance::Balance, base_types::ObjectID, id::UID, TIMELOCK_ADDRESS};

#[cfg(test)]
#[path = "../unit_tests/timelock/timelock_tests.rs"]
mod timelock_tests;

pub const TIMELOCK_MODULE_NAME: &IdentStr = ident_str!("timelock");
pub const TIMELOCK_STRUCT_NAME: &IdentStr = ident_str!("TimeLock");

/// Rust version of the Move stardust::timelock::TimeLock type.
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema, Eq, PartialEq)]
pub struct TimeLock<T> {
    pub id: UID,
    /// The locked object.
    pub locked: T,
    /// This is the epoch time stamp of when the lock expires.
    pub expiration_timestamp_ms: u64,
}

impl<T> TimeLock<T> {
    /// Constructor.
    pub fn new(id: UID, locked: T, expiration_timestamp_ms: u64) -> Self {
        Self {
            id,
            locked,
            expiration_timestamp_ms,
        }
    }

    /// Get the TimeLock's `type`.
    pub fn type_(type_param: TypeTag) -> StructTag {
        StructTag {
            address: TIMELOCK_ADDRESS,
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
}

impl<'de, T> TimeLock<T>
where
    T: Serialize + Deserialize<'de>,
{
    /// Create a `TimeLock` from BCS bytes.
    pub fn from_bcs_bytes(content: &'de [u8]) -> Result<Self, bcs::Error> {
        bcs::from_bytes(content)
    }

    /// Serialize a `TimeLock` as a `Vec<u8>` of BCS.
    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }
}

/// Is this other StructTag representing a TimeLock?
pub fn is_timelock(other: &StructTag) -> bool {
    other.address == TIMELOCK_ADDRESS
        && other.module.as_ident_str() == TIMELOCK_MODULE_NAME
        && other.name.as_ident_str() == TIMELOCK_STRUCT_NAME
}

/// Is this other StructTag representing a TimeLock<Balance<T>>?
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
