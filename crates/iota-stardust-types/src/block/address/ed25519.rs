// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;

use derive_more::{AsRef, Deref, From};

use crate::block::Error;

/// An Ed25519 address.
#[derive(
    Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, From, AsRef, Deref, packable::Packable,
)]
#[as_ref(forward)]
pub struct Ed25519Address([u8; Self::LENGTH]);

impl Ed25519Address {
    /// The [`Address`](crate::block::address::Address) kind of an
    /// [`Ed25519Address`].
    pub const KIND: u8 = 0;
    /// The length of an [`Ed25519Address`].
    pub const LENGTH: usize = 32;

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
