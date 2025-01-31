// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_stardust_sdk::types::block::output::AliasOutput as StardustAlias;
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    STARDUST_ADDRESS, TypeTag,
    balance::Balance,
    base_types::{IotaAddress, ObjectID, SequenceNumber, TxContext},
    collection_types::Bag,
    error::IotaError,
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    stardust::{coin_type::CoinType, stardust_to_iota_address},
};

pub const ALIAS_MODULE_NAME: &IdentStr = ident_str!("alias");
pub const ALIAS_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("alias_output");
pub const ALIAS_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("AliasOutput");
pub const ALIAS_STRUCT_NAME: &IdentStr = ident_str!("Alias");
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
        StructTag {
            address: STARDUST_ADDRESS,
            module: ALIAS_MODULE_NAME.to_owned(),
            name: ALIAS_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    /// Creates the Move-based Alias model from a Stardust-based Alias Output.
    pub fn try_from_stardust(
        alias_id: ObjectID,
        alias: &StardustAlias,
    ) -> Result<Self, anyhow::Error> {
        if alias_id.as_ref() == [0; 32] {
            anyhow::bail!("alias_id must be non-zeroed");
        }

        let state_metadata: Option<Vec<u8>> = if alias.state_metadata().is_empty() {
            None
        } else {
            Some(alias.state_metadata().to_vec())
        };
        let sender: Option<IotaAddress> = alias
            .features()
            .sender()
            .map(|sender_feat| stardust_to_iota_address(sender_feat.address()))
            .transpose()?;
        let metadata: Option<Vec<u8>> = alias
            .features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());
        let immutable_issuer: Option<IotaAddress> = alias
            .immutable_features()
            .issuer()
            .map(|issuer_feat| stardust_to_iota_address(issuer_feat.address()))
            .transpose()?;
        let immutable_metadata: Option<Vec<u8>> = alias
            .immutable_features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());

        Ok(Alias {
            id: UID::new(alias_id),
            legacy_state_controller: stardust_to_iota_address(alias.state_controller_address())?,
            state_index: alias.state_index(),
            state_metadata,
            sender,
            metadata,
            immutable_issuer,
            immutable_metadata,
        })
    }

    pub fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object> {
        // Construct the Alias object.
        let move_alias_object = {
            MoveObject::new_from_execution(
                Self::tag().into(),
                version,
                bcs::to_bytes(&self)?,
                protocol_config,
            )?
        };

        let move_alias_object = Object::new_from_genesis(
            Data::Move(move_alias_object),
            // We will later overwrite the owner we set here since this object will be added
            // as a dynamic field on the alias output object.
            owner,
            tx_context.digest(),
        );

        Ok(move_alias_object)
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
        StructTag {
            address: STARDUST_ADDRESS,
            module: ALIAS_OUTPUT_MODULE_NAME.to_owned(),
            name: ALIAS_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: vec![type_param],
        }
    }

    /// Creates the Move-based Alias Output model from a Stardust-based Alias
    /// Output.
    pub fn try_from_stardust(
        object_id: ObjectID,
        alias: &StardustAlias,
        native_tokens: Bag,
    ) -> Result<Self, anyhow::Error> {
        Ok(AliasOutput {
            id: UID::new(object_id),
            balance: Balance::new(alias.amount()),
            native_tokens,
        })
    }

    pub fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: CoinType,
    ) -> anyhow::Result<Object> {
        // Construct the Alias Output object.
        let move_alias_output_object = {
            MoveObject::new_from_execution(
                AliasOutput::tag(coin_type.to_type_tag()).into(),
                version,
                bcs::to_bytes(&self)?,
                protocol_config,
            )?
        };

        let move_alias_output_object = Object::new_from_genesis(
            Data::Move(move_alias_output_object),
            owner,
            tx_context.digest(),
        );

        Ok(move_alias_output_object)
    }

    /// Create an `AliasOutput` from BCS bytes.
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize AliasOutput object: {:?}", err),
        })
    }

    pub fn is_alias_output(s: &StructTag) -> bool {
        s.address == STARDUST_ADDRESS
            && s.module.as_ident_str() == ALIAS_OUTPUT_MODULE_NAME
            && s.name.as_ident_str() == ALIAS_OUTPUT_STRUCT_NAME
    }
}

impl TryFrom<&Object> for AliasOutput {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_alias_output() {
                    return AliasOutput::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not an AliasOutput: {:?}", object),
        })
    }
}
