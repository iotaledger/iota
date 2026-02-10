// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Extension traits for creating `Nft` and `NftOutput` from Stardust types
//! during migration.

use anyhow::anyhow;
use iota_protocol_config::ProtocolConfig;
use iota_stardust_types::block::output::{
    NftOutput as StardustNft, feature::Irc27Metadata as StardustIrc27,
};
use iota_types::{
    balance::Balance,
    base_types::{IotaAddress, ObjectID, SequenceNumber, TxContext},
    collection_types::{Bag, Entry, VecMap},
    id::UID,
    object::{Data, MoveObject, Object, Owner},
    stardust::{
        coin_type::CoinType,
        output::{
            nft::{FixedPoint32, Irc27Metadata, Nft, NftOutput, Url},
            unlock_conditions::{
                ExpirationUnlockCondition, StorageDepositReturnUnlockCondition,
                TimelockUnlockCondition,
            },
        },
    },
};
use num_rational::Ratio;

use super::{
    super::address::stardust_to_iota_address,
    unlock_conditions::{
        ExpirationUnlockConditionExt, StorageDepositReturnUnlockConditionExt,
        TimelockUnlockConditionExt,
    },
};

/// Create a fixed-point value from a rational number specified by its
/// numerator and denominator. Imported from Move std lib.
fn create_fixed_point32_from_rational(numerator: u64, denominator: u64) -> FixedPoint32 {
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

/// Extension trait for FixedPoint32 to support conversion from f64.
pub trait FixedPoint32Ext: Sized {
    fn try_from_f64(value: f64) -> anyhow::Result<Self>;
}

impl FixedPoint32Ext for FixedPoint32 {
    fn try_from_f64(value: f64) -> anyhow::Result<Self> {
        let value = Ratio::from_float(value).ok_or(anyhow!("Missing attribute"))?;
        let numerator = value.numer().clone().try_into()?;
        let denominator = value.denom().clone().try_into()?;
        Ok(create_fixed_point32_from_rational(numerator, denominator))
    }
}

/// Creates the default placeholder Irc27Metadata for NFTs without valid
/// metadata.
pub fn default_irc27_metadata() -> Irc27Metadata {
    // The currently supported version per <https://github.com/iotaledger/tips/blob/main/tips/TIP-0027/tip-0027.md#nft-schema>.
    let version = "v1.0".to_owned();
    // Matches the media type of the URI below.
    let media_type = "image/png".to_owned();
    // A placeholder for NFTs without metadata from which we can extract a URI.
    let uri = Url::try_from("https://opensea.io/static/images/placeholder.png".to_string())
        .expect("url should only contain ascii characters");
    let name = "NFT".to_owned();

    Irc27Metadata {
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

/// Extension trait for creating `Irc27Metadata` from Stardust types.
pub trait Irc27MetadataExt {
    fn try_from_stardust(irc27: StardustIrc27) -> anyhow::Result<Irc27Metadata>;
}

impl Irc27MetadataExt for Irc27Metadata {
    fn try_from_stardust(irc27: StardustIrc27) -> anyhow::Result<Irc27Metadata> {
        Ok(Irc27Metadata {
            version: irc27.version().to_string(),
            media_type: irc27.media_type().to_string(),
            uri: Url::try_from(irc27.uri().clone())?,
            name: irc27.name().to_string(),
            collection_name: irc27.collection_name().clone(),
            royalties: VecMap {
                contents: irc27
                    .royalties()
                    .iter()
                    .map(|(addr, value)| {
                        // The address is a bech32-encoded string, parse it and convert
                        use iota_stardust_types::block::address::Bech32Address;
                        let bech32_addr: Bech32Address = addr.parse().map_err(|e| {
                            anyhow::anyhow!("failed to parse bech32 address: {:?}", e)
                        })?;
                        Ok(Entry {
                            key: stardust_to_iota_address(bech32_addr.inner())?,
                            value: FixedPoint32::try_from_f64(*value)?,
                        })
                    })
                    .collect::<Result<Vec<Entry<IotaAddress, FixedPoint32>>, anyhow::Error>>()?,
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

/// Extension trait for creating `Nft` from Stardust types.
pub trait NftExt {
    /// Creates the Move-based Nft model from a Stardust-based Nft Output.
    fn try_from_stardust(nft_id: ObjectID, nft: &StardustNft) -> Result<Nft, anyhow::Error>;
    /// Converts the immutable metadata of the NFT into an [`Irc27Metadata`].
    fn convert_immutable_metadata(nft: &StardustNft) -> anyhow::Result<Irc27Metadata>;
    /// Creates a genesis object from this NFT.
    fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object>;
}

impl NftExt for Nft {
    fn try_from_stardust(nft_id: ObjectID, nft: &StardustNft) -> Result<Nft, anyhow::Error> {
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
    fn convert_immutable_metadata(nft: &StardustNft) -> anyhow::Result<Irc27Metadata> {
        let Some(metadata) = nft.immutable_features().metadata() else {
            return Ok(default_irc27_metadata());
        };

        if let Ok(parsed_irc27_metadata) = serde_json::from_slice::<StardustIrc27>(metadata.data())
        {
            return Irc27Metadata::try_from_stardust(parsed_irc27_metadata);
        }

        if let Ok(serde_json::Value::Object(json_object)) =
            serde_json::from_slice::<serde_json::Value>(metadata.data())
        {
            let mut irc_metadata = default_irc27_metadata();

            for (key, value) in json_object.into_iter() {
                irc_metadata.non_standard_fields.contents.push(Entry {
                    key,
                    value: value.to_string(),
                })
            }

            return Ok(irc_metadata);
        }

        let mut irc_metadata = default_irc27_metadata();
        let hex_encoded_metadata = hex::encode(metadata.data());
        irc_metadata.non_standard_fields.contents.push(Entry {
            key: "data".to_owned(),
            value: hex_encoded_metadata,
        });
        Ok(irc_metadata)
    }

    fn to_genesis_object(
        &self,
        owner: Owner,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
    ) -> anyhow::Result<Object> {
        // Construct the Nft object.
        let move_nft_object = {
            MoveObject::new_from_execution(
                Nft::tag().into(),
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

/// Extension trait for creating `NftOutput` from Stardust types.
pub trait NftOutputExt {
    /// Creates the Move-based Nft Output model from a Stardust-based Nft
    /// Output.
    fn try_from_stardust(
        object_id: ObjectID,
        nft: &StardustNft,
        native_tokens: Bag,
    ) -> Result<NftOutput, anyhow::Error>;

    /// Creates a genesis object from this NFT output.
    fn to_genesis_object(
        &self,
        owner: IotaAddress,
        protocol_config: &ProtocolConfig,
        tx_context: &TxContext,
        version: SequenceNumber,
        coin_type: CoinType,
    ) -> anyhow::Result<Object>;
}

impl NftOutputExt for NftOutput {
    fn try_from_stardust(
        object_id: ObjectID,
        nft: &StardustNft,
        native_tokens: Bag,
    ) -> Result<NftOutput, anyhow::Error> {
        let unlock_conditions = nft.unlock_conditions();
        Ok(NftOutput {
            id: UID::new(object_id),
            balance: Balance::new(nft.amount()),
            native_tokens,
            storage_deposit_return: unlock_conditions
                .storage_deposit_return()
                .map(StorageDepositReturnUnlockCondition::try_from_stardust)
                .transpose()?,
            timelock: unlock_conditions
                .timelock()
                .map(TimelockUnlockCondition::from_stardust),
            expiration: unlock_conditions
                .expiration()
                .map(|expiration| {
                    ExpirationUnlockCondition::new_from_stardust(nft.address(), expiration)
                })
                .transpose()?,
        })
    }

    fn to_genesis_object(
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
}
