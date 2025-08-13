// Copyright 2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::{Deref, From};

use crate::types::block::signature::Signature;

/// An [`Unlock`](crate::types::block::unlock::Unlock) which is used to unlock a
/// signature locked [`Input`](crate::types::block::input::Input).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, Deref, packable::Packable)]
pub struct SignatureUnlock(pub(crate) Signature);

impl SignatureUnlock {
    /// The [`Unlock`](crate::types::block::unlock::Unlock) kind of a
    /// [`SignatureUnlock`].
    pub const KIND: u8 = 0;

    /// Creates a new [`SignatureUnlock`].
    #[inline(always)]
    pub fn new(signature: Signature) -> Self {
        Self(signature)
    }

    /// Returns the actual [`Signature`] of the [`SignatureUnlock`].
    #[inline(always)]
    pub fn signature(&self) -> &Signature {
        &self.0
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use serde::{Deserialize, Serialize};

    use crate::types::block::signature::dto::SignatureDto;

    /// Defines an unlock containing signature(s) unlocking input(s).
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct SignatureUnlockDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub signature: SignatureDto,
    }
}
