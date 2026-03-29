// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust types and logic for the Move counterparts in the `stardust` system
//! package.

use anyhow::Result;
use iota_sdk_types::{Identifier, StructTag, TypeTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::unlock_conditions::{
    ExpirationUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
};
use crate::{
    balance::Balance,
    base_types::IotaAddress,
    collection_types::Bag,
    error::IotaError,
    id::UID,
    object::{Data, Object},
};

pub const BASIC_OUTPUT_MODULE_NAME: Identifier = Identifier::from_static("basic_output");
pub const BASIC_OUTPUT_STRUCT_NAME: Identifier = Identifier::from_static("BasicOutput");

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
        StructTag::new(
            IotaAddress::STARDUST,
            BASIC_OUTPUT_MODULE_NAME,
            BASIC_OUTPUT_STRUCT_NAME,
            vec![type_param],
        )
    }

    /// Create a `BasicOutput` from BCS bytes.
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize BasicOutput object: {err:?}"),
        })
    }

    /// Whether the given `StructTag` represents a `BasicOutput`.
    pub fn is_basic_output(s: &StructTag) -> bool {
        s.address() == IotaAddress::STARDUST
            && s.module() == &BASIC_OUTPUT_MODULE_NAME
            && s.name() == &BASIC_OUTPUT_STRUCT_NAME
    }
}

impl TryFrom<&Object> for BasicOutput {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Struct(o) => {
                if BasicOutput::is_basic_output(o.struct_tag()) {
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
