// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod alias;
mod bech32;
mod ed25519;
mod nft;

use derive_more::{Display, From};

pub use self::{
    alias::AliasAddress,
    bech32::{Bech32Address, Hrp},
    ed25519::Ed25519Address,
    nft::NftAddress,
};
use crate::types::block::{
    ConvertTo, Error,
    output::{Output, OutputId},
    semantic::{ConflictReason, ValidationContext},
    signature::Signature,
    unlock::Unlock,
};

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
            Self::Ed25519(address) => address.fmt(f),
            Self::Alias(address) => address.fmt(f),
            Self::Nft(address) => address.fmt(f),
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
        Bech32Address::try_from_str(address).map(|res| res.inner)
    }

    /// Checks if an string is a valid bech32 encoded address.
    #[must_use]
    pub fn is_valid_bech32(address: &str) -> bool {
        Self::try_from_bech32(address).is_ok()
    }

    ///
    pub fn unlock(
        &self,
        unlock: &Unlock,
        inputs: &[(&OutputId, &Output)],
        context: &mut ValidationContext<'_>,
    ) -> Result<(), ConflictReason> {
        match (self, unlock) {
            (Self::Ed25519(ed25519_address), Unlock::Signature(unlock)) => {
                if context.unlocked_addresses.contains(self) {
                    return Err(ConflictReason::InvalidUnlock);
                }

                let Signature::Ed25519(signature) = unlock.signature();

                if signature
                    .is_valid(&context.essence_hash, ed25519_address)
                    .is_err()
                {
                    return Err(ConflictReason::InvalidSignature);
                }

                context.unlocked_addresses.insert(*self);
            }
            (Self::Ed25519(_ed25519_address), Unlock::Reference(_unlock)) => {
                // TODO actually check that it was unlocked by the same signature.
                if !context.unlocked_addresses.contains(self) {
                    return Err(ConflictReason::InvalidUnlock);
                }
            }
            (Self::Alias(alias_address), Unlock::Alias(unlock)) => {
                // PANIC: indexing is fine as it is already syntactically verified that indexes
                // reference below.
                if let (output_id, Output::Alias(alias_output)) = inputs[unlock.index() as usize] {
                    if &alias_output.alias_id_non_null(output_id) != alias_address.alias_id() {
                        return Err(ConflictReason::InvalidUnlock);
                    }
                    if !context.unlocked_addresses.contains(self) {
                        return Err(ConflictReason::InvalidUnlock);
                    }
                } else {
                    return Err(ConflictReason::InvalidUnlock);
                }
            }
            (Self::Nft(nft_address), Unlock::Nft(unlock)) => {
                // PANIC: indexing is fine as it is already syntactically verified that indexes
                // reference below.
                if let (output_id, Output::Nft(nft_output)) = inputs[unlock.index() as usize] {
                    if &nft_output.nft_id_non_null(output_id) != nft_address.nft_id() {
                        return Err(ConflictReason::InvalidUnlock);
                    }
                    if !context.unlocked_addresses.contains(self) {
                        return Err(ConflictReason::InvalidUnlock);
                    }
                } else {
                    return Err(ConflictReason::InvalidUnlock);
                }
            }
            _ => return Err(ConflictReason::InvalidUnlock),
        }

        Ok(())
    }
}

pub trait ToBech32Ext: Sized {
    /// Try to encode this address to a bech32 string with the given Human
    /// Readable Part as prefix.
    fn try_to_bech32(self, hrp: impl ConvertTo<Hrp>) -> Result<Bech32Address, Error>;

    /// Encodes this address to a bech32 string with the given Human Readable
    /// Part as prefix.
    fn to_bech32(self, hrp: Hrp) -> Bech32Address;

    /// Encodes this address to a bech32 string with the given Human Readable
    /// Part as prefix without checking validity.
    fn to_bech32_unchecked(self, hrp: impl ConvertTo<Hrp>) -> Bech32Address;
}

impl<T: Into<Address>> ToBech32Ext for T {
    /// Try to encode this address to a bech32 string with the given Human
    /// Readable Part as prefix.
    fn try_to_bech32(self, hrp: impl ConvertTo<Hrp>) -> Result<Bech32Address, Error> {
        Bech32Address::try_new(hrp, self)
    }

