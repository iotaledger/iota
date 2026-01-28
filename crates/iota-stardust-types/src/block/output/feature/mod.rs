// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod issuer;
mod metadata;
mod sender;
mod tag;

use alloc::{boxed::Box, collections::BTreeSet, vec::Vec};

use bitflags::bitflags;
use derive_more::{Deref, From};
use iterator_sorted::is_unique_sorted;
use packable::{Packable, bounded::BoundedU8, prefix::BoxedSlicePrefix};

#[cfg(feature = "irc_27")]
pub use self::metadata::irc_27::{Attribute, Irc27Metadata};
#[cfg(feature = "irc_30")]
pub use self::metadata::irc_30::Irc30Metadata;
pub use self::{
    issuer::IssuerFeature, metadata::MetadataFeature, sender::SenderFeature, tag::TagFeature,
};
pub(crate) use self::{metadata::MetadataFeatureLength, tag::TagFeatureLength};
use crate::block::Error;

///
#[derive(Clone, Eq, PartialEq, Hash, From, Packable)]
#[packable(unpack_error = Error)]
#[packable(tag_type = u8, with_error = Error::InvalidFeatureKind)]
pub enum Feature {
    /// A sender feature.
    #[packable(tag = SenderFeature::KIND)]
    Sender(SenderFeature),
    /// An issuer feature.
    #[packable(tag = IssuerFeature::KIND)]
    Issuer(IssuerFeature),
    /// A metadata feature.
    #[packable(tag = MetadataFeature::KIND)]
    Metadata(MetadataFeature),
    /// A tag feature.
    #[packable(tag = TagFeature::KIND)]
    Tag(TagFeature),
}

impl PartialOrd for Feature {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Feature {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.kind().cmp(&other.kind())
    }
}

impl core::fmt::Debug for Feature {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Sender(feature) => feature.fmt(f),
            Self::Issuer(feature) => feature.fmt(f),
            Self::Metadata(feature) => feature.fmt(f),
            Self::Tag(feature) => feature.fmt(f),
        }
    }
}

impl Feature {
    /// Return the output kind of an `Output`.
    pub fn kind(&self) -> u8 {
        match self {
            Self::Sender(_) => SenderFeature::KIND,
            Self::Issuer(_) => IssuerFeature::KIND,
            Self::Metadata(_) => MetadataFeature::KIND,
            Self::Tag(_) => TagFeature::KIND,
        }
    }

    /// Returns the [`FeatureFlags`] for the given [`Feature`].
    pub fn flag(&self) -> FeatureFlags {
        match self {
            Self::Sender(_) => FeatureFlags::SENDER,
            Self::Issuer(_) => FeatureFlags::ISSUER,
            Self::Metadata(_) => FeatureFlags::METADATA,
            Self::Tag(_) => FeatureFlags::TAG,
        }
    }

    /// Checks whether the feature is a [`SenderFeature`].
    pub fn is_sender(&self) -> bool {
        matches!(self, Self::Sender(_))
    }

    /// Gets the feature as an actual [`SenderFeature`].
    /// NOTE: Will panic if the feature is not a [`SenderFeature`].
    pub fn as_sender(&self) -> &SenderFeature {
        if let Self::Sender(feature) = self {
            feature
        } else {
            panic!("invalid downcast of non-SenderFeature");
        }
    }

    /// Checks whether the feature is an [`IssuerFeature`].
    pub fn is_issuer(&self) -> bool {
        matches!(self, Self::Issuer(_))
    }

    /// Gets the feature as an actual [`IssuerFeature`].
    /// NOTE: Will panic if the feature is not an [`IssuerFeature`].
    pub fn as_issuer(&self) -> &IssuerFeature {
        if let Self::Issuer(feature) = self {
            feature
        } else {
            panic!("invalid downcast of non-IssuerFeature");
        }
    }

    /// Checks whether the feature is a [`MetadataFeature`].
    pub fn is_metadata(&self) -> bool {
        matches!(self, Self::Metadata(_))
    }

    /// Gets the feature as an actual [`MetadataFeature`].
    /// NOTE: Will panic if the feature is not a [`MetadataFeature`].
    pub fn as_metadata(&self) -> &MetadataFeature {
        if let Self::Metadata(feature) = self {
            feature
        } else {
            panic!("invalid downcast of non-MetadataFeature");
        }
    }

    /// Checks whether the feature is a [`TagFeature`].
    pub fn is_tag(&self) -> bool {
        matches!(self, Self::Tag(_))
    }

