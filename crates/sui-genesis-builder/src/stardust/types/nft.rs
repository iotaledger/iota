use anyhow::anyhow;
use iota_sdk::types::block::output::{
    feature::Irc27Metadata as StardustIrc27, NftOutput as StardustNft,
};
use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};
use num_rational::Ratio;
use packable::PackableExt;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sui_protocol_config::ProtocolConfig;
use sui_types::{
    balance::Balance,
    base_types::{ObjectID, SequenceNumber, SuiAddress, TxContext},
    collection_types::{Bag, Entry, Table, VecMap},
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    STARDUST_PACKAGE_ID,
};

use crate::stardust::error::StardustError;

use super::{
    output::{
        ExpirationUnlockCondition, StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
    },
    stardust_to_sui_address,
};

pub const IRC27_MODULE_NAME: &IdentStr = ident_str!("irc27");
pub const NFT_MODULE_NAME: &IdentStr = ident_str!("nft");
pub const NFT_OUTPUT_MODULE_NAME: &IdentStr = ident_str!("nft_output");
pub const NFT_OUTPUT_STRUCT_NAME: &IdentStr = ident_str!("NftOutput");
pub const NFT_STRUCT_NAME: &IdentStr = ident_str!("Nft");
pub const IRC27_STRUCT_NAME: &IdentStr = ident_str!("Irc27Metadata");

/// Rust version of the Move std::fixed_point32::FixedPoint32 type.
#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct FixedPoint32 {
    pub value: u64,
}

impl FixedPoint32 {
    /// Create a fixed-point value from a rational number specified by its
    /// numerator and denominator. Imported from Move std lib.
    /// This will abort if the denominator is zero. It will also
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
        // Return the quotient as a fixed-point number. We first need to check whether the cast
        // can succeed.
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

/// Rust version of the Move sui::url::Url type.
#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Url {
    pub url: String,
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

    /// The human-readable name of the native token.
    pub name: String,

    /// The human-readable collection name of the native token.
    pub collection_name: Option<String>,

    /// Royalty payment addresses mapped to the payout percentage.
    /// Contains a hash of the 32 bytes parsed from the BECH32 encoded IOTA address in the metadata, it is a legacy address.
    /// Royalties are not supported by the protocol and needed to be processed by an integrator.
    pub royalties: VecMap<SuiAddress, FixedPoint32>,

    /// The human-readable name of the native token creator.
    pub issuer_name: Option<String>,

    /// The human-readable description of the token.
    pub description: Option<String>,

    /// Additional attributes which follow [OpenSea Metadata standards](https://docs.opensea.io/docs/metadata-standards).
    pub attributes: VecMap<String, String>,

    /// Legacy non-standard metadata fields.
    pub non_standard_fields: Table,
}

