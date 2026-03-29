// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Identifier, StructTag, TypeTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    balance::Balance,
    base_types::IotaAddress,
    collection_types::Bag,
    error::IotaError,
    id::UID,
    object::{Data, Object},
};

pub const ALIAS_MODULE_NAME: Identifier = Identifier::from_static("alias");
pub const ALIAS_OUTPUT_MODULE_NAME: Identifier = Identifier::from_static("alias_output");
pub const ALIAS_OUTPUT_STRUCT_NAME: Identifier = Identifier::from_static("AliasOutput");
pub const ALIAS_STRUCT_NAME: Identifier = Identifier::from_static("Alias");
pub const ALIAS_DYNAMIC_OBJECT_FIELD_KEY: &[u8] = b"alias";
pub const ALIAS_DYNAMIC_OBJECT_FIELD_KEY_TYPE: &str = "vector<u8>";

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Alias {
    /// The ID of the Alias = hash of the Output ID that created the Alias
    /// Output in Stardust. This is the AliasID from Stardust.
    pub id: UID,

    /// The last State Controller address assigned before the migration.
    pub legacy_state_controller: IotaAddress,
    /// A counter increased by 1 every time the alias was state transitioned.
    pub state_index: u32,
    /// State metadata that can be used to store additional information.
    pub state_metadata: Option<Vec<u8>>,

    /// The sender feature.
    pub sender: Option<IotaAddress>,
    /// The metadata feature.
    pub metadata: Option<Vec<u8>>,

    /// The immutable issuer feature.
    pub immutable_issuer: Option<IotaAddress>,
    /// The immutable metadata feature.
    pub immutable_metadata: Option<Vec<u8>>,
}

impl Alias {
    /// Returns the struct tag that represents the fully qualified path of an
    /// [`Alias`] in its move package.
    pub fn tag() -> StructTag {
        StructTag::new(
            IotaAddress::STARDUST,
            ALIAS_MODULE_NAME,
            ALIAS_STRUCT_NAME,
            Vec::new(),
        )
    }
}

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
    /// Returns the struct tag that represents the fully qualified path of an
    /// [`AliasOutput`] in its move package.
    pub fn tag(type_param: TypeTag) -> StructTag {
        StructTag::new(
            IotaAddress::STARDUST,
            ALIAS_OUTPUT_MODULE_NAME,
            ALIAS_OUTPUT_STRUCT_NAME,
            vec![type_param],
        )
    }

    /// Create an `AliasOutput` from BCS bytes.
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AliasOutput object: {err:?}"),
        })
    }

    pub fn is_alias_output(s: &StructTag) -> bool {
        s.address() == IotaAddress::STARDUST
            && s.module() == &ALIAS_OUTPUT_MODULE_NAME
            && s.name() == &ALIAS_OUTPUT_STRUCT_NAME
    }
}

impl TryFrom<&Object> for AliasOutput {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Struct(o) => {
                if AliasOutput::is_alias_output(o.struct_tag()) {
                    return AliasOutput::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not an AliasOutput: {object:?}"),
        })
    }
}
