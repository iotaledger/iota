// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::block::{Error, unlock::UnlockIndex};

/// Points to the unlock of a consumed alias output.
#[derive(Clone, Debug, Eq, PartialEq, Hash, packable::Packable)]
#[packable(unpack_error = Error, with = Error::InvalidAliasIndex)]
pub struct AliasUnlock(
    /// Index of input and unlock corresponding to an
    /// [`AliasOutput`](crate::types::block::output::AliasOutput).
    UnlockIndex,
);

impl TryFrom<u16> for AliasUnlock {
    type Error = Error;

    fn try_from(index: u16) -> Result<Self, Self::Error> {
        Self::new(index)
    }
}

impl AliasUnlock {
    /// The [`Unlock`](crate::types::block::unlock::Unlock) kind of an
    /// [`AliasUnlock`].
    pub const KIND: u8 = 2;

    /// Creates a new [`AliasUnlock`].
    #[inline(always)]
    pub fn new(index: u16) -> Result<Self, Error> {
        index.try_into().map(Self).map_err(Error::InvalidAliasIndex)
    }

    /// Return the index of an [`AliasUnlock`].
    #[inline(always)]
    pub fn index(&self) -> u16 {
        self.0.get()
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use serde::{Deserialize, Serialize};

    /// Points to the unlock of a consumed alias output.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct AliasUnlockDto {
        #[serde(rename = "type")]
        pub kind: u8,
        #[serde(rename = "reference")]
        pub index: u16,
    }
}