impl TryFrom<StardustIrc27> for Irc27Metadata {
    type Error = anyhow::Error;
    fn try_from(irc27: StardustIrc27) -> Result<Self, Self::Error> {
        Ok(Self {
            version: irc27.version().to_string(),
            media_type: irc27.media_type().to_string(),
            uri: Url {
                url: irc27.uri().to_string(),
            },
            name: irc27.name().to_string(),
            collection_name: irc27.collection_name().clone(),
            royalties: VecMap {
                contents: irc27
                    .royalties()
                    .iter()
                    .map(|(addr, value)| {
                        Ok(Entry {
                            key: stardust_to_sui_address(addr.inner())?,
                            value: FixedPoint32::try_from(*value)?,
                        })
                    })
                    .collect::<Result<Vec<Entry<SuiAddress, FixedPoint32>>, Self::Error>>()?,
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
            non_standard_fields: Default::default(),
        })
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Nft {
    /// The ID of the Nft = hash of the Output ID that created the Nft Output in Stardust.
    /// This is the NftID from Stardust.
    pub id: UID,

    /// The sender feature holds the last sender address assigned before the migration and
    /// is not supported by the protocol after it.
    pub legacy_sender: Option<SuiAddress>,
    /// The metadata feature.
    pub metadata: Option<Vec<u8>>,
    /// The tag feature.
    pub tag: Option<Vec<u8>>,

    /// The immutable issuer feature.
    pub immutable_issuer: Option<SuiAddress>,
    /// The immutable metadata feature.
    pub immutable_metadata: Irc27Metadata,
}

impl Nft {
    pub fn tag() -> StructTag {
        StructTag {
            address: STARDUST_PACKAGE_ID.into(),
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

        let legacy_sender: Option<SuiAddress> = nft
            .features()
            .sender()
            .map(|sender_feat| stardust_to_sui_address(sender_feat.address()))
            .transpose()?;
        let metadata: Option<Vec<u8>> = nft
            .features()
            .metadata()
            .map(|metadata_feat| metadata_feat.data().to_vec());
        let tag: Option<Vec<u8>> = nft.features().tag().map(|tag_feat| tag_feat.pack_to_vec());
        let immutable_issuer: Option<SuiAddress> = nft
            .immutable_features()
            .issuer()
            .map(|issuer_feat| stardust_to_sui_address(issuer_feat.address()))
            .transpose()?;
        let irc27: StardustIrc27 = serde_json::from_slice(
            nft.immutable_features()
                .metadata()
                .ok_or(StardustError::NftImmutableMetadataNotFound)?
                .data(),
        )
        .map_err(|e| StardustError::Irc27ConversionError {
            nft_id: *nft.nft_id(),
            err: e.into(),
        })?;

        Ok(Nft {
            id: UID::new(nft_id),
            legacy_sender,
            metadata,
            tag,
            immutable_issuer,
            immutable_metadata: irc27.try_into()?,
        })
    }

    pub fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object> {
        // Construct the Nft object.
        let move_nft_object = unsafe {
            // Safety: we know from the definition of `Nft` in the stardust package
            // that it has public transfer (`store` ability is present).
            MoveObject::new_from_execution(
                Self::tag().into(),
                true,
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
    pub iota: Balance,
    /// The `Bag` holds native tokens, key-ed by the stringified type of the asset.
    /// Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>.
    pub native_tokens: Bag,

    /// The storage deposit return unlock condition.
    pub storage_deposit_return: Option<StorageDepositReturnUnlockCondition>,
    /// The timelock unlock condition.
    pub timelock: Option<TimelockUnlockCondition>,
    /// The expiration unlock condition.
    pub expiration: Option<ExpirationUnlockCondition>,
}

impl NftOutput {
    pub fn tag() -> StructTag {
        StructTag {
            address: STARDUST_PACKAGE_ID.into(),
            module: NFT_OUTPUT_MODULE_NAME.to_owned(),
            name: NFT_OUTPUT_STRUCT_NAME.to_owned(),
            type_params: Vec::new(),
        }
    }

    /// Creates the Move-based Nft Output model from a Stardust-based Nft Output.
    pub fn try_from_stardust(
        object_id: ObjectID,
        nft: &StardustNft,
        native_tokens: Bag,
    ) -> Result<Self, anyhow::Error> {
        let unlock_conditions = nft.unlock_conditions();
        Ok(NftOutput {
            id: UID::new(object_id),
            iota: Balance::new(nft.amount()),
            native_tokens,
            storage_deposit_return: unlock_conditions
                .storage_deposit_return()
                .and_then(|unlock| unlock.try_into().ok()),
            timelock: unlock_conditions.timelock().map(|unlock| unlock.into()),
            expiration: nft.try_into().ok(),
        })
    }

    pub fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object> {
        // Construct the Nft Output object.
        let move_nft_output_object = unsafe {
            // Safety: we know from the definition of `NftOutput` in the stardust package
            // that it does not have public transfer (`store` ability is absent).
            MoveObject::new_from_execution(
                NftOutput::tag().into(),
                false,
                version,
                bcs::to_bytes(&self)?,
                protocol_config,
            )?
        };

        let move_nft_output_object = Object::new_from_genesis(
            Data::Move(move_nft_output_object),
            owner,
            tx_context.digest(),
        );

        Ok(move_nft_output_object)
    }
}
