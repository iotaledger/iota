// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, Identifier, StructTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{balance::Balance, collection_types::Bag, error::IotaError, id::UID, object::Object};

pub const ALIAS_OUTPUT_MODULE_NAME: Identifier = Identifier::from_static("alias_output");
pub const ALIAS_OUTPUT_STRUCT_NAME: Identifier = Identifier::from_static("AliasOutput");
pub const ALIAS_DYNAMIC_OBJECT_FIELD_KEY: &[u8] = b"alias";
pub const ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE: &str = "vector<u8>";

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AliasOutput {
    /// This is a "random" UID, not the AliasID from Stardust.
    pub id: UID,

    /// The amount of coins held by the output.
    pub balance: Balance,
    /// The `Bag` holds native tokens, key-ed by the stringified type of the
    /// asset. Example: key: "0xabcded::soon::SOON", value:
    /// Balance<0xabcded::soon::SOON>.
    pub native_tokens: Bag,
}

impl AliasOutput {
    /// Create an `AliasOutput` from BCS bytes.
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AliasOutput object: {err:?}"),
        })
    }

    pub fn is_alias_output(s: &StructTag) -> bool {
        s.address() == Address::STARDUST
            && s.module() == &ALIAS_OUTPUT_MODULE_NAME
            && s.name() == &ALIAS_OUTPUT_STRUCT_NAME
    }
}

impl TryFrom<&Object> for AliasOutput {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        if let Some(o) = object.data.as_opt_struct() {
            if AliasOutput::is_alias_output(o.struct_tag()) {
                return AliasOutput::from_bcs_bytes(o.contents());
            }
        }

        Err(IotaError::Type {
            error: format!("Object type is not an AliasOutput: {object:?}"),
        })
    }
}
