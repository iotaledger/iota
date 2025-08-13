// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{Error, unlock::UnlockIndex};

/// Points to the unlock of a consumed NFT output.
#[derive(Clone, Debug, Eq, PartialEq, Hash, packable::Packable)]
#[packable(unpack_error = Error, with = Error::InvalidNftIndex)]
pub struct NftUnlock(
    /// Index of input and unlock corresponding to an
    /// [`NftOutput`](crate::types::block::output::NftOutput).
    UnlockIndex,
);

impl TryFrom<u16> for NftUnlock {
    type Error = Error;

    fn try_from(index: u16) -> Result<Self, Self::Error> {
        Self::new(index)
    }
}

impl NftUnlock {
    /// The [`Unlock`](crate::types::block::unlock::Unlock) kind of a
    /// [`NftUnlock`].
    pub const KIND: u8 = 3;

    /// Creates a new [`NftUnlock`].
    #[inline(always)]
    pub fn new(index: u16) -> Result<Self, Error> {
        index.try_into().map(Self).map_err(Error::InvalidNftIndex)
    }

    /// Return the index of a [`NftUnlock`].
    #[inline(always)]
    pub fn index(&self) -> u16 {
        self.0.get()
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use serde::{Deserialize, Serialize};

    /// Points to the unlock of a consumed NFT output.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct NftUnlockDto {
        #[serde(rename = "type")]
        pub kind: u8,
        #[serde(rename = "reference")]
        pub index: u16,
    }
}
