// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust types and logic for the Move counterparts in the `stardust` system
//! package.

use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::unlock_conditions::{
    ExpirationUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
};
use crate::{
    STARDUST_ADDRESS, TypeTag,
    balance::Balance,
    base_types::IotaAddress,
    collection_types::Bag,
    error::IotaError,
    id::UID,
    object::{Data, Object},
};

pub const BASIC_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("basic_output");
pub const BASIC_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("BasicOutput");

/// Rust version of the stardust basic output.
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct BasicOutput {
    /// Hash of the `OutputId` that was migrated.
    pub id: UID,

    /// The amount of coins held by the output.
    pub balance: Balance,

    /// The `Bag` holds native tokens, key-ed by the stringified type of the
    /// asset. Example: key: "0xabcded::soon::SOON", value:
    /// Balance<0xabcded::soon::SOON>.
    pub native_tokens: Bag,

    /// The storage deposit return unlock condition.
    pub storage_deposit_return: Option<StorageDepositReturnUnlockCondition>,
    /// The timelock unlock condition.
    pub timelock: Option<TimelockUnlockCondition>,
    /// The expiration unlock condition.
    pub expiration: Option<ExpirationUnlockCondition>,

    // Possible features, they have no effect and only here to hold data until the object is
    // deleted.
    /// The metadata feature.
    pub metadata: Option<Vec<u8>>,
    /// The tag feature.
    pub tag: Option<Vec<u8>>,
    /// The sender feature.
    pub sender: Option<IotaAddress>,
}

impl BasicOutput {
    /// Returns the struct tag of the BasicOutput struct
    pub fn tag(type_param: TypeTag) -> StructTag {
        StructTag {
            address: STARDUST_ADDRESS,
            module: BASIC_OUTPUT_MODULE_NAME.to_owned(),
            name: BASIC_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: vec![type_param],
        }
    }

    /// Create a `BasicOutput` from BCS bytes.
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize BasicOutput object: {err:?}"),
        })
    }

    /// Whether the given `StructTag` represents a `BasicOutput`.
    pub fn is_basic_output(s: &StructTag) -> bool {
        s.address == STARDUST_ADDRESS
            && s.module.as_ident_str() == BASIC_OUTPUT_MODULE_NAME
            && s.name.as_ident_str() == BASIC_OUTPUT_STRUCT_NAME
    }
}

impl TryFrom<&Object> for BasicOutput {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_basic_output() {
                    return BasicOutput::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a BasicOutput: {object:?}"),
        })
    }
}