    /// Encodes this address to a bech32 string with the given Human Readable
    /// Part as prefix.
    fn to_bech32(self, hrp: Hrp) -> Bech32Address {
        Bech32Address::new(hrp, self)
    }

    /// Encodes this address to a bech32 string with the given Human Readable
    /// Part as prefix without checking validity.
    fn to_bech32_unchecked(self, hrp: impl ConvertTo<Hrp>) -> Bech32Address {
        Bech32Address::new(hrp.convert_unchecked(), self)
    }
}

impl From<&Self> for Address {
    fn from(value: &Self) -> Self {
        *value
    }
}

#[cfg(feature = "serde")]
pub mod dto {
    use alloc::format;

    use serde::{Deserialize, Serialize, Serializer};
    use serde_json::Value;

    use super::*;
    pub use super::{
        alias::dto::AliasAddressDto, ed25519::dto::Ed25519AddressDto, nft::dto::NftAddressDto,
    };
    use crate::types::block::Error;

    /// Describes all the different address types.
    #[derive(Clone, Debug, Eq, PartialEq, From)]
    pub enum AddressDto {
        /// An Ed25519 address.
        Ed25519(Ed25519AddressDto),
        /// An alias address.
        Alias(AliasAddressDto),
        /// A NFT address.
        Nft(NftAddressDto),
    }

    impl From<&Address> for AddressDto {
        fn from(value: &Address) -> Self {
            match value {
                Address::Ed25519(a) => Self::Ed25519(a.into()),
                Address::Alias(a) => Self::Alias(a.into()),
                Address::Nft(a) => Self::Nft(a.into()),
            }
        }
    }

    impl TryFrom<AddressDto> for Address {
        type Error = Error;

        fn try_from(value: AddressDto) -> Result<Self, Self::Error> {
            match value {
                AddressDto::Ed25519(a) => Ok(Self::Ed25519(a.try_into()?)),
                AddressDto::Alias(a) => Ok(Self::Alias(a.try_into()?)),
                AddressDto::Nft(a) => Ok(Self::Nft(a.try_into()?)),
            }
        }
    }

    impl<'de> Deserialize<'de> for AddressDto {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let value = Value::deserialize(d)?;
            Ok(
                match value
                    .get("type")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| serde::de::Error::custom("invalid address type"))?
                    as u8
                {
                    Ed25519Address::KIND => {
                        Self::Ed25519(Ed25519AddressDto::deserialize(value).map_err(|e| {
                            serde::de::Error::custom(format!(
                                "cannot deserialize ed25519 address: {e}"
                            ))
                        })?)
                    }
                    AliasAddress::KIND => {
                        Self::Alias(AliasAddressDto::deserialize(value).map_err(|e| {
                            serde::de::Error::custom(format!(
                                "cannot deserialize alias address: {e}"
                            ))
                        })?)
                    }
                    NftAddress::KIND => {
                        Self::Nft(NftAddressDto::deserialize(value).map_err(|e| {
                            serde::de::Error::custom(format!("cannot deserialize NFT address: {e}"))
                        })?)
                    }
                    _ => return Err(serde::de::Error::custom("invalid address type")),
                },
            )
        }
    }

    impl Serialize for AddressDto {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            #[derive(Serialize)]
            #[serde(untagged)]
            enum AddressDto_<'a> {
                T1(&'a Ed25519AddressDto),
                T2(&'a AliasAddressDto),
                T3(&'a NftAddressDto),
            }
            #[derive(Serialize)]
            struct TypedAddress<'a> {
                #[serde(flatten)]
                address: AddressDto_<'a>,
            }
            let address = match self {
                Self::Ed25519(o) => TypedAddress {
                    address: AddressDto_::T1(o),
                },
                Self::Alias(o) => TypedAddress {
                    address: AddressDto_::T2(o),
                },
                Self::Nft(o) => TypedAddress {
                    address: AddressDto_::T3(o),
                },
            };
            address.serialize(serializer)
        }
    }
}