    /// Gets the feature as an actual [`TagFeature`].
    /// NOTE: Will panic if the feature is not a [`TagFeature`].
    pub fn as_tag(&self) -> &TagFeature {
        if let Self::Tag(feature) = self {
            feature
        } else {
            panic!("invalid downcast of non-TagFeature");
        }
    }
}

create_bitflags!(
    /// A bitflags-based representation of the set of active [`Feature`]s.
    pub FeatureFlags,
    u16,
    [
        (SENDER, SenderFeature),
        (ISSUER, IssuerFeature),
        (METADATA, MetadataFeature),
        (TAG, TagFeature),
    ]
);

pub(crate) type FeatureCount = BoundedU8<0, { Features::COUNT_MAX }>;

///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Deref, Packable)]
#[packable(unpack_error = Error, with = |e| e.unwrap_item_err_or_else(|p| Error::InvalidFeatureCount(p.into())))]
pub struct Features(
    #[packable(verify_with = verify_unique_sorted)] BoxedSlicePrefix<Feature, FeatureCount>,
);

impl TryFrom<Vec<Feature>> for Features {
    type Error = Error;

    #[inline(always)]
    fn try_from(features: Vec<Feature>) -> Result<Self, Self::Error> {
        Self::from_vec(features)
    }
}

impl TryFrom<BTreeSet<Feature>> for Features {
    type Error = Error;

    #[inline(always)]
    fn try_from(features: BTreeSet<Feature>) -> Result<Self, Self::Error> {
        Self::from_set(features)
    }
}

impl IntoIterator for Features {
    type Item = Feature;
    type IntoIter = alloc::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Vec::from(Into::<Box<[Feature]>>::into(self.0)).into_iter()
    }
}

impl Features {
    ///
    pub const COUNT_MAX: u8 = 4;

    /// Creates a new [`Features`] from a vec.
    pub fn from_vec(features: Vec<Feature>) -> Result<Self, Error> {
        let mut features =
            BoxedSlicePrefix::<Feature, FeatureCount>::try_from(features.into_boxed_slice())
                .map_err(Error::InvalidFeatureCount)?;

        features.sort_by_key(Feature::kind);
        // Sort is obviously fine now but uniqueness still needs to be checked.
        verify_unique_sorted::<true>(&features, &())?;

        Ok(Self(features))
    }

    /// Creates a new [`Features`] from an ordered set.
    pub fn from_set(features: BTreeSet<Feature>) -> Result<Self, Error> {
        Ok(Self(
            features
                .into_iter()
                .collect::<Box<[_]>>()
                .try_into()
                .map_err(Error::InvalidFeatureCount)?,
        ))
    }

    /// Gets a reference to a [`Feature`] from a feature kind, if any.
    #[inline(always)]
    pub fn get(&self, key: u8) -> Option<&Feature> {
        self.0
            .binary_search_by_key(&key, Feature::kind)
            // PANIC: indexation is fine since the index has been found.
            .map(|index| &self.0[index])
            .ok()
    }

    /// Gets a reference to a [`SenderFeature`], if any.
    pub fn sender(&self) -> Option<&SenderFeature> {
        self.get(SenderFeature::KIND).map(Feature::as_sender)
    }

    /// Gets a reference to a [`IssuerFeature`], if any.
    pub fn issuer(&self) -> Option<&IssuerFeature> {
        self.get(IssuerFeature::KIND).map(Feature::as_issuer)
    }

    /// Gets a reference to a [`MetadataFeature`], if any.
    pub fn metadata(&self) -> Option<&MetadataFeature> {
        self.get(MetadataFeature::KIND).map(Feature::as_metadata)
    }

    /// Gets a reference to a [`TagFeature`], if any.
    pub fn tag(&self) -> Option<&TagFeature> {
        self.get(TagFeature::KIND).map(Feature::as_tag)
    }
}

#[inline]
fn verify_unique_sorted<const VERIFY: bool>(features: &[Feature], _: &()) -> Result<(), Error> {
    if VERIFY && !is_unique_sorted(features.iter().map(Feature::kind)) {
        Err(Error::FeaturesNotUniqueSorted)
    } else {
        Ok(())
    }
}

pub(crate) fn verify_allowed_features(
    features: &Features,
    allowed_features: FeatureFlags,
) -> Result<(), Error> {
    for (index, feature) in features.iter().enumerate() {
        if !allowed_features.contains(feature.flag()) {
            return Err(Error::UnallowedFeature {
                index,
                kind: feature.kind(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn all_flags_present() {
        assert_eq!(
            FeatureFlags::ALL_FLAGS,
            &[
                FeatureFlags::SENDER,
                FeatureFlags::ISSUER,
                FeatureFlags::METADATA,
                FeatureFlags::TAG
            ]
        );
    }
}
