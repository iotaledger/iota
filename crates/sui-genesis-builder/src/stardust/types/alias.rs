use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sui_types::{
    balance::Balance, base_types::SuiAddress, collection_types::Bag, id::UID, STARDUST_PACKAGE_ID,
};

pub const ALIAS_MODULE_NAME: &IdentStr = ident_str!("alias");
pub const ALIAS_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("alias_output");
pub const ALIAS_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("AliasOutput");
pub const ALIAS_STRUCT_NAME: &IdentStr = ident_str!("Alias");
// Matches `ALIAS_NAME` in alias_output.move.
pub const ALIAS_DYNAMIC_OBJECT_FIELD_NAME: &[u8] = b"alias";

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Alias {
    /// The ID of the Alias = hash of the Output ID that created the Alias Output in Stardust.
    /// This is the AliasID from Stardust.
    pub id: UID,

    /// The last State Controller address assigned before the migration.
    pub legacy_state_controller: Option<SuiAddress>,
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
}
