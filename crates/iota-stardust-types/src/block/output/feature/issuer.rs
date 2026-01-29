// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use derive_more::From;

use crate::block::address::Address;

/// Identifies the validated issuer of the UTXO state machine.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, From, packable::Packable)]
pub struct IssuerFeature(Address);

impl IssuerFeature {
    /// The [`Feature`](crate::block::output::Feature) kind of an
    /// [`IssuerFeature`].
    pub const KIND: u8 = 1;

    /// Creates a new [`IssuerFeature`].
    #[inline(always)]
    pub fn new(address: impl Into<Address>) -> Self {
        Self(address.into())
    }

    /// Returns the issuer [`Address`].
    #[inline(always)]
    pub fn address(&self) -> &Address {
        &self.0
    }
}
