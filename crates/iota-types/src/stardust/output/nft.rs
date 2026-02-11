// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
    collection_types::{Bag, VecMap},
    error::IotaError,
    id::UID,
    object::{Data, Object},
};

pub const IRC27_MODULE_NAME: &IdentStr = ident_str!("irc27");
pub const NFT_MODULE_NAME: &IdentStr = ident_str!("nft");
pub const NFT_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("nft_output");
pub const NFT_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("NftOutput");
pub const NFT_STRUCT_NAME: &IdentStr = ident_str!("Nft");
pub const IRC27_STRUCT_NAME: &IdentStr = ident_str!("Irc27Metadata");
pub const NFT_DYNAMIC_OBJECT_FIELD_KEY: &[u8] = b"nft";
pub const NFT_DYNAMIC_OBJECT_FIELD_KEY_TYPE: &str = "vector<u8>";

/// Rust version of the Move std::fixed_point32::FixedPoint32 type.
#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct FixedPoint32 {
    pub value: u64,
}

/// Rust version of the Move iota::url::Url type.
#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Url {
    /// The underlying URL as a string.
    ///
    /// # SAFETY
    ///
    /// Note that this String is UTF-8 encoded while the URL type in Move is
    /// ascii-encoded. Setting this field requires ensuring that the string
    /// consists of only ASCII characters.
    url: String,
}

impl Url {
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl TryFrom<String> for Url {
    type Error = anyhow::Error;

    /// Creates a new `Url` ensuring that it only consists of ascii characters.
    fn try_from(url: String) -> Result<Self, Self::Error> {
        if !url.is_ascii() {
            anyhow::bail!("url `{url}` does not consist of only ascii characters")
        }
        Ok(Self { url })
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Irc27Metadata {
    /// Version of the metadata standard.
    pub version: String,

    /// The media type (MIME) of the asset.
    ///
    /// ## Examples
    /// - Image files: `image/jpeg`, `image/png`, `image/gif`, etc.
    /// - Video files: `video/x-msvideo` (avi), `video/mp4`, `video/mpeg`, etc.
    /// - Audio files: `audio/mpeg`, `audio/wav`, etc.
    /// - 3D Assets: `model/obj`, `model/u3d`, etc.
    /// - Documents: `application/pdf`, `text/plain`, etc.
    pub media_type: String,

    /// URL pointing to the NFT file location.
    pub uri: Url,

    /// Alphanumeric text string defining the human identifiable name for the
    /// NFT.
    pub name: String,

    /// The human-readable collection name of the NFT.
    pub collection_name: Option<String>,

    /// Royalty payment addresses mapped to the payout percentage.
    /// Contains a hash of the 32 bytes parsed from the BECH32 encoded IOTA
    /// address in the metadata, it is a legacy address. Royalties are not
    /// supported by the protocol and needed to be processed by an integrator.
    pub royalties: VecMap<IotaAddress, FixedPoint32>,

    /// The human-readable name of the NFT creator.
    pub issuer_name: Option<String>,

    /// The human-readable description of the NFT.
    pub description: Option<String>,

    /// Additional attributes which follow [OpenSea Metadata standards](https://docs.opensea.io/docs/metadata-standards).
    pub attributes: VecMap<String, String>,

    /// Legacy non-standard metadata fields.
    pub non_standard_fields: VecMap<String, String>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Nft {
    /// The ID of the Nft = hash of the Output ID that created the Nft Output in
    /// Stardust. This is the NftID from Stardust.
    pub id: UID,

    /// The sender feature holds the last sender address assigned before the
    /// migration and is not supported by the protocol after it.
    pub legacy_sender: Option<IotaAddress>,
    /// The metadata feature.
    pub metadata: Option<Vec<u8>>,
    /// The tag feature.
    pub tag: Option<Vec<u8>>,

    /// The immutable issuer feature.
    pub immutable_issuer: Option<IotaAddress>,
    /// The immutable metadata feature.
    pub immutable_metadata: Irc27Metadata,
}

impl Nft {
    /// Returns the struct tag that represents the fully qualified path of an
    /// [`Nft`] in its move package.
    pub fn tag() -> StructTag {
        StructTag {
            address: STARDUST_ADDRESS,
            module: NFT_MODULE_NAME.to_owned(),
            name: NFT_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct NftOutput {
    /// This is a "random" UID, not the NftID from Stardust.
    pub id: UID,

    /// The amount of IOTA coins held by the output.
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
}

impl NftOutput {
    /// Returns the struct tag that represents the fully qualified path of an
    /// [`NftOutput`] in its move package.
    pub fn tag(type_param: TypeTag) -> StructTag {
        StructTag {
            address: STARDUST_ADDRESS,
            module: NFT_OUTPUT_MODULE_NAME.to_owned(),
            name: NFT_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: vec![type_param],
        }
    }

    /// Create an `NftOutput` from BCS bytes.
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize NftOutput object: {err:?}"),
        })
    }

    pub fn is_nft_output(s: &StructTag) -> bool {
        s.address == STARDUST_ADDRESS
            && s.module.as_ident_str() == NFT_OUTPUT_MODULE_NAME
            && s.name.as_ident_str() == NFT_OUTPUT_STRUCT_NAME
    }
}

impl TryFrom<&Object> for NftOutput {
    type Error = IotaError;
    fn try_from(object: &Object) -> Result<Self, Self::Error> {
        match &object.data {
            Data::Move(o) => {
                if o.type_().is_nft_output() {
                    return NftOutput::from_bcs_bytes(o.contents());
                }
            }
            Data::Package(_) => {}
        }

        Err(IotaError::Type {
            error: format!("Object type is not a NftOutput: {object:?}"),
        })
    }
}
