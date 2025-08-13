// Copyright 2021-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use alloc::{boxed::Box, string::String, vec::Vec};
use core::{ops::RangeInclusive, str::FromStr};

use packable::{bounded::BoundedU16, prefix::BoxedSlicePrefix};

use crate::types::block::Error;

pub(crate) type MetadataFeatureLength = BoundedU16<
    { *MetadataFeature::LENGTH_RANGE.start() },
    { *MetadataFeature::LENGTH_RANGE.end() },
>;

/// Defines metadata, arbitrary binary data, that will be stored in the output.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, packable::Packable)]
#[packable(unpack_error = Error, with = |err| Error::InvalidMetadataFeatureLength(err.into_prefix_err().into()))]
pub struct MetadataFeature(
    // Binary data.
    BoxedSlicePrefix<u8, MetadataFeatureLength>,
);

macro_rules! impl_from_vec {
    ($type:ty) => {
        impl TryFrom<$type> for MetadataFeature {
            type Error = Error;

            fn try_from(value: $type) -> Result<Self, Self::Error> {
                Vec::<u8>::from(value).try_into()
            }
        }
    };
}
impl_from_vec!(&str);
impl_from_vec!(String);
impl_from_vec!(&[u8]);

impl<const N: usize> TryFrom<[u8; N]> for MetadataFeature {
    type Error = Error;

    fn try_from(value: [u8; N]) -> Result<Self, Self::Error> {
        value.to_vec().try_into()
    }
}

impl TryFrom<Vec<u8>> for MetadataFeature {
    type Error = Error;

    fn try_from(data: Vec<u8>) -> Result<Self, Error> {
        data.into_boxed_slice().try_into()
    }
}

impl TryFrom<Box<[u8]>> for MetadataFeature {
    type Error = Error;

    fn try_from(data: Box<[u8]>) -> Result<Self, Error> {
        data.try_into()
            .map(Self)
            .map_err(Error::InvalidMetadataFeatureLength)
    }
}

impl FromStr for MetadataFeature {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(prefix_hex::decode::<Vec<u8>>(s).map_err(Error::Hex)?)
    }
}

impl MetadataFeature {
    /// The [`Feature`](crate::types::block::output::Feature) kind of
    /// [`MetadataFeature`].
    pub const KIND: u8 = 2;
    /// Valid lengths for a [`MetadataFeature`].
    pub const LENGTH_RANGE: RangeInclusive<u16> = 1..=8192;

    /// Creates a new [`MetadataFeature`].
    #[inline(always)]
    pub fn new(data: impl Into<Vec<u8>>) -> Result<Self, Error> {
        Self::try_from(data.into())
    }

    /// Returns the data.
    #[inline(always)]
    pub fn data(&self) -> &[u8] {
        &self.0
    }
}

impl core::fmt::Display for MetadataFeature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", prefix_hex::encode(self.data()))
    }
}

impl core::fmt::Debug for MetadataFeature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MetadataFeature({self})")
    }
}

#[cfg(feature = "irc_27")]
pub(crate) mod irc_27 {
    use alloc::{
        borrow::ToOwned,
        collections::{BTreeMap, BTreeSet},
        string::String,
    };

    use getset::Getters;
    use serde::{Deserialize, Serialize};
    use url::Url;

    use super::*;
    use crate::types::block::address::Bech32Address;

    /// The IRC27 NFT standard schema.
    #[derive(Clone, Debug, Serialize, Deserialize, Getters, PartialEq)]
    #[serde(rename_all = "camelCase")]
    #[serde(tag = "standard", rename = "IRC27")]
    #[getset(get = "pub")]
    pub struct Irc27Metadata {
        version: String,
        /// The media type (MIME) of the asset.
        ///
        /// ## Examples
        /// - Image files: `image/jpeg`, `image/png`, `image/gif`, etc.
        /// - Video files: `video/x-msvideo` (avi), `video/mp4`, `video/mpeg`,
        ///   etc.
        /// - Audio files: `audio/mpeg`, `audio/wav`, etc.
        /// - 3D Assets: `model/obj`, `model/u3d`, etc.
        /// - Documents: `application/pdf`, `text/plain`, etc.
        #[serde(rename = "type")]
        media_type: String,
        /// URL pointing to the NFT file location.
        uri: Url,
        /// The human-readable name of the native token.
        name: String,
        /// The human-readable collection name of the native token.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        collection_name: Option<String>,
        /// Royalty payment addresses mapped to the payout percentage.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        royalties: BTreeMap<Bech32Address, f64>,
        /// The human-readable name of the native token creator.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        issuer_name: Option<String>,
        /// The human-readable description of the token.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// Additional attributes which follow [OpenSea Metadata standards](https://docs.opensea.io/docs/metadata-standards).
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        attributes: BTreeSet<Attribute>,
    }

