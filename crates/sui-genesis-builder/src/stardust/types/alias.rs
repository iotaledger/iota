use fastcrypto::hash::{Blake2b256, HashFunction};
use iota_sdk::types::block::output::AliasOutput as StardustAlias;
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sui_types::{
    balance::Balance,
    base_types::{ObjectID, SuiAddress},
    collection_types::Bag,
    id::UID,
    STARDUST_PACKAGE_ID,
};

use super::stardust_to_sui_address;

pub const ALIAS_MODULE_NAME: &IdentStr = ident_str!("alias");
pub const ALIAS_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("alias_output");
pub const ALIAS_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("AliasOutput");
pub const ALIAS_STRUCT_NAME: &IdentStr = ident_str!("Alias");

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Alias {
    /// The ID of the Alias = hash of the Output ID that created the Alias Output in Stardust.
    /// This is the AliasID from Stardust.
    pub id: UID,

    /// The last State Controller address assigned before the migration.
    pub legacy_state_controller: SuiAddress,
    /// A counter increased by 1 every time the alias was state transitioned.
    pub state_index: u32,
    /// State metadata that can be used to store additional information.
    pub state_metadata: Option<Vec<u8>>,

    /// The sender feature.
    pub sender: Option<SuiAddress>,
    /// The metadata feature.
    pub metadata: Option<Vec<u8>>,

    /// The immutable issuer feature.
    pub immutable_issuer: Option<SuiAddress>,
    /// The immutable metadata feature.
    pub immutable_metadata: Option<Vec<u8>>,
}

impl Alias {
    pub fn tag() -> StructTag {
        StructTag {
            address: STARDUST_PACKAGE_ID.into(),
            module: ALIAS_MODULE_NAME.to_owned(),
            name: ALIAS_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    /// Creates the Move-based Alias model from a Stardust-based Alias Output.
    ///
    /// # Warning
    ///
    /// The caller must ensure that `alias_id` is non-zeroed.
    pub fn try_from_stardust(
        alias_id: ObjectID,
        alias: &StardustAlias,
    ) -> Result<Self, anyhow::Error> {
        let state_metadata: Option<Vec<u8>> = if alias.state_metadata().is_empty() {
            None
        } else {
            Some(alias.state_metadata().to_vec())
        };
        let sender: Option<SuiAddress> = alias
            .features()
            .sender()
            .map(|sender_feat| stardust_to_sui_address(sender_feat.address()))
            .transpose()?;
        let metadata: Option<Vec<u8>> = alias
            .features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());
        let immutable_issuer: Option<SuiAddress> = alias
            .immutable_features()
            .issuer()
            .map(|issuer_feat| stardust_to_sui_address(issuer_feat.address()))
            .transpose()?;
        let immutable_metadata: Option<Vec<u8>> = alias
            .immutable_features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());

        Ok(Alias {
            id: UID::new(alias_id),
            legacy_state_controller: stardust_to_sui_address(alias.state_controller_address())?,
            state_index: alias.state_index(),
            state_metadata,
            sender,
            metadata,
            immutable_issuer,
            immutable_metadata,
        })
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct AliasOutput {
    /// This is a "random" UID, not the AliasID from Stardust.
    pub id: UID,

    /// The amount of IOTA coins held by the output.
    pub iota: Balance,
    /// The `Bag` holds native tokens, key-ed by the stringified type of the asset.
    /// Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>.
    pub native_tokens: Bag,
}

impl AliasOutput {
    pub fn tag() -> StructTag {
        StructTag {
            address: STARDUST_PACKAGE_ID.into(),
            module: ALIAS_OUTPUT_MODULE_NAME.to_owned(),
            name: ALIAS_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    /// Creates the Move-based Alias Output model from a Stardust-based Alias Output.
    ///
    /// # Warning
    ///
    /// The caller must ensure that `alias_id` is non-zeroed.
    pub fn try_from_stardust(
        alias_id: ObjectID,
        alias: &StardustAlias,
        native_tokens: Bag,
    ) -> Result<Self, anyhow::Error> {
        // We need an ID that is different from Alias ID to identify the Alias Output.
        // Hashing Alias ID means the generated ID is consistent across runs of the genesis builder.
        let move_alias_output_id = UID::new(ObjectID::new(Blake2b256::digest(alias_id).into()));
        Ok(AliasOutput {
            id: move_alias_output_id.clone(),
            iota: Balance::new(alias.amount()),
            native_tokens,
        })
    }
}
