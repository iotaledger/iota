// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::block::address::Address;

/// Identifies the validated sender of an output.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
pub struct SenderFeature(Address);

impl SenderFeature {
    /// The [`Feature`](crate::block::output::Feature) kind of a
    /// [`SenderFeature`].
    pub const KIND: u8 = 0;

    /// Creates a new [`SenderFeature`].
    #[inline(always)]
    pub fn new(address: impl Into<Address>) -> Self {
        Self(address.into())
    }

    /// Returns the sender [`Address`].
    #[inline(always)]
    pub fn address(&self) -> &Address {
        &self.0
    }
}