    impl Irc27Metadata {
        pub fn new(media_type: impl Into<String>, uri: Url, name: impl Into<String>) -> Self {
            Self {
                version: "v1.0".to_owned(),
                media_type: media_type.into(),
                uri,
                name: name.into(),
                collection_name: Default::default(),
                royalties: Default::default(),
                issuer_name: Default::default(),
                description: Default::default(),
                attributes: Default::default(),
            }
        }

        pub fn with_collection_name(mut self, collection_name: impl Into<String>) -> Self {
            self.collection_name.replace(collection_name.into());
            self
        }

        pub fn add_royalty(mut self, address: Bech32Address, percentage: f64) -> Self {
            self.royalties.insert(address, percentage);
            self
        }

        pub fn with_royalties(mut self, royalties: BTreeMap<Bech32Address, f64>) -> Self {
            self.royalties = royalties;
            self
        }

        pub fn with_issuer_name(mut self, issuer_name: impl Into<String>) -> Self {
            self.issuer_name.replace(issuer_name.into());
            self
        }

        pub fn with_description(mut self, description: impl Into<String>) -> Self {
            self.description.replace(description.into());
            self
        }

        pub fn add_attribute(mut self, attribute: Attribute) -> Self {
            self.attributes.insert(attribute);
            self
        }

        pub fn with_attributes(mut self, attributes: BTreeSet<Attribute>) -> Self {
            self.attributes = attributes;
            self
        }

        pub fn to_bytes(&self) -> Vec<u8> {
            // Unwrap: Safe because this struct is known to be valid
            serde_json::to_string(self).unwrap().into_bytes()
        }
    }

    impl TryFrom<Irc27Metadata> for MetadataFeature {
        type Error = Error;
        fn try_from(value: Irc27Metadata) -> Result<Self, Error> {
            Self::new(value.to_bytes())
        }
    }

    impl From<Irc27Metadata> for Vec<u8> {
        fn from(value: Irc27Metadata) -> Self {
            value.to_bytes()
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Getters, PartialEq, Eq)]
    #[getset(get = "pub")]
    pub struct Attribute {
        trait_type: String,
        value: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display_type: Option<String>,
    }

    impl Attribute {
        pub fn new(trait_type: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
            Self {
                trait_type: trait_type.into(),
                display_type: None,
                value: value.into(),
            }
        }

        pub fn with_display_type(mut self, display_type: impl Into<String>) -> Self {
            self.display_type.replace(display_type.into());
            self
        }
    }

    impl Ord for Attribute {
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            self.trait_type.cmp(&other.trait_type)
        }
    }
    impl PartialOrd for Attribute {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl core::hash::Hash for Attribute {
        fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
            self.trait_type.hash(state);
        }
    }

    #[cfg(test)]
    mod test {
        use pretty_assertions::assert_eq;

        use super::*;
        use crate::types::block::{address::ToBech32Ext, rand::address::rand_address};

        #[test]
        fn serialization() {
            let metadata = Irc27Metadata::new(
                "image/jpeg",
                "https://mywebsite.com/my-nft-files-1.jpeg".parse().unwrap(),
                "My NFT #0001",
            )
            .with_collection_name("My Collection of Art")
            .add_royalty(rand_address().to_bech32_unchecked("iota1"), 0.025)
            .add_royalty(rand_address().to_bech32_unchecked("iota1"), 0.025)
            .with_issuer_name("My Artist Name")
            .with_description("A little information about my NFT collection")
            .add_attribute(Attribute::new("Background", "Purple"))
            .add_attribute(Attribute::new("Element", "Water"))
            .add_attribute(Attribute::new("Attack", 150))
            .add_attribute(Attribute::new("Health", 500));
            let json = serde_json::json!(
                {
                    "standard": "IRC27",
                    "version": metadata.version(),
                    "type": metadata.media_type(),
                    "uri": metadata.uri(),
                    "name": metadata.name(),
                    "collectionName": metadata.collection_name(),
                    "royalties": metadata.royalties(),
                    "issuerName": metadata.issuer_name(),
                    "description": metadata.description(),
                    "attributes": metadata.attributes()
                  }
            );
            let metadata_deser = serde_json::from_value::<Irc27Metadata>(json.clone()).unwrap();

            assert_eq!(metadata, metadata_deser);
            assert_eq!(json, serde_json::to_value(metadata).unwrap())
        }
    }
}

