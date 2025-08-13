// Copyright 2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;

use derive_more::{AsRef, Deref, From};

use crate::types::block::{
    Error,
    output::{AliasId, OutputId},
};

/// An alias address.
#[derive(
    Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, From, AsRef, Deref, packable::Packable,
)]
#[as_ref(forward)]
pub struct AliasAddress(AliasId);

impl AliasAddress {
    /// The [`Address`](crate::types::block::address::Address) kind of an
    /// [`AliasAddress`].
    pub const KIND: u8 = 8;
    /// The length of an [`AliasAddress`].
    pub const LENGTH: usize = AliasId::LENGTH;

    /// Creates a new [`AliasAddress`].
    #[inline(always)]
    pub fn new(id: AliasId) -> Self {
        Self::from(id)
    }

    /// Returns the [`AliasId`] of an [`AliasAddress`].
    #[inline(always)]
    pub fn alias_id(&self) -> &AliasId {
        &self.0
    }

    /// Consumes an [`AliasAddress`] and returns its [`AliasId`].
    #[inline(always)]
    pub fn into_alias_id(self) -> AliasId {
        self.0
    }
}

#[cfg(feature = "serde")]
string_serde_impl!(AliasAddress);

impl FromStr for AliasAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(AliasId::from_str(s)?))
    }
}

impl From<&OutputId> for AliasAddress {
    fn from(output_id: &OutputId) -> Self {
        Self(AliasId::from(output_id))
    }
}

impl core::fmt::Display for AliasAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Debug for AliasAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AliasAddress({self})")
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::string::{String, ToString};

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::block::Error;

    /// Describes an alias address.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AliasAddressDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub alias_id: String,
    }

    impl From<&AliasAddress> for AliasAddressDto {
        fn from(value: &AliasAddress) -> Self {
            Self {
                kind: AliasAddress::KIND,
                alias_id: value.to_string(),
            }
        }
    }

    impl TryFrom<AliasAddressDto> for AliasAddress {
        type Error = Error;

        fn try_from(value: AliasAddressDto) -> Result<Self, Self::Error> {
            value
                .alias_id
                .parse::<Self>()
                .map_err(|_| Error::InvalidField("aliasId"))
        }
    }
}
