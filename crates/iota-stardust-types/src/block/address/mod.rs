// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias;
mod bech32;
mod ed25519;
mod nft;

use derive_more::{Display, From};

pub use self::{
    alias::AliasAddress,
    bech32::{Bech32Address, Hrp, ToBech32Ext},
    ed25519::Ed25519Address,
    nft::NftAddress,
};
use crate::block::Error;

/// A generic address supporting different address kinds.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, From, Display, packable::Packable)]
#[packable(tag_type = u8, with_error = Error::InvalidAddressKind)]
#[packable(unpack_error = Error)]
pub enum Address {
    /// An Ed25519 address.
    #[packable(tag = Ed25519Address::KIND)]
    Ed25519(Ed25519Address),
    /// An alias address.
    #[packable(tag = AliasAddress::KIND)]
    Alias(AliasAddress),
    /// An NFT address.
    #[packable(tag = NftAddress::KIND)]
    Nft(NftAddress),
}

impl core::fmt::Debug for Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ed25519(address) => core::fmt::Display::fmt(address, f),
            Self::Alias(address) => core::fmt::Display::fmt(address, f),
            Self::Nft(address) => core::fmt::Display::fmt(address, f),
        }
    }
}

impl Address {
    /// Returns the address kind of an [`Address`].
    pub fn kind(&self) -> u8 {
        match self {
            Self::Ed25519(_) => Ed25519Address::KIND,
            Self::Alias(_) => AliasAddress::KIND,
            Self::Nft(_) => NftAddress::KIND,
        }
    }

    /// Returns the address kind of an [`Address`] as a string.
    pub fn kind_str(&self) -> &str {
        match self {
            Self::Ed25519(_) => "Ed25519",
            Self::Alias(_) => "Alias",
            Self::Nft(_) => "Nft",
        }
    }

    /// Checks whether the address is an [`Ed25519Address`].
    pub fn is_ed25519(&self) -> bool {
        matches!(self, Self::Ed25519(_))
    }

    /// Gets the address as an actual [`Ed25519Address`].
    /// PANIC: do not call on a non-ed25519 address.
    pub fn as_ed25519(&self) -> &Ed25519Address {
        if let Self::Ed25519(address) = self {
            address
        } else {
            panic!("as_ed25519 called on a non-ed25519 address");
        }
    }

    /// Checks whether the address is an [`AliasAddress`].
    pub fn is_alias(&self) -> bool {
        matches!(self, Self::Alias(_))
    }

    /// Gets the address as an actual [`AliasAddress`].
    /// PANIC: do not call on a non-alias address.
    pub fn as_alias(&self) -> &AliasAddress {
        if let Self::Alias(address) = self {
            address
        } else {
            panic!("as_alias called on a non-alias address");
        }
    }

    /// Checks whether the address is an [`NftAddress`].
    pub fn is_nft(&self) -> bool {
        matches!(self, Self::Nft(_))
    }

    /// Gets the address as an actual [`NftAddress`].
    /// PANIC: do not call on a non-nft address.
    pub fn as_nft(&self) -> &NftAddress {
        if let Self::Nft(address) = self {
            address
        } else {
            panic!("as_nft called on a non-nft address");
        }
    }

    /// Tries to create an [`Address`] from a bech32 encoded string.
    pub fn try_from_bech32(address: impl AsRef<str>) -> Result<Self, Error> {
        Bech32Address::try_from_str(address).map(|res| res.into_inner())
    }

    /// Checks if a string is a valid bech32 encoded address.
    #[must_use]
    pub fn is_valid_bech32(address: &str) -> bool {
        Self::try_from_bech32(address).is_ok()
    }
}

impl From<&Self> for Address {
    fn from(value: &Self) -> Self {
        *value
    }
}
