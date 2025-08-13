// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;

use crypto::signatures::ed25519::PublicKey;
use derive_more::{AsRef, Deref, From};

use crate::types::block::Error;

/// An Ed25519 address.
#[derive(
    Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, From, AsRef, Deref, packable::Packable,
)]
#[as_ref(forward)]
pub struct Ed25519Address([u8; Self::LENGTH]);

impl Ed25519Address {
    /// The [`Address`](crate::types::block::address::Address) kind of an
    /// [`Ed25519Address`].
    pub const KIND: u8 = 0;
    /// The length of an [`Ed25519Address`].
    pub const LENGTH: usize = PublicKey::LENGTH;

    /// Creates a new [`Ed25519Address`].
    #[inline(always)]
    pub fn new(address: [u8; Self::LENGTH]) -> Self {
        Self::from(address)
    }
}

#[cfg(feature = "serde")]
string_serde_impl!(Ed25519Address);

impl FromStr for Ed25519Address {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(prefix_hex::decode(s).map_err(Error::Hex)?))
    }
}

impl core::fmt::Display for Ed25519Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", prefix_hex::encode(self.0))
    }
}

impl core::fmt::Debug for Ed25519Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Ed25519Address({self})")
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::string::{String, ToString};

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::block::Error;

    /// Describes an Ed25519 address.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Ed25519AddressDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub pub_key_hash: String,
    }

    impl From<&Ed25519Address> for Ed25519AddressDto {
        fn from(value: &Ed25519Address) -> Self {
            Self {
                kind: Ed25519Address::KIND,
                pub_key_hash: value.to_string(),
            }
        }
    }

    impl TryFrom<Ed25519AddressDto> for Ed25519Address {
        type Error = Error;

        fn try_from(value: Ed25519AddressDto) -> Result<Self, Self::Error> {
            value
                .pub_key_hash
                .parse::<Self>()
                .map_err(|_| Error::InvalidField("pubKeyHash"))
        }
    }
}
