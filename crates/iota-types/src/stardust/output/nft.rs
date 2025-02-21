// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::anyhow;
use iota_protocol_config::ProtocolConfig;
use iota_stardust_sdk::types::block::output::{
    NftOutput as StardustNft, feature::Irc27Metadata as StardustIrc27,
};
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use num_rational::Ratio;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::unlock_conditions::{
    ExpirationUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
};
use crate::{
    STARDUST_ADDRESS, TypeTag,
    balance::Balance,
    base_types::{IotaAddress, ObjectID, SequenceNumber, TxContext},
    collection_types::{Bag, Entry, VecMap},
    error::IotaError,
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    stardust::{coin_type::CoinType, stardust_to_iota_address},
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

impl FixedPoint32 {
    /// Create a fixed-point value from a rational number specified by its
    /// numerator and denominator. Imported from Move std lib.
    /// This will panic if the denominator is zero. It will also
    /// abort if the numerator is nonzero and the ratio is not in the range
    /// 2^-32 .. 2^32-1. When specifying decimal fractions, be careful about
    /// rounding errors: if you round to display N digits after the decimal
    /// point, you can use a denominator of 10^N to avoid numbers where the
    /// very small imprecision in the binary representation could change the
    /// rounding, e.g., 0.0125 will round down to 0.012 instead of up to 0.013.
    fn create_from_rational(numerator: u64, denominator: u64) -> Self {
        // If the denominator is zero, this will abort.
        // Scale the numerator to have 64 fractional bits and the denominator
        // to have 32 fractional bits, so that the quotient will have 32
        // fractional bits.
        let scaled_numerator = (numerator as u128) << 64;
        let scaled_denominator = (denominator as u128) << 32;
        assert!(scaled_denominator != 0);
        let quotient = scaled_numerator / scaled_denominator;
        assert!(quotient != 0 || numerator == 0);
        // Return the quotient as a fixed-point number. We first need to check whether
        // the cast can succeed.
        assert!(quotient <= u64::MAX as u128);
        FixedPoint32 {
            value: quotient as u64,
        }
    }
}

impl TryFrom<f64> for FixedPoint32 {
    type Error = anyhow::Error;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let value = Ratio::from_float(value).ok_or(anyhow!("Missing attribute"))?;
        let numerator = value.numer().clone().try_into()?;
        let denominator = value.denom().clone().try_into()?;
        Ok(FixedPoint32::create_from_rational(numerator, denominator))
    }
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
        Ok(Url { url })
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

impl TryFrom<StardustIrc27> for Irc27Metadata {
    type Error = anyhow::Error;
    fn try_from(irc27: StardustIrc27) -> Result<Self, Self::Error> {
        Ok(Self {
            version: irc27.version().to_string(),
            media_type: irc27.media_type().to_string(),
            // We are converting a `Url` to an ASCII string here (as the URL type in move is based
            // on ASCII strings). The `ToString` implementation of the `Url` ensures
            // only ascii characters are returned and this conversion is therefore safe
            // to do.
            uri: Url::try_from(irc27.uri().to_string())
                .expect("url should only contain ascii characters"),
            name: irc27.name().to_string(),
            collection_name: irc27.collection_name().clone(),
            royalties: VecMap {
                contents: irc27
                    .royalties()
                    .iter()
                    .map(|(addr, value)| {
                        Ok(Entry {
                            key: stardust_to_iota_address(addr.inner())?,
                            value: FixedPoint32::try_from(*value)?,
                        })
                    })
                    .collect::<Result<Vec<Entry<IotaAddress, FixedPoint32>>, Self::Error>>()?,
            },
            issuer_name: irc27.issuer_name().clone(),
            description: irc27.description().clone(),
            attributes: VecMap {
                contents: irc27
                    .attributes()
                    .iter()
                    .map(|attribute| Entry {
                        key: attribute.trait_type().to_string(),
                        value: attribute.value().to_string(),
                    })
                    .collect(),
            },
            non_standard_fields: VecMap {
                contents: Vec::new(),
            },
        })
    }
}

impl Default for Irc27Metadata {
    fn default() -> Self {
        // The currently supported version per <https://github.com/iotaledger/tips/blob/main/tips/TIP-0027/tip-0027.md#nft-schema>.
        let version = "v1.0".to_owned();
        // Matches the media type of the URI below.
        let media_type = "image/png".to_owned();
        // A placeholder for NFTs without metadata from which we can extract a URI.
        let uri = Url::try_from(
            iota_stardust_sdk::Url::parse("https://opensea.io/static/images/placeholder.png")
                .expect("should be a valid url")
                .to_string(),
        )
        .expect("url should only contain ascii characters");
        let name = "NFT".to_owned();

        Self {
            version,
            media_type,
            uri,
            name,
            collection_name: Default::default(),
            royalties: VecMap {
                contents: Vec::new(),
            },
            issuer_name: Default::default(),
            description: Default::default(),
            attributes: VecMap {
                contents: Vec::new(),
            },
            non_standard_fields: VecMap {
                contents: Vec::new(),
            },
        }
    }
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

    /// Creates the Move-based Nft model from a Stardust-based Nft Output.
    pub fn try_from_stardust(nft_id: ObjectID, nft: &StardustNft) -> Result<Self, anyhow::Error> {
        if nft_id.as_ref() == [0; 32] {
            anyhow::bail!("nft_id must be non-zeroed");
        }

        let legacy_sender: Option<IotaAddress> = nft
            .features()
            .sender()
            .map(|sender_feat| stardust_to_iota_address(sender_feat.address()))
            .transpose()?;
        let metadata: Option<Vec<u8>> = nft
            .features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());
        let tag: Option<Vec<u8>> = nft.features().tag().map(|tag_feat| tag_feat.tag().to_vec());
        let immutable_issuer: Option<IotaAddress> = nft
            .immutable_features()
            .issuer()
            .map(|issuer_feat| stardust_to_iota_address(issuer_feat.address()))
            .transpose()?;
        let irc27: Irc27Metadata = Self::convert_immutable_metadata(nft)?;

        Ok(Nft {
            id: UID::new(nft_id),
            legacy_sender,
            metadata,
            tag,
            immutable_issuer,
            immutable_metadata: irc27,
        })
    }

    /// Converts the immutable metadata of the NFT into an [`Irc27Metadata`].
    ///
    /// - If the metadata does not exist returns the default `Irc27Metadata`.
    /// - If the metadata can be parsed into [`StardustIrc27`] returns that
    ///   converted into `Irc27Metadata`.
    /// - If the metadata can be parsed into a JSON object returns the default
    ///   `Irc27Metadata` with `non_standard_fields` set to the fields of the
    ///   object.
    /// - Otherwise, returns the default `Irc27Metadata` with
    ///   `non_standard_fields` containing a `data` key with the hex-encoded
    ///   metadata (without `0x` prefix).
    ///
    /// Note that the metadata feature of the NFT cannot be present _and_ empty
    /// per the protocol rules: <https://github.com/iotaledger/tips/blob/main/tips/TIP-0018/tip-0018.md#additional-syntactic-transaction-validation-rules-2>.
    pub fn convert_immutable_metadata(nft: &StardustNft) -> anyhow::Result<Irc27Metadata> {
        let Some(metadata) = nft.immutable_features().metadata() else {
            return Ok(Irc27Metadata::default());
        };

        if let Ok(parsed_irc27_metadata) = serde_json::from_slice::<StardustIrc27>(metadata.data())
        {
            return Irc27Metadata::try_from(parsed_irc27_metadata);
        }

        if let Ok(serde_json::Value::Object(json_object)) =
            serde_json::from_slice::<serde_json::Value>(metadata.data())
        {
            let mut irc_metadata = Irc27Metadata::default();

            for (key, value) in json_object.into_iter() {
                irc_metadata.non_standard_fields.contents.push(Entry {
                    key,
                    value: value.to_string(),
                })
            }

            return Ok(irc_metadata);
        }

        let mut irc_metadata = Irc27Metadata::default();
        let hex_encoded_metadata = hex::encode(metadata.data());
        irc_metadata.non_standard_fields.contents.push(Entry {
            key: "data".to_owned(),
            value: hex_encoded_metadata,
        });
        Ok(irc_metadata)
    }

    pub fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object> {
        // Construct the Nft object.
        let move_nft_object = {
            MoveObject::new_from_execution(
                Self::tag().into(),
                version,
                bcs::to_bytes(&self)?,
                protocol_config,
            )?
        };

        let move_nft_object = Object::new_from_genesis(
            Data::Move(move_nft_object),
            // We will later overwrite the owner we set here since this object will be added
            // as a dynamic field on the nft output object.
            owner,
            tx_context.digest(),
        );

        Ok(move_nft_object)
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

    /// Creates the Move-based Nft Output model from a Stardust-based Nft
    /// Output.
    pub fn try_from_stardust(
        object_id: ObjectID,
        nft: &StardustNft,
        native_tokens: Bag,
    ) -> Result<Self, anyhow::Error> {
        let unlock_conditions = nft.unlock_conditions();
        Ok(NftOutput {
            id: UID::new(object_id),
            balance: Balance::new(nft.amount()),
            native_tokens,
            storage_deposit_return: unlock_conditions
                .storage_deposit_return()
                .map(|unlock| unlock.try_into())
                .transpose()?,
            timelock: unlock_conditions.timelock().map(|unlock| unlock.into()),
            expiration: unlock_conditions
                .expiration()
                .map(|expiration| ExpirationUnlockCondition::new(nft.address(), expiration))
                .transpose()?,
        })
    }

    pub fn to_genesis_object(
        &self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: CoinType,
    ) -> anyhow::Result<Object> {
        // Construct the Nft Output object.
        let move_nft_output_object = {
            MoveObject::new_from_execution(
                NftOutput::tag(coin_type.to_type_tag()).into(),
                version,
                bcs::to_bytes(&self)?,
                protocol_config,
            )?
        };

        let owner = if self.expiration.is_some() {
            Owner::Shared {
                initial_shared_version: version,
            }
        } else {
            Owner::AddressOwner(owner)
        };

        let move_nft_output_object = Object::new_from_genesis(
            Data::Move(move_nft_output_object),
            owner,
            tx_context.digest(),
        );

        Ok(move_nft_output_object)
    }

    /// Create an `NftOutput` from BCS bytes.
    pub fn from_bcs_bytes(content: &[u8]) -> Result<Self, IotaError> {
        bcs::from_bytes(content).map_err(|err| IotaError::ObjectDeserialization {
            error: format!("Unable to deserialize NftOutput object: {:?}", err),
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
            error: format!("Object type is not a NftOutput: {:?}", object),
        })
    }
}