#[cfg(feature = "irc_30")]
pub(crate) mod irc_30 {
    use alloc::string::String;

    use getset::Getters;
    use serde::{Deserialize, Serialize};
    use url::Url;

    use super::*;

    /// The IRC30 native token metadata standard schema.
    #[derive(Clone, Debug, Serialize, Deserialize, Getters, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    #[serde(tag = "standard", rename = "IRC30")]
    #[getset(get = "pub")]
    pub struct Irc30Metadata {
        /// The human-readable name of the native token.
        name: String,
        /// The symbol/ticker of the token.
        symbol: String,
        /// Number of decimals the token uses (divide the token amount by
        /// `10^decimals` to get its user representation).
        decimals: u32,
        /// The human-readable description of the token.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        /// URL pointing to more resources about the token.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<Url>,
        /// URL pointing to an image resource of the token logo.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        logo_url: Option<Url>,
        /// The svg logo of the token encoded as a byte string.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        logo: Option<String>,
    }

    impl Irc30Metadata {
        pub fn new(name: impl Into<String>, symbol: impl Into<String>, decimals: u32) -> Self {
            Self {
                name: name.into(),
                symbol: symbol.into(),
                decimals,
                description: Default::default(),
                url: Default::default(),
                logo_url: Default::default(),
                logo: Default::default(),
            }
        }

        pub fn with_description(mut self, description: impl Into<String>) -> Self {
            self.description.replace(description.into());
            self
        }

        pub fn with_url(mut self, url: Url) -> Self {
            self.url.replace(url);
            self
        }

        pub fn with_logo_url(mut self, logo_url: Url) -> Self {
            self.logo_url.replace(logo_url);
            self
        }

        pub fn with_logo(mut self, logo: impl Into<String>) -> Self {
            self.logo.replace(logo.into());
            self
        }

        pub fn to_bytes(&self) -> Vec<u8> {
            // Unwrap: Safe because this struct is known to be valid
            serde_json::to_string(self).unwrap().into_bytes()
        }
    }

    impl TryFrom<Irc30Metadata> for MetadataFeature {
        type Error = Error;
        fn try_from(value: Irc30Metadata) -> Result<Self, Error> {
            Self::new(value.to_bytes())
        }
    }

    impl From<Irc30Metadata> for Vec<u8> {
        fn from(value: Irc30Metadata) -> Self {
            value.to_bytes()
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn serialization() {
            let description = "FooCoin is the utility and governance token of FooLand, \
                a revolutionary protocol in the play-to-earn crypto gaming field.";
            let metadata = Irc30Metadata::new("FooCoin", "FOO", 3)
                .with_description(description)
                .with_url("https://foocoin.io/".parse().unwrap())
                .with_logo_url(
                    "https://ipfs.io/ipfs/QmR36VFfo1hH2RAwVs4zVJ5btkopGip5cW7ydY4jUQBrkR"
                        .parse()
                        .unwrap(),
                );
            let json = serde_json::json!(
                {
                    "standard": "IRC30",
                    "name": metadata.name(),
                    "description": metadata.description(),
                    "decimals": metadata.decimals(),
                    "symbol": metadata.symbol(),
                    "url": metadata.url(),
                    "logoUrl": metadata.logo_url()
                }
            );
            let metadata_deser = serde_json::from_value::<Irc30Metadata>(json.clone()).unwrap();

            assert_eq!(metadata, metadata_deser);
            assert_eq!(json, serde_json::to_value(metadata).unwrap())
        }
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::boxed::Box;

    use serde::{Deserialize, Serialize};

    use crate::utils::serde::prefix_hex_bytes;

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct MetadataFeatureDto {
        #[serde(rename = "type")]
        pub kind: u8,
        #[serde(
            skip_serializing_if = "<[_]>::is_empty",
            default,
            with = "prefix_hex_bytes"
        )]
        pub data: Box<[u8]>,
    }
}
